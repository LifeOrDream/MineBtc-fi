//! Faction War lifecycle and reward distribution.
//!
//! # Faction War Cycle
//!
//! Each war cycle follows a strict lifecycle.  Off-chain crankers and the game
//! round pipeline coordinate to move the cycle forward.
//!
//! ```text
//!   ┌─────────────────────────────────────────────────────────────────────────┐
//!   │  PHASE 0  —  INIT CONFIG  (once, admin)                                │
//!   │  call: initialize_war_config                                   │
//!   │  creates: FactionWarConfig PDA                                         │
//!   └─────────────────────────────────────────────────────────────────────────┘
//!                                    │
//!                                    ▼  (repeats every cycle)
//!   ┌─────────────────────────────────────────────────────────────────────────┐
//!   │  PHASE 1  —  START WAR  (cranker)                                      │
//!   │  call: initialize_faction_war(war_id)                          │
//!   │  creates: FactionWarState + FactionWarSettlement PDAs                  │
//!   │  seeds treasury base from unassigned tax, sets settle_cycle            │
//!   └─────────────────────────────────────────────────────────────────────────┘
//!                                    │
//!                                    ▼
//!   ┌─────────────────────────────────────────────────────────────────────────┐
//!   │  PHASE 2  —  ACTIVE  (many rounds, permissionless gameplay)            │
//!   │  start_round → end_round → settle_round  (loop)                        │
//!   │  settle_round calls track_faction_war_round_completion()               │
//!   │    · accumulates gameplay_scores, round_wins, mined amounts            │
//!   │    · when lp_operations_count >= settle_at_lp_op_count → auto-settle│
//!   └─────────────────────────────────────────────────────────────────────────┘
//!                                    │
//!                                    ▼
//!   ┌─────────────────────────────────────────────────────────────────────────┐
//!   │  PHASE 3  —  SETTLE  (cranker, once per cycle)                         │
//!   │  call: settle_war                                              │
//!   │  internally calls finalize_war_settlement()                    │
//!   │    · computes final_ranks from gameplay_scores + round_wins            │
//!   │    · computes rank_deltas, resolved_directions                         │
//!   │    · applies mining_multiplier_bps, splits pool into lanes             │
//!   │    · advances current_war_id                                   │
//!   └─────────────────────────────────────────────────────────────────────────┘
//!                                    │
//!                                    ▼
//!   ┌─────────────────────────────────────────────────────────────────────────┐
//!   │  PHASE 4  —  CLAIMS  (users + factions, permissionless)                │
//!   │  users:  claim_faction_war_rewards  (reads FactionWarSettlement)       │
//!   │  factions: claim_faction_treasury_for_faction_war (reads final_ranks)  │
//!   └─────────────────────────────────────────────────────────────────────────┘
//!                                    │
//!                                    ▼
//!                           back to PHASE 1
//! ```
//!
//! # File layout
//!
//! 1. **Helpers** – pure/computation functions (rankings, reward pools, etc.)
//! 2. **Lifecycle functions** – ordered by call sequence
//!    · `initialize_war_config_internal`  (admin, once)
//!    · `initialize_faction_war_internal`         (cranker, per cycle start)
//!    · `finalize_war_settlement`         (internal, called by settle)
//!    · `settle_war_internal`             (cranker, per cycle end)
//!    · `claim_faction_war_rewards_internal`      (user claim)
//! 3. **Account structs** – all `#[derive(Accounts)]` grouped at the end,
//!    in the same order as their handler functions above.

use anchor_lang::prelude::*;

use crate::errors::ErrorCode;
use crate::events::*;
use crate::genescience::{calculate_mutation_result, MutationType};
use crate::instructions::helper;
use crate::state::*;

// ========================================================================================
// ============================= HELPERS ==================================================
// ========================================================================================

pub fn compute_rankings_into(
    scores: &[i64; NUM_FACTIONS],
    round_wins: &[u16; NUM_FACTIONS],
    active_factions: usize,
    ranks: &mut [u8; NUM_FACTIONS],
) -> Result<()> {
    let mut ordered = [0u8; NUM_FACTIONS];
    for (idx, slot) in ordered.iter_mut().enumerate() {
        *slot = idx as u8;
    }

    // Tiebreaker chain: gameplay score → round wins → faction_id (ascending).
    ordered[..active_factions].sort_by(|a, b| {
        scores[*b as usize]
            .cmp(&scores[*a as usize])
            .then_with(|| round_wins[*b as usize].cmp(&round_wins[*a as usize]))
            .then_with(|| a.cmp(b))
    });
    *ranks = [0u8; NUM_FACTIONS];
    for (rank, faction_id) in ordered[..active_factions].iter().enumerate() {
        ranks[*faction_id as usize] = rank as u8;
    }
    Ok(())
}

pub fn resolve_direction_from_ranks(start_rank: u8, final_rank: u8) -> (PredictionDirection, i8) {
    let delta = start_rank as i8 - final_rank as i8;
    let direction = if delta > 0 {
        PredictionDirection::Up
    } else if delta < 0 {
        PredictionDirection::Down
    } else {
        PredictionDirection::Neutral
    };

    (direction, delta)
}

/// Apply the dynamic mining multiplier to the raw degenBTC mined in a cycle.
/// `multiplier_bps` is in basis points (10_000 = 1.0x).
#[inline(always)]
fn apply_mining_multiplier(raw_mined: u64, multiplier_bps: u16) -> Result<u64> {
    require!(
        (MIN_FACTION_WAR_MINING_MULTIPLIER_BPS..=MAX_FACTION_WAR_MINING_MULTIPLIER_BPS)
            .contains(&multiplier_bps),
        ErrorCode::InvalidParameters
    );
    if raw_mined == 0 || multiplier_bps == 10_000 {
        return Ok(raw_mined);
    }
    let result = (raw_mined as u128)
        .checked_mul(multiplier_bps as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(BASIS_POINTS_DENOMINATOR as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let result_u64 = u64::try_from(result).map_err(|_| ErrorCode::ArithmeticOverflow)?;
    Ok(result_u64)
}

fn mutation_type_to_u8(mutation_type: MutationType) -> u8 {
    match mutation_type {
        MutationType::Evolution => 1,
        MutationType::Power => 2,
        MutationType::Trait => 3,
    }
}

fn checked_bps_mul(lhs_bps: u64, rhs_bps: u64) -> u64 {
    lhs_bps
        .saturating_mul(rhs_bps)
        .saturating_div(BASIS_POINTS_DENOMINATOR)
}

fn total_cycle_weighted_volume(war_state: &FactionWarState, active_factions: usize) -> u64 {
    let mut total = 0u64;
    for faction_id in 0..active_factions {
        for direction in 0..PredictionDirection::COUNT {
            total = total
                .saturating_add(war_state.faction_direction_totals[faction_id][direction]);
        }
    }
    total
}

fn user_correct_cycle_sol(
    user_faction_war_bets: &UserFactionWarBets,
    war_settlement: &FactionWarSettlement,
    active_factions: usize,
) -> u64 {
    let mut total = 0u64;
    for faction_id in 0..active_factions {
        let resolved = war_settlement.resolved_directions[faction_id] as usize;
        if resolved < PredictionDirection::COUNT {
            total = total
                .saturating_add(user_faction_war_bets.sol_direction_bets[faction_id][resolved]);
        }
    }
    total
}

#[allow(clippy::too_many_arguments)]
fn process_faction_war_claim_hashbeast_update<'info>(
    war_id: u64,
    owner_key: Pubkey,
    war_state: &FactionWarState,
    war_settlement: &FactionWarSettlement,
    user_faction_war_bets: &UserFactionWarBets,
    player_data: &mut PlayerData,
    tuning: &GameplayTuningConfig,
    hashbeast_metadata: Option<&mut Box<Account<'info, HashBeastMetadata>>>,
    hashbeast_bonus_amount: u64,
    claim_won: bool,
) -> Result<u8> {
    if !claim_won
        || user_faction_war_bets.gameplay_hashbeast == Pubkey::default()
        || user_faction_war_bets.gameplay_hashbeast != player_data.gameplay_hashbeast
        || player_data.gameplay_hashbeast == Pubkey::default()
    {
        return Ok(0);
    }

    let active_factions = war_state.faction_count as usize;

    let player_faction_id = player_data.faction_id as usize;
    if player_faction_id >= active_factions {
        return Ok(0);
    }

    let resolved_direction = war_settlement.resolved_directions[player_faction_id] as usize;
    if resolved_direction >= PredictionDirection::COUNT {
        return Ok(0);
    }

    let home_correct_sol =
        user_faction_war_bets.sol_direction_bets[player_faction_id][resolved_direction];
    let all_correct_sol =
        user_correct_cycle_sol(user_faction_war_bets, war_settlement, active_factions);
    let mut stake = home_correct_sol;
    let mut chance_boost_bps = 0u64;

    if home_correct_sol > 0 {
        let rank_delta_abs = war_settlement.rank_deltas[player_faction_id].unsigned_abs() as u64;
        let movement_boost_bps = match resolved_direction {
            // Correctly backing your own country moving up is the premium mutation path.
            2 => 18_000u64
                .saturating_add(rank_delta_abs.saturating_mul(2_000))
                .min(30_000),
            1 => 12_000,
            _ => 8_000,
        };
        chance_boost_bps = checked_bps_mul(movement_boost_bps, 15_000);
    } else if all_correct_sol > 0 {
        // Correct cycle calls outside the user's home country can still produce a small roll.
        stake = all_correct_sol / 4;
        chance_boost_bps = 5_000;
    }

    let mut mutation_type_u8 = 0u8;
    if tuning.rpg_progression && stake > 0 && chance_boost_bps > 0 {
        let total_sol = war_state.total_cycle_sol;
        let total_weighted = total_cycle_weighted_volume(war_state, active_factions);
        // We no longer track per-faction loyalty SOL; both branches collapse to
        // total cycle SOL as the volume_factor denominator.
        let faction_volume = total_sol;

        let mutation_result = calculate_mutation_result(
            STORY_EVENT_ORIGIN_FACTION_WAR,
            war_id,
            stake,
            player_data.active_multiplier,
            player_data.gameplay_hashbeast_dna,
            player_data.gameplay_hashbeast_xp,
            tuning.max_evolution_stage_unlocked,
            0,
            faction_volume.max(stake),
            tuning,
            chance_boost_bps,
            tuning.target_rounds_per_cycle,
            0,
            total_sol.max(stake),
            total_weighted,
            total_weighted,
            war_state
                .start_timestamp
                .saturating_add(war_id),
            &owner_key,
            &player_data.gameplay_hashbeast,
        );

        player_data.gameplay_hashbeast_xp = player_data
            .gameplay_hashbeast_xp
            .checked_add(mutation_result.xp_gained)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        if let Some(mutation_type) = mutation_result.mutation_type {
            let new_mult = player_data
                .active_multiplier
                .checked_add(mutation_result.multiplier_increase)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            player_data.active_multiplier = new_mult.min(GAMEPLAY_MAX_MULTIPLIER as u32);
            player_data.gameplay_hashbeast_xp = player_data
                .gameplay_hashbeast_xp
                .saturating_sub(mutation_result.xp_consumed);
            player_data.gameplay_hashbeast_dna = mutation_result.new_dna;

            mutation_type_u8 = mutation_type_to_u8(mutation_type);
            emit!(StoryEventTriggered {
                origin: STORY_EVENT_ORIGIN_FACTION_WAR,
                origin_id: war_id,
                user: owner_key,
                hashbeast_mint: player_data.gameplay_hashbeast,
                story_event_type: mutation_type_u8,
                xp_gained: mutation_result.xp_gained,
                multiplier_after: player_data.active_multiplier,
            });
        }
    }

    let should_sync_hashbeast = hashbeast_bonus_amount > 0 || (tuning.rpg_progression && stake > 0);
    if should_sync_hashbeast {
        require!(
            hashbeast_metadata.is_some()
                && hashbeast_metadata.as_ref().unwrap().mint
                    == user_faction_war_bets.gameplay_hashbeast,
            ErrorCode::HashBeastMetadataNotFound
        );
        let hashbeast_metadata = hashbeast_metadata.unwrap();
        if hashbeast_bonus_amount > 0 {
            hashbeast_metadata.accumulated_val = hashbeast_metadata
                .accumulated_val
                .checked_add(hashbeast_bonus_amount)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
        hashbeast_metadata.dna = player_data.gameplay_hashbeast_dna;
        hashbeast_metadata.xp = player_data.gameplay_hashbeast_xp;
        hashbeast_metadata.multiplier = player_data.active_multiplier;
        emit!(HashBeastSynced {
            hashbeast_mint: hashbeast_metadata.mint,
            hashbeast_metadata_account: hashbeast_metadata.key(),
            dna: hashbeast_metadata.dna.to_vec(),
            xp: hashbeast_metadata.xp,
            multiplier: hashbeast_metadata.multiplier,
            accumulated_val: hashbeast_metadata.accumulated_val,
            accum_pct: 1000,
        });
    }

    Ok(mutation_type_u8)
}

fn compute_rank_weighted_pools_into(
    pool_total: u64,
    final_ranks: &[u8; NUM_FACTIONS],
    eligible_factions: &[bool; NUM_FACTIONS],
    active_factions: usize,
    pools: &mut [u64; NUM_FACTIONS],
) -> Result<()> {
    *pools = [0u64; NUM_FACTIONS];
    if pool_total == 0 {
        return Ok(());
    }

    let mut ordered_factions = [0u8; NUM_FACTIONS];
    for (idx, slot) in ordered_factions.iter_mut().enumerate() {
        *slot = idx as u8;
    }
    ordered_factions[..active_factions].sort_by_key(|faction_id| final_ranks[*faction_id as usize]);

    let mut total_rank_weight: u128 = 0;
    let mut eligible_count = 0usize;
    for faction_id in ordered_factions.iter().take(active_factions).copied() {
        if eligible_factions[faction_id as usize] {
            let weight =
                FACTION_WAR_RANK_WEIGHT_BPS[final_ranks[faction_id as usize] as usize] as u128;
            total_rank_weight = total_rank_weight
                .checked_add(weight)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            eligible_count += 1;
        }
    }

    if eligible_count == 0 || total_rank_weight == 0 {
        return Ok(());
    }

    let mut distributed = 0u64;
    let mut remaining_eligible = eligible_count;

    for faction_id in ordered_factions.iter().take(active_factions).copied() {
        let faction_index = faction_id as usize;
        if !eligible_factions[faction_index] {
            continue;
        }

        remaining_eligible -= 1;
        let share = if remaining_eligible == 0 {
            pool_total
                .checked_sub(distributed)
                .ok_or(ErrorCode::ArithmeticOverflow)?
        } else {
            let rank_weight =
                FACTION_WAR_RANK_WEIGHT_BPS[final_ranks[faction_index] as usize] as u128;
            let raw_share = (pool_total as u128)
                .checked_mul(rank_weight)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(total_rank_weight)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            u64::try_from(raw_share).map_err(|_| error!(ErrorCode::ArithmeticOverflow))?
        };

        pools[faction_index] = share;
        distributed = distributed
            .checked_add(share)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    Ok(())
}

fn pool_share_from_bps(pool: u64, bps: u16) -> Result<u64> {
    let share = (pool as u128)
        .checked_mul(bps as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(BASIS_POINTS_DENOMINATOR as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    u64::try_from(share).map_err(|_| ErrorCode::ArithmeticOverflow.into())
}

/// Rank-weight curve for the MVP bonus lane. 30/20/14/10/8/6/5/4 bps for
/// the top 8 ranks; ranks 8..active share an additional 3% equally. Curve
/// shape inspired by ALGS / poker-tournament payouts — top-heavy but with
/// a meaningful tail so smaller countries' MVPs still get rewarded.
fn mvp_rank_weight_bps(rank: usize, active_factions: usize) -> u64 {
    match rank {
        0 => 3000,
        1 => 2000,
        2 => 1400,
        3 => 1000,
        4 => 800,
        5 => 600,
        6 => 500,
        7 => 400,
        _ => {
            let tail_count = active_factions.saturating_sub(8).max(1) as u64;
            300 / tail_count
        }
    }
}

/// Compute how the faction_war mining pool is split across factions.
///
/// The total pool is first split into three lanes:
/// - base rewards: anyone correct on a country's resolved direction
/// - mvp rewards: top contributor per faction (distributed at settlement by rank)
/// - HashBeast rewards: gameplay HashBeasts backing the resolved home-country outcome
///
/// Each lane is then distributed across factions by final rank, normalized only
/// across factions that have eligible claimants for that lane.
pub fn compute_base_reward_pools(
    war_state: &FactionWarState,
    war_settlement: &mut FactionWarSettlement,
    tuning: &GameplayTuningConfig,
) -> Result<()> {
    let active_factions = war_state.faction_count as usize;

    let pool = war_state.dbtc_mined_this_war;
    let sol_pool = war_state.sol_reward_pool;
    if pool == 0 && sol_pool == 0 {
        war_settlement.base_reward_pools = [0u64; NUM_FACTIONS];
        war_settlement.hashbeast_reward_pools = [0u64; NUM_FACTIONS];
        war_settlement.mvp_bonus = [0u64; NUM_FACTIONS];
        war_settlement.sol_base_pool = 0;
        war_settlement.sol_hb_pool = 0;
        war_settlement.sol_mvp_pool = 0;
        return Ok(());
    }

    let base_pool_total = pool_share_from_bps(pool, tuning.faction_war_base_reward_bps)?;
    let mvp_pool_total = pool_share_from_bps(pool, tuning.faction_war_mvp_reward_bps)?;
    let hashbeast_pool_total = pool
        .checked_sub(base_pool_total)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_sub(mvp_pool_total)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // SOL lane split — same bps as dBTC. Pool is the SOL accumulated from
    // cycle_sol_split across all rounds. Distribution to users is computed
    // at claim time by scaling each lane's pool by the user's dBTC share of
    // that lane (`user_sol_<lane> = sol_<lane>_pool * user_dbtc_<lane> /
    // total_dbtc_<lane>`), so we only need to track lane totals here.
    let sol_base_total = pool_share_from_bps(sol_pool, tuning.faction_war_base_reward_bps)?;
    let sol_mvp_total = pool_share_from_bps(sol_pool, tuning.faction_war_mvp_reward_bps)?;
    let sol_hb_total = sol_pool
        .checked_sub(sol_base_total)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_sub(sol_mvp_total)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    let mut eligible_base = [false; NUM_FACTIONS];
    let mut eligible_hashbeast = [false; NUM_FACTIONS];

    for f in 0..active_factions {
        let winning_dir = war_settlement.resolved_directions[f] as usize;
        eligible_base[f] = war_state.faction_direction_totals[f][winning_dir] > 0;
        // HB lane is gameplay-driven: faction qualifies when at least one of
        // its players landed a mutation roll during the cycle.
        eligible_hashbeast[f] = war_state.faction_mutation_score[f] > 0;
    }

    let any_hashbeast_eligible = eligible_hashbeast.iter().take(active_factions).any(|&e| e);

    // Orphan-cascade: if a sub-pool has zero globally-eligible factions, fold
    // it into the base pool instead of stranding the rewards. Applied to both
    // dBTC and SOL symmetrically so the lanes stay in sync.
    let mut effective_base_pool = base_pool_total;
    let mut effective_sol_base = sol_base_total;
    let effective_sol_hb;
    if !any_hashbeast_eligible {
        if hashbeast_pool_total > 0 {
            effective_base_pool = effective_base_pool
                .checked_add(hashbeast_pool_total)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
        if sol_hb_total > 0 {
            effective_sol_base = effective_sol_base
                .checked_add(sol_hb_total)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
        effective_sol_hb = 0;
    } else {
        effective_sol_hb = sol_hb_total;
    }

    compute_rank_weighted_pools_into(
        effective_base_pool,
        &war_settlement.final_ranks,
        &eligible_base,
        active_factions,
        &mut war_settlement.base_reward_pools,
    )?;
    if any_hashbeast_eligible {
        compute_rank_weighted_pools_into(
            hashbeast_pool_total,
            &war_settlement.final_ranks,
            &eligible_hashbeast,
            active_factions,
            &mut war_settlement.hashbeast_reward_pools,
        )?;
    } else {
        war_settlement.hashbeast_reward_pools = [0u64; NUM_FACTIONS];
    }

    war_settlement.sol_base_pool = effective_sol_base;
    war_settlement.sol_hb_pool = effective_sol_hb;
    war_settlement.sol_mvp_pool = sol_mvp_total;

    Ok(())
}

// ========================================================================================
// ============================= LIFECYCLE — ADMIN CONFIG ===================================
// ========================================================================================

pub fn initialize_war_config_internal(
    ctx: Context<InitializeFactionWarConfig>,
) -> Result<()> {
    crate::log_fn!("faction_war", "initialize_war_config_internal");

    let war_config = &mut ctx.accounts.war_config;
    war_config.bump = ctx.bumps.war_config;
    war_config.current_war_id = 1;

    let (_vault_pda, vault_bump) =
        Pubkey::find_program_address(&[FACTION_WAR_SOL_VAULT_SEED], &crate::id());
    war_config.rewards_sol_vault_bump = vault_bump;
    war_config.settle_at_lp_op_count = 0;

    let mut initial_ranks = [0u8; NUM_FACTIONS];
    for (i, rank) in initial_ranks.iter_mut().enumerate().take(NUM_FACTIONS) {
        *rank = i as u8;
    }
    war_config.prev_ranks = initial_ranks;
    war_config.reset_cycle_round_tracking();

    // Initialize dynamic mining multiplier defaults
    war_config.mining_multiplier_bps = DEFAULT_MINING_MULTIPLIER_BPS;
    war_config.multiplier_increase_bps = DEFAULT_MULTIPLIER_INCREASE_BPS;
    war_config.multiplier_decrease_bps = DEFAULT_MULTIPLIER_DECREASE_BPS;
    war_config.multiplier_min_bps = DEFAULT_MULTIPLIER_MIN_BPS;
    war_config.multiplier_max_bps = DEFAULT_MULTIPLIER_MAX_BPS;

    Ok(())
}

// ========================================================================================
// ============================= LIFECYCLE — INITIALIZE WAR ================================
// ========================================================================================

/// Initialize a new FactionWarState PDA for the current war.
/// Must be called once per war cycle before the first round's settle_round.
/// Permissionless — anyone can initialize the war state for the current war ID.
pub fn initialize_faction_war_internal(
    ctx: Context<InitializeFactionWar>,
    war_id: u64,
) -> Result<()> {
    crate::log_fn!("faction_war", "initialize_faction_war_internal");
    msg!("🪖 [initialize_faction_war] war={}", war_id);

    let war_config = &mut ctx.accounts.war_config;
    let war_state = &mut ctx.accounts.war_state;
    let tax_config = &mut ctx.accounts.tax_config;
    let global_config = &ctx.accounts.global_config;

    require!(
        war_config.current_war_id == war_id,
        ErrorCode::InvalidParameters
    );

    let faction_count = global_config.supported_factions.len() as u8;
    let start_ranks = war_config.prev_ranks;
    let unassigned = tax_config.unassigned_faction_war_treasury_amount;

    // Anchor init zeroes the account; only set non-zero fields
    war_state.bump = ctx.bumps.war_state;
    war_state.war_id = war_id;
    war_state.start_timestamp = Clock::get()?.unix_timestamp.max(0) as u64;
    war_state.stage = 0;
    war_state.faction_count = faction_count;
    war_state.treasury_reward_base_amount = unassigned;

    let war_settlement = &mut ctx.accounts.war_settlement;
    war_settlement.bump = ctx.bumps.war_settlement;
    war_settlement.war_id = war_id;
    war_settlement.final_ranks = start_ranks;
    war_settlement.resolved_directions =
        [PredictionDirection::Neutral.as_index() as u8; NUM_FACTIONS];

    tax_config.unassigned_faction_war_treasury_amount = 0;

    let lp_ops = ctx.accounts.dbtc_mining.pol_stats.lp_operations_count;
    let settle_cycle = lp_ops.checked_add(1).ok_or(ErrorCode::ArithmeticOverflow)?;
    war_config.settle_at_lp_op_count = settle_cycle;
    war_config.reset_cycle_round_tracking();

    msg!(
        "🪖 [initialize_faction_war] seeded war={} factions={} settle_after_lp={} treasury_base={}",
        war_id,
        faction_count,
        settle_cycle,
        unassigned,
    );
    Ok(())
}

// ========================================================================================
// ============================= LIFECYCLE — SETTLEMENT =====================================
// ========================================================================================

/// Internal settlement logic.  Called by `settle_war_internal`.
///
/// Computes final rankings, reward pools, MVP bonuses, and advances
/// `current_war_id` so the next cycle can begin.
pub fn finalize_war_settlement(
    war_config: &mut FactionWarConfig,
    war_state: &mut FactionWarState,
    war_settlement: &mut FactionWarSettlement,
    tax_config: &mut TaxConfig,
    tuning: &GameplayTuningConfig,
) -> Result<()> {
    let active_factions = war_state.faction_count as usize;
    msg!(
        "⚔️ [faction_war.finalize_war_settlement] war_id={} active_factions={}",
        war_state.war_id,
        active_factions
    );

    // Empty faction_war (no bets ever placed, e.g. never populated by join_bets):
    // settle with no rewards and advance current_war_id so the cycle can
    // keep moving. Without this, empty wars would block all subsequent LP burns.
    if active_factions == 0 {
        msg!(
            "⚔️ [faction_war.finalize_war_settlement] FactionWar #{} has 0 active factions — settling empty and advancing",
            war_state.war_id
        );
        if war_state.treasury_reward_base_amount > 0 {
            tax_config.unassigned_faction_war_treasury_amount = tax_config
                .unassigned_faction_war_treasury_amount
                .checked_add(war_state.treasury_reward_base_amount)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            war_state.treasury_reward_base_amount = 0;
        }
        war_state.stage = 1;
        war_state.dbtc_mined_this_war = 0;
        war_settlement.base_reward_pools = [0u64; NUM_FACTIONS];
        war_settlement.hashbeast_reward_pools = [0u64; NUM_FACTIONS];
        war_config.current_war_id = war_config.current_war_id + 1; 
        war_config.cycle_end_round_id = 0;
        msg!("✅ [faction_war.finalize_war_settlement] empty settlement done");
        return Ok(());
    }

    msg!(
        "⚔️ [faction_war.finalize_war_settlement] FactionWar #{}, {} factions, {} degenBTC mined",
        war_state.war_id,
        active_factions,
        war_state.total_dbtc_mined_in_rounds
    );

    let total_gameplay_score: u64 = war_state
        .gameplay_scores
        .iter()
        .take(active_factions)
        .sum();
    msg!(
        "⚔️ [faction_war.finalize_war_settlement] total_gameplay_score={}",
        total_gameplay_score
    );

    let has_bets = war_state
        .faction_direction_totals
        .iter()
        .take(active_factions)
        .any(|row| row.iter().any(|&v| v > 0));
    msg!(
        "⚔️ [faction_war.finalize_war_settlement] has_bets={} rpg_progression={}",
        has_bets,
        tuning.rpg_progression
    );

    if !has_bets {
        msg!(
            "⚔️ [faction_war.finalize_war_settlement] FactionWar #{} has no bets (rpg_progression={}) — rolling treasury forward and settling empty",
            war_state.war_id,
            tuning.rpg_progression
        );
        if war_state.treasury_reward_base_amount > 0 {
            tax_config.unassigned_faction_war_treasury_amount = tax_config
                .unassigned_faction_war_treasury_amount
                .checked_add(war_state.treasury_reward_base_amount)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            war_state.treasury_reward_base_amount = 0;
        }
        war_state.dbtc_mined_this_war = 0;
        war_settlement.base_reward_pools = [0u64; NUM_FACTIONS];
        war_settlement.hashbeast_reward_pools = [0u64; NUM_FACTIONS];
        war_state.stage = 1;
    } else {
        war_state.dbtc_mined_this_war = apply_mining_multiplier(
            war_state.total_dbtc_mined_in_rounds,
            war_config.mining_multiplier_bps,
        )?;

        let mut gameplay_scores_i64 = [0i64; NUM_FACTIONS];
        for (i, score) in gameplay_scores_i64
            .iter_mut()
            .enumerate()
            .take(active_factions)
        {
            *score = war_state.gameplay_scores[i] as i64;
        }
        let mut final_ranks = [0u8; NUM_FACTIONS];
        compute_rankings_into(
            &gameplay_scores_i64,
            &war_state.round_wins,
            active_factions,
            &mut final_ranks,
        )?;
        war_settlement.final_ranks = final_ranks;

        for (faction_id, _) in final_ranks.iter().enumerate().take(active_factions) {
            let (direction, rank_delta) = resolve_direction_from_ranks(
                war_config.prev_ranks[faction_id],
                final_ranks[faction_id],
            );
            war_settlement.rank_deltas[faction_id] = rank_delta;
            war_settlement.resolved_directions[faction_id] = direction.as_index() as u8;
        }

        compute_base_reward_pools(war_state, war_settlement, tuning)?;

        // --- MVP Bonus: rank-weighted across all faction MVPs ---
        // Curve: 30/20/14/10/8/6/5/4/share-3% (ALGS-style top-heavy with a
        // meaningful tail). With 12 factions: 3000+2000+1400+1000+800+600+500+400
        // bps for top 8, then 300 bps shared equally across ranks 8..N-1.
        // When fewer factions exist, the distribution loop normalizes per
        // eligible weight, so the missing tail doesn't strand pool.
        let mvp_pool_total = pool_share_from_bps(
            war_state.dbtc_mined_this_war,
            tuning.faction_war_mvp_reward_bps,
        )?;

        if mvp_pool_total > 0 {
            let mut total_weight: u128 = 0;
            let mut eligible_count = 0usize;
            for fid in 0..active_factions {
                if war_state.mvp_user[fid] != Pubkey::default() {
                    let weight_bps =
                        mvp_rank_weight_bps(final_ranks[fid] as usize, active_factions);
                    total_weight += weight_bps as u128;
                    eligible_count += 1;
                }
            }

            if eligible_count > 0 && total_weight > 0 {
                let mut distributed = 0u64;
                let mut remaining = eligible_count;
                for fid in 0..active_factions {
                    if war_state.mvp_user[fid] == Pubkey::default() {
                        continue;
                    }
                    remaining -= 1;
                    let weight_bps =
                        mvp_rank_weight_bps(final_ranks[fid] as usize, active_factions);

                    let bonus = if remaining == 0 {
                        mvp_pool_total
                            .checked_sub(distributed)
                            .ok_or(ErrorCode::ArithmeticOverflow)?
                    } else {
                        let computed_u128 = (mvp_pool_total as u128)
                            .checked_mul(weight_bps as u128)
                            .ok_or(ErrorCode::ArithmeticOverflow)?
                            .checked_div(total_weight)
                            .ok_or(ErrorCode::ArithmeticOverflow)?;
                        u64::try_from(computed_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?
                    };

                    war_settlement.mvp_bonus[fid] = bonus;
                    distributed = distributed
                        .checked_add(bonus)
                        .ok_or(ErrorCode::ArithmeticOverflow)?;

                    emit!(crate::events::FactionWarMvp {
                        war_id: war_state.war_id,
                        faction_id: fid as u8,
                        user: war_state.mvp_user[fid],
                        mvp_score: war_state.mvp_score[fid],
                        bonus_amount: bonus,
                        timestamp: Clock::get()?.unix_timestamp,
                    });
                }
            }
        }
        war_state.stage = 1;

        war_config.prev_ranks = final_ranks;
    }

    war_config.current_war_id = war_config.current_war_id + 1;
    // Clear the cycle boundary so start_round + the next war can resume.
    war_config.cycle_end_round_id = 0;

    msg!(
        "✅ [faction_war.finalize_war_settlement] FactionWar settled. Next war_id: {}",
        war_config.current_war_id
    );
    Ok(())
}

/// Fully permissionless — all ranking inputs are already on-chain.
/// Anyone can crank settlement once the economy cycle's LP burn has completed.
pub fn settle_war_internal(ctx: Context<SettleFactionWar>) -> Result<()> {
    crate::log_fn!("faction_war", "settle_war_internal");

    let war_config = &mut *ctx.accounts.war_config;
    let war_state = &mut *ctx.accounts.war_state;
    let tax_config = &mut *ctx.accounts.tax_config;
    let mining = &*ctx.accounts.dbtc_mining;
    let tuning = &ctx.accounts.global_config.gameplay_tuning;

    msg!(
        "⚔️ [faction_war.settle_war_internal] FactionWar #{}, stage={}, lp_ops={}, settle_cycle={}",
        war_state.war_id,
        war_state.stage,
        mining.pol_stats.lp_operations_count,
        war_config.settle_at_lp_op_count
    );

    require!(war_state.stage == 0, ErrorCode::FactionWarNotActive);
    require!(
        mining.pol_stats.lp_operations_count >= war_config.settle_at_lp_op_count,
        ErrorCode::FactionWarNotEnded
    );

    // Cycle-boundary invariant:
    //   1. LP burn must have snapshotted the final round of this cycle
    //      (`cycle_end_round_id != 0`).
    //   2. That final round must already be fully settled and folded into
    //      `war_state` (`last_processed_round_id == cycle_end_round_id`).
    // start_round is blocked while `cycle_end_round_id != 0`, so no new bets
    // can sneak in between the boundary round's fold and this settlement.
    require!(
        war_config.cycle_end_round_id != 0
            && war_config.last_processed_round_id
                == war_config.cycle_end_round_id,
        ErrorCode::RoundFinalizationPending
    );
    msg!(
        "✅ [faction_war.settle_war_internal] cycle-boundary check passed (cycle_end_round_id={})",
        war_config.cycle_end_round_id
    );

    let war_settlement = &mut *ctx.accounts.war_settlement;
    finalize_war_settlement(war_config, war_state, war_settlement, tax_config, tuning)?;

    let clock = Clock::get()?;
    msg!("⚔️ [faction_war.settle_war_internal] emitting FactionWarSettled: war_id={} total_degenbtc_mined={} mining_pool={} timestamp={}",
        war_state.war_id,
        war_state.total_dbtc_mined_in_rounds,
        war_state.dbtc_mined_this_war,
        clock.unix_timestamp
    );
    emit!(FactionWarSettled {
        war_id: war_state.war_id,
        total_degenbtc_mined: war_state.total_dbtc_mined_in_rounds,
        dbtc_mined_this_war: war_state.dbtc_mined_this_war,
        final_ranks: war_settlement.final_ranks,
        rank_deltas: war_settlement.rank_deltas,
        resolved_directions: war_settlement.resolved_directions,
        base_reward_pools: war_settlement.base_reward_pools,
        hashbeast_reward_pools: war_settlement.hashbeast_reward_pools,
        round_wins: war_state.round_wins,
        gameplay_scores: war_state.gameplay_scores,
        timestamp: clock.unix_timestamp,
    });

    msg!("✅ [faction_war.settle_war_internal] settlement complete");
    Ok(())
}

// ========================================================================================
// ============================= LIFECYCLE — CLAIMS =========================================
// ========================================================================================

pub fn claim_faction_war_rewards_internal(
    ctx: Context<ClaimFactionWarRewards>,
    war_id: u64,
) -> Result<()> {
    crate::log_fn!("faction_war", "claim_faction_war_rewards_internal");
    msg!(
        "⚔️ [faction_war.claim_faction_war_rewards_internal] FactionWar #{}, user={}",
        war_id,
        ctx.accounts.user_faction_war_bets.owner
    );
    let war_state = &ctx.accounts.war_state;
    let war_settlement = &ctx.accounts.war_settlement;
    let user_faction_war_bets = &ctx.accounts.user_faction_war_bets;
    let player_data_key = ctx.accounts.player_data.key();
    let player_data = &mut ctx.accounts.player_data;
    let hodl_pool = &mut ctx.accounts.hodl_pool;
    let clock = Clock::get()?;
    let owner_key = user_faction_war_bets.owner;

    require!(
        war_state.stage == 1,
        ErrorCode::FactionWarNotSettled
    );
    msg!("✅ [faction_war.claim_faction_war_rewards_internal] stage==1 check passed");
    require_keys_eq!(player_data.owner, owner_key, ErrorCode::InvalidOwner);
    msg!("✅ [faction_war.claim_faction_war_rewards_internal] owner check passed");

    let active_factions = war_state.faction_count as usize;
    let player_faction_id = player_data.faction_id as usize;
    let mut base_reward_amount = 0u64;
    let mut hashbeast_bonus_amount = 0u64;
    let mut hashbeast_mint = Pubkey::default();
    msg!("⚔️ [faction_war.claim_faction_war_rewards_internal] active_factions={} player_faction_id={}", active_factions, player_faction_id);

    if active_factions == 0 {
        msg!(
            "⚔️ [faction_war.claim_faction_war_rewards_internal] FactionWar {} settled with 0 active factions. Closing claim with 0 reward.",
            war_id
        );
    } else {

        msg!("✅ [faction_war.claim_faction_war_rewards_internal] validation skipped");

        // --- Base reward: per-faction, requires correct direction. ---
        for faction_id in 0..active_factions {
            let resolved_direction = war_settlement.resolved_directions[faction_id] as usize;
            let user_bet = user_faction_war_bets.direction_bets[faction_id][resolved_direction];
            msg!("⚔️ [faction_war.claim_faction_war_rewards_internal] loop faction_id={} resolved_direction={} user_bet={}", faction_id, resolved_direction, user_bet);
            if user_bet == 0 {
                msg!("⚔️ [faction_war.claim_faction_war_rewards_internal] skip faction_id={} (user_bet==0)", faction_id);
                continue;
            }

            let total_bet =
                war_state.faction_direction_totals[faction_id][resolved_direction];
            let faction_pool = war_settlement.base_reward_pools[faction_id];
            msg!("📊 [faction_war.claim_faction_war_rewards_internal] base calc faction_id={} total_bet={} faction_pool={}", faction_id, total_bet, faction_pool);

            if total_bet > 0 && faction_pool > 0 {
                let reward_u128 = (faction_pool as u128)
                    .checked_mul(user_bet as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?
                    .checked_div(total_bet as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
                let reward =
                    u64::try_from(reward_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;
                let old_base = base_reward_amount;
                base_reward_amount = base_reward_amount
                    .checked_add(reward)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
                msg!("📊 [faction_war.claim_faction_war_rewards_internal] base_reward: old={} add={} new={}", old_base, reward, base_reward_amount);
            } else {
                msg!("📊 [faction_war.claim_faction_war_rewards_internal] base calc skipped total_bet={} faction_pool={}", total_bet, faction_pool);
            }
        }

        // --- HB bonus: pure gameplay lane.
        // Numerator   = user's cumulative mutation_score on their home faction
        //               (incremented each time a round-claim mutation roll lands).
        // Denominator = country's total mutation_score across all HB players.
        // No mutations → no HB-bonus, regardless of how much SOL was bet. The
        // base + SOL-base lanes already pay the active-bettor case.
        if user_faction_war_bets.mutation_score > 0 && player_faction_id < active_factions {
            let hashbeast_pool = war_settlement.hashbeast_reward_pools[player_faction_id];
            let faction_mutation_total = war_state.faction_mutation_score[player_faction_id];
            let user_mutation_score = user_faction_war_bets.mutation_score;
            msg!("📊 [faction_war.claim_faction_war_rewards_internal] hashbeast calc hashbeast_pool={} faction_mutation_total={} user_mutation_score={}",
                hashbeast_pool, faction_mutation_total, user_mutation_score);
            if hashbeast_pool > 0 && faction_mutation_total > 0 {
                let bonus_u128 = (hashbeast_pool as u128)
                    .checked_mul(user_mutation_score as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?
                    .checked_div(faction_mutation_total as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
                hashbeast_bonus_amount =
                    u64::try_from(bonus_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;
                msg!("📊 [faction_war.claim_faction_war_rewards_internal] hashbeast_bonus_amount={}", hashbeast_bonus_amount);
                if hashbeast_bonus_amount > 0 {
                    hashbeast_mint = user_faction_war_bets.gameplay_hashbeast;
                    msg!("⚔️ [faction_war.claim_faction_war_rewards_internal] hashbeast_mint set={}", hashbeast_mint);
                }
            } else {
                msg!("📊 [faction_war.claim_faction_war_rewards_internal] hashbeast calc skipped");
            }
        }
    }

    let mut total_reward = base_reward_amount;
    msg!(
        "📊 [faction_war.claim_faction_war_rewards_internal] total_reward after base={}",
        total_reward
    );

    // --- MVP Bonus: if this user is any faction's MVP, add their pre-computed bonus ---
    let mut mvp_bonus_amount = 0u64;
    for fid in 0..active_factions {
        if war_state.mvp_user[fid] == owner_key {
            mvp_bonus_amount = war_settlement.mvp_bonus[fid];
            msg!("🏆 [faction_war.claim_faction_war_rewards_internal] MVP match fid={} mvp_bonus_amount={}", fid, mvp_bonus_amount);
            if mvp_bonus_amount > 0 {
                let old_total = total_reward;
                total_reward = total_reward
                    .checked_add(mvp_bonus_amount)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
                msg!(
                    "🏆 [faction_war.claim_faction_war_rewards_internal] MVP Bonus claimed: faction={} rank={} bonus={} total_reward: {} -> {}",
                    fid,
                    war_settlement.final_ranks[fid] + 1,
                    mvp_bonus_amount,
                    old_total,
                    total_reward
                );
            }
            break;
        }
    }

    // --- SOL rewards: mirror the dBTC 3-lane split (base / HB / MVP).
    // For each lane: user_sol = sol_<lane>_pool * user_dbtc_<lane> / total_dbtc_<lane>_pool.
    // Distributing SOL by the same proportions preserves identical relative
    // payouts to dBTC across the cohort while only paying out where dBTC was
    // also paid (skips orphan/zero lanes naturally).
    let scale_sol_lane = |sol_lane_pool: u64,
                          user_dbtc_lane: u64,
                          total_dbtc_lane: u64|
     -> Result<u64> {
        if sol_lane_pool == 0 || user_dbtc_lane == 0 || total_dbtc_lane == 0 {
            return Ok(0);
        }
        let s = (sol_lane_pool as u128)
            .checked_mul(user_dbtc_lane as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(total_dbtc_lane as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        u64::try_from(s).map_err(|_| error!(ErrorCode::ArithmeticOverflow))
    };

    let total_dbtc_base: u64 = war_settlement
        .base_reward_pools
        .iter()
        .take(active_factions)
        .copied()
        .try_fold(0u64, |acc, v| acc.checked_add(v))
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let total_dbtc_hb: u64 = war_settlement
        .hashbeast_reward_pools
        .iter()
        .take(active_factions)
        .copied()
        .try_fold(0u64, |acc, v| acc.checked_add(v))
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let total_dbtc_mvp: u64 = war_settlement
        .mvp_bonus
        .iter()
        .take(active_factions)
        .copied()
        .try_fold(0u64, |acc, v| acc.checked_add(v))
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    let sol_base_share =
        scale_sol_lane(war_settlement.sol_base_pool, base_reward_amount, total_dbtc_base)?;
    let sol_hb_share = scale_sol_lane(
        war_settlement.sol_hb_pool,
        hashbeast_bonus_amount,
        total_dbtc_hb,
    )?;
    let sol_mvp_share =
        scale_sol_lane(war_settlement.sol_mvp_pool, mvp_bonus_amount, total_dbtc_mvp)?;

    let sol_reward: u64 = sol_base_share
        .checked_add(sol_hb_share)
        .and_then(|s| s.checked_add(sol_mvp_share))
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    msg!(
        "💰 [faction_war.claim_faction_war_rewards_internal] sol shares: base={} hb={} mvp={} total={}",
        sol_base_share,
        sol_hb_share,
        sol_mvp_share,
        sol_reward
    );
    if sol_reward > 0 {
        msg!(
            "💰 [faction_war.claim_faction_war_rewards_internal] Transferring SOL cycle reward: {} lamports ({} SOL)",
            sol_reward,
            sol_reward as f64 / 1e9
        );
        let vault_seeds = &[
            FACTION_WAR_SOL_VAULT_SEED.as_ref(),
            &[ctx.bumps.faction_war_sol_vault],
        ];
        let signer = &[&vault_seeds[..]];
        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.faction_war_sol_vault.to_account_info(),
                    to: ctx.accounts.player.to_account_info(),
                },
                signer,
            ),
            sol_reward,
        )?;
        msg!("💰 [faction_war.claim_faction_war_rewards_internal] SOL transfer complete");
    }

    let claim_won = total_reward > 0 || sol_reward > 0 || hashbeast_bonus_amount > 0;
    let claim_mutation_type = process_faction_war_claim_hashbeast_update(
        war_id,
        owner_key,
        war_state,
        war_settlement,
        user_faction_war_bets,
        player_data,
        &ctx.accounts.global_config.gameplay_tuning,
        ctx.accounts.hashbeast_metadata.as_mut(),
        hashbeast_bonus_amount,
        claim_won,
    )?;
    if claim_mutation_type > 0
        || (claim_won && user_faction_war_bets.gameplay_hashbeast != Pubkey::default())
    {
        hashbeast_mint = user_faction_war_bets.gameplay_hashbeast;
    }

    msg!(
        "⚔️ [faction_war.claim_faction_war_rewards_internal] Player faction {}: base_reward={}, mvp_bonus={}, total_reward={}, hashbeast_bonus={}, user_mutation_score={}, sol_reward={}",
        player_faction_id,
        base_reward_amount,
        mvp_bonus_amount,
        total_reward,
        hashbeast_bonus_amount,
        user_faction_war_bets.mutation_score,
        sol_reward
    );

    if total_reward > 0 {
        msg!("⚔️ [faction_war.claim_faction_war_rewards_internal] adding to total_claimable total_reward={}", total_reward);
        helper::add_to_total_claimable(
            hodl_pool,
            player_data,
            total_reward,
            owner_key,
            player_data_key,
            CLAIMABLE_DBTC_SOURCE_FACTION_WAR,
            war_id,
        )?;
        msg!("✅ [faction_war.claim_faction_war_rewards_internal] add_to_total_claimable done");
    } else {
        msg!("⚔️ [faction_war.claim_faction_war_rewards_internal] total_reward==0, skipping add_to_total_claimable");
    }

    let old_pending = player_data.pending_faction_war_claims;
    player_data.pending_faction_war_claims = player_data
        .pending_faction_war_claims
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "⚔️ [faction_war.claim_faction_war_rewards_internal] pending_faction_war_claims: {} -> {}",
        old_pending,
        player_data.pending_faction_war_claims
    );

    // Note: lootbox rolls fire on round-claim for losing players, not on
    // cycle-claim. See `claim_round_rewards` in `user.rs`.

    msg!("⚔️ [faction_war.claim_faction_war_rewards_internal] emitting FactionWarRewardsClaimed: war_id={} user={} reward_amount={} base={} mvp={} hashbeast={} sol={} timestamp={}",
        war_id,
        user_faction_war_bets.owner,
        total_reward,
        base_reward_amount,
        mvp_bonus_amount,
        hashbeast_bonus_amount,
        sol_reward,
        clock.unix_timestamp
    );
    emit!(FactionWarRewardsClaimed {
        war_id,
        user: user_faction_war_bets.owner,
        reward_amount: total_reward,
        base_reward_amount,
        mvp_bonus_amount,
        hashbeast_bonus_amount,
        sol_reward_amount: sol_reward,
        hashbeast_mint,
        timestamp: clock.unix_timestamp,
    });

    msg!("✅ [faction_war.claim_faction_war_rewards_internal] claim complete");
    Ok(())
}

// ========================================================================================
// ============================= ACCOUNTS =================================================
// ========================================================================================

#[derive(Accounts)]
pub struct InitializeFactionWarConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = FactionWarConfig::LEN,
        seeds = [FACTION_WAR_CONFIG_SEED],
        bump,
    )]
    pub war_config: Account<'info, FactionWarConfig>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(war_id: u64)]
pub struct InitializeFactionWar<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [FACTION_WAR_CONFIG_SEED],
        bump = war_config.bump,
    )]
    pub war_config: Box<Account<'info, FactionWarConfig>>,

    #[account(
        init,
        payer = authority,
        space = FactionWarState::LEN,
        seeds = [FACTION_WAR_STATE_SEED, &war_id.to_le_bytes()],
        bump,
    )]
    pub war_state: Box<Account<'info, FactionWarState>>,

    #[account(
        init,
        payer = authority,
        space = FactionWarSettlement::LEN,
        seeds = [FACTION_WAR_SETTLEMENT_SEED, &war_id.to_le_bytes()],
        bump,
    )]
    pub war_settlement: Box<Account<'info, FactionWarSettlement>>,

    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED],
        bump = tax_config.bump
    )]
    pub tax_config: Box<Account<'info, TaxConfig>>,

    #[account(
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump = dbtc_mining.bump
    )]
    pub dbtc_mining: Box<Account<'info, DegenBtcMining>>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Fully permissionless — all ranking inputs are already on-chain.
/// Anyone can settle once the economy cycle's LP burn has completed.
#[derive(Accounts)]
pub struct SettleFactionWar<'info> {
    #[account(
        mut,
        seeds = [FACTION_WAR_CONFIG_SEED],
        bump,
    )]
    pub war_config: Box<Account<'info, FactionWarConfig>>,

    #[account(
        mut,
        seeds = [FACTION_WAR_STATE_SEED, &war_state.war_id.to_le_bytes()],
        bump,
    )]
    pub war_state: Box<Account<'info, FactionWarState>>,

    #[account(
        mut,
        seeds = [FACTION_WAR_SETTLEMENT_SEED, &war_state.war_id.to_le_bytes()],
        bump,
    )]
    pub war_settlement: Box<Account<'info, FactionWarSettlement>>,

    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED],
        bump,
    )]
    pub tax_config: Box<Account<'info, TaxConfig>>,

    #[account(
        seeds = [MINE_BTC_MINING_SEED],
        bump,
    )]
    pub dbtc_mining: Box<Account<'info, DegenBtcMining>>,

    /// Needed to read reward/evolution tuning for `finalize_war_settlement`.
    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// Anyone can settle — no authority check needed.
    pub cranker: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(war_id: u64)]
pub struct ClaimFactionWarRewards<'info> {
    #[account(
        seeds = [FACTION_WAR_STATE_SEED, &war_id.to_le_bytes()],
        bump,
    )]
    pub war_state: Box<Account<'info, FactionWarState>>,

    #[account(
        seeds = [FACTION_WAR_SETTLEMENT_SEED, &war_id.to_le_bytes()],
        bump,
    )]
    pub war_settlement: Box<Account<'info, FactionWarSettlement>>,

    #[account(
        mut,
        close = cranker,
        seeds = [USER_FACTION_WAR_BETS_SEED, user_faction_war_bets.owner.as_ref(), &war_id.to_le_bytes()],
        bump,
    )]
    pub user_faction_war_bets: Box<Account<'info, UserFactionWarBets>>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, user_faction_war_bets.owner.as_ref()],
        bump,
        constraint = player_data.owner == user_faction_war_bets.owner @ ErrorCode::InvalidOwner,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    #[account(
        mut,
        seeds = [HODL_POOL_SEED],
        bump,
    )]
    pub hodl_pool: Box<Account<'info, HodlPool>>,

    #[account(seeds = [GLOBAL_CONFIG_SEED], bump)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(mut)]
    pub hashbeast_metadata: Option<Box<Account<'info, HashBeastMetadata>>>,

    /// CHECK: Faction-war SOL vault (cycle jackpot reserve)
    #[account(
        mut,
        seeds = [FACTION_WAR_SOL_VAULT_SEED.as_ref()],
        bump,
    )]
    pub faction_war_sol_vault: UncheckedAccount<'info>,

    /// CHECK: Validated by constraint that player.key() == user_faction_war_bets.owner
    #[account(
        mut,
        constraint = player.key() == user_faction_war_bets.owner @ ErrorCode::InvalidOwner,
    )]
    pub player: AccountInfo<'info>,

    #[account(mut)]
    pub cranker: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mining_multiplier_is_hard_capped_at_three_x() {
        assert_eq!(
            apply_mining_multiplier(1_000_000, 30_000).unwrap(),
            3_000_000
        );
        assert!(apply_mining_multiplier(1_000_000, 30_001).is_err());
    }

    #[test]
    fn mining_multiplier_rejects_below_point_one_x() {
        assert_eq!(apply_mining_multiplier(1_000_000, 1_000).unwrap(), 100_000);
        assert!(apply_mining_multiplier(1_000_000, 999).is_err());
    }
}
