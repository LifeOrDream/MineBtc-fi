use anchor_lang::prelude::*;

use crate::errors::ErrorCode;
use crate::events::*;
use crate::genescience::{calculate_mutation_result, MutationType};
use crate::instructions::helper;
use crate::state::*;

fn validate_active_faction_count(active_factions: usize) -> Result<()> {
    require!(
        active_factions > 0 && active_factions <= NUM_FACTIONS,
        ErrorCode::InvalidFactionId
    );
    Ok(())
}

pub fn compute_rankings_into(
    scores: &[i64; NUM_FACTIONS],
    round_wins: &[u16; NUM_FACTIONS],
    sol_totals: &[u64; NUM_FACTIONS],
    active_factions: usize,
    ranks: &mut [u8; NUM_FACTIONS],
) -> Result<()> {
    validate_active_faction_count(active_factions)?;

    let mut ordered = [0u8; NUM_FACTIONS];
    for (idx, slot) in ordered.iter_mut().enumerate() {
        *slot = idx as u8;
    }

    ordered[..active_factions].sort_by(|a, b| {
        scores[*b as usize]
            .cmp(&scores[*a as usize])
            .then_with(|| round_wins[*b as usize].cmp(&round_wins[*a as usize]))
            .then_with(|| sol_totals[*b as usize].cmp(&sol_totals[*a as usize]))
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

fn total_cycle_sol_volume(faction_war_state: &FactionWarState, active_factions: usize) -> u64 {
    let mut total = 0u64;
    for faction_id in 0..active_factions {
        for direction in 0..PredictionDirection::COUNT {
            total = total.saturating_add(
                faction_war_state.faction_sol_direction_totals[faction_id][direction],
            );
        }
    }
    total
}

fn total_cycle_weighted_volume(faction_war_state: &FactionWarState, active_factions: usize) -> u64 {
    let mut total = 0u64;
    for faction_id in 0..active_factions {
        for direction in 0..PredictionDirection::COUNT {
            total = total
                .saturating_add(faction_war_state.faction_direction_totals[faction_id][direction]);
        }
    }
    total
}

fn user_correct_cycle_sol(
    user_faction_war_bets: &UserFactionWarBets,
    faction_war_state: &FactionWarState,
    active_factions: usize,
) -> u64 {
    let mut total = 0u64;
    for faction_id in 0..active_factions {
        let resolved = faction_war_state.resolved_directions[faction_id] as usize;
        if resolved < PredictionDirection::COUNT {
            total = total
                .saturating_add(user_faction_war_bets.sol_direction_bets[faction_id][resolved]);
        }
    }
    total
}

#[allow(clippy::too_many_arguments)]
fn process_faction_war_claim_doge_update<'info>(
    faction_war_id: u64,
    owner_key: Pubkey,
    faction_war_state: &FactionWarState,
    user_faction_war_bets: &UserFactionWarBets,
    player_data: &mut PlayerData,
    tuning: &GameplayTuningConfig,
    doge_metadata: Option<&mut Box<Account<'info, DogeMetadata>>>,
    doge_bonus_amount: u64,
    claim_won: bool,
) -> Result<u8> {
    if !claim_won
        || user_faction_war_bets.gameplay_doge == Pubkey::default()
        || user_faction_war_bets.gameplay_doge != player_data.gameplay_doge
        || player_data.gameplay_doge == Pubkey::default()
    {
        return Ok(0);
    }

    let active_factions = faction_war_state.active_faction_count as usize;
    validate_active_faction_count(active_factions)?;
    let player_faction_id = player_data.faction_id as usize;
    if player_faction_id >= active_factions {
        return Ok(0);
    }

    let resolved_direction = faction_war_state.resolved_directions[player_faction_id] as usize;
    if resolved_direction >= PredictionDirection::COUNT {
        return Ok(0);
    }

    let home_correct_sol =
        user_faction_war_bets.sol_direction_bets[player_faction_id][resolved_direction];
    let all_correct_sol =
        user_correct_cycle_sol(user_faction_war_bets, faction_war_state, active_factions);
    let mut stake = home_correct_sol;
    let mut chance_boost_bps = 0u64;

    if home_correct_sol > 0 {
        let rank_delta_abs = faction_war_state.rank_deltas[player_faction_id].unsigned_abs() as u64;
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
        let total_sol = total_cycle_sol_volume(faction_war_state, active_factions);
        let total_weighted = total_cycle_weighted_volume(faction_war_state, active_factions);
        let highest = if home_correct_sol > 0 {
            faction_war_state.faction_sol_direction_totals[player_faction_id][resolved_direction]
        } else {
            total_sol
        };
        let faction_volume = if home_correct_sol > 0 {
            faction_war_state.faction_sol_totals[player_faction_id]
        } else {
            total_sol
        };

        let mutation_result = calculate_mutation_result(
            STORY_EVENT_ORIGIN_FACTION_WAR,
            faction_war_id,
            stake,
            highest.max(stake),
            player_data.active_multiplier,
            player_data.gameplay_doge_dna,
            player_data.gameplay_doge_xp,
            tuning.max_evolution_stage_unlocked,
            0,
            faction_volume.max(stake),
            tuning,
            chance_boost_bps,
            tuning.target_rounds_per_cycle,
            0,
            0,
            total_sol.max(stake),
            total_weighted,
            total_weighted,
            faction_war_state
                .start_timestamp
                .saturating_add(faction_war_id),
            &owner_key,
            &player_data.gameplay_doge,
        );

        player_data.gameplay_doge_xp = player_data
            .gameplay_doge_xp
            .checked_add(mutation_result.xp_gained)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        if let Some(mutation_type) = mutation_result.mutation_type {
            let new_mult = player_data
                .active_multiplier
                .checked_add(mutation_result.multiplier_increase)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            player_data.active_multiplier = new_mult.min(GAMEPLAY_MAX_MULTIPLIER as u32);
            player_data.gameplay_doge_xp = player_data
                .gameplay_doge_xp
                .saturating_sub(mutation_result.xp_consumed);
            player_data.gameplay_doge_dna = mutation_result.new_dna;

            mutation_type_u8 = mutation_type_to_u8(mutation_type);
            emit!(StoryEventTriggered {
                origin: STORY_EVENT_ORIGIN_FACTION_WAR,
                origin_id: faction_war_id,
                user: owner_key,
                doge_mint: player_data.gameplay_doge,
                story_event_type: mutation_type_u8,
                xp_gained: mutation_result.xp_gained,
                multiplier_after: player_data.active_multiplier,
            });
        }
    }

    if doge_bonus_amount > 0 || stake > 0 {
        require!(
            doge_metadata.is_some()
                && doge_metadata.as_ref().unwrap().mint == user_faction_war_bets.gameplay_doge,
            ErrorCode::DogeMetadataNotFound
        );
        let doge_metadata = doge_metadata.unwrap();
        if doge_bonus_amount > 0 {
            doge_metadata.accumulated_val = doge_metadata
                .accumulated_val
                .checked_add(doge_bonus_amount)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
        doge_metadata.dna = player_data.gameplay_doge_dna;
        doge_metadata.xp = player_data.gameplay_doge_xp;
        doge_metadata.multiplier = player_data.active_multiplier;
        emit!(DogeSynced {
            doge_mint: doge_metadata.mint,
            doge_metadata_account: doge_metadata.key(),
            dna: doge_metadata.dna.to_vec(),
            xp: doge_metadata.xp,
            multiplier: doge_metadata.multiplier,
            accumulated_val: doge_metadata.accumulated_val,
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

/// Compute how the faction_war mining pool is split across factions.
///
/// The total pool is first split into four lanes:
/// - base rewards: anyone correct on a country's resolved direction
/// - loyalty rewards: only users backing their own country correctly
/// - mvp rewards: top contributor per faction (distributed at settlement by rank)
/// - Doge rewards: gameplay Doges backing the resolved home-country outcome
///
/// Each lane is then distributed across factions by final rank, normalized only
/// across factions that have eligible claimants for that lane.
pub fn compute_faction_reward_pools(
    faction_war_state: &mut FactionWarState,
    tuning: &GameplayTuningConfig,
) -> Result<()> {
    let active_factions = faction_war_state.active_faction_count as usize;
    validate_active_faction_count(active_factions)?;

    let pool = faction_war_state.faction_war_mining_pool;
    if pool == 0 {
        faction_war_state.faction_reward_pools = [0u64; NUM_FACTIONS];
        faction_war_state.loyalty_reward_pools = [0u64; NUM_FACTIONS];
        faction_war_state.faction_doge_reward_pools = [0u64; NUM_FACTIONS];
        faction_war_state.faction_mvp_bonus = [0u64; NUM_FACTIONS];
        return Ok(());
    }

    let base_pool_total = pool_share_from_bps(pool, tuning.faction_war_base_reward_bps)?;
    let loyalty_pool_total = pool_share_from_bps(pool, tuning.faction_war_loyalty_reward_bps)?;
    let mvp_pool_total = pool_share_from_bps(pool, tuning.faction_war_mvp_reward_bps)?;
    let doge_pool_total = pool
        .checked_sub(base_pool_total)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_sub(loyalty_pool_total)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_sub(mvp_pool_total)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let mut eligible_base = [false; NUM_FACTIONS];
    let mut eligible_loyalty = [false; NUM_FACTIONS];
    let mut eligible_doge = [false; NUM_FACTIONS];

    for f in 0..active_factions {
        let winning_dir = faction_war_state.resolved_directions[f] as usize;
        eligible_base[f] = faction_war_state.faction_direction_totals[f][winning_dir] > 0;
        eligible_loyalty[f] = faction_war_state.loyalty_direction_totals[f][winning_dir] > 0;
        eligible_doge[f] = faction_war_state.eligible_doge_direction_totals[f][winning_dir] > 0;
    }

    let any_loyalty_eligible = eligible_loyalty.iter().take(active_factions).any(|&e| e);
    let any_doge_eligible = eligible_doge.iter().take(active_factions).any(|&e| e);

    // Orphan-cascade: if a sub-pool has zero globally-eligible factions, fold
    // it into the base pool instead of stranding the degenBTC in the mining
    // vault. With no base eligibles either, those tokens stay in the vault
    // (extremely rare — would require zero correct bets on every faction's
    // resolved direction across the whole cycle).
    let mut effective_base_pool = base_pool_total;
    if !any_loyalty_eligible && loyalty_pool_total > 0 {
        effective_base_pool = effective_base_pool
            .checked_add(loyalty_pool_total)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    if !any_doge_eligible && doge_pool_total > 0 {
        effective_base_pool = effective_base_pool
            .checked_add(doge_pool_total)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    compute_rank_weighted_pools_into(
        effective_base_pool,
        &faction_war_state.final_ranks,
        &eligible_base,
        active_factions,
        &mut faction_war_state.faction_reward_pools,
    )?;
    if any_loyalty_eligible {
        compute_rank_weighted_pools_into(
            loyalty_pool_total,
            &faction_war_state.final_ranks,
            &eligible_loyalty,
            active_factions,
            &mut faction_war_state.loyalty_reward_pools,
        )?;
    } else {
        faction_war_state.loyalty_reward_pools = [0u64; NUM_FACTIONS];
    }
    if any_doge_eligible {
        compute_rank_weighted_pools_into(
            doge_pool_total,
            &faction_war_state.final_ranks,
            &eligible_doge,
            active_factions,
            &mut faction_war_state.faction_doge_reward_pools,
        )?;
    } else {
        faction_war_state.faction_doge_reward_pools = [0u64; NUM_FACTIONS];
    }
    Ok(())
}

// ========================================================================================
// ============================= FACTION_WAR CONFIG ==============================================
// ========================================================================================

pub fn initialize_faction_war_config_internal(
    ctx: Context<InitializeFactionWarConfig>,
) -> Result<()> {
    crate::log_fn!("faction_war", "initialize_faction_war_config_internal");
    msg!("⚔️ [faction_war.initialize_faction_war_config_internal] Initializing faction_war system");
    let faction_war_config = &mut ctx.accounts.faction_war_config;
    faction_war_config.bump = ctx.bumps.faction_war_config;
    msg!(
        "🔑 [faction_war.initialize_faction_war_config_internal] bump assigned: {} -> {}",
        faction_war_config.bump,
        ctx.bumps.faction_war_config
    );
    faction_war_config.current_faction_war_id = 1;
    msg!("⚔️ [faction_war.initialize_faction_war_config_internal] current_faction_war_id set=1");
    faction_war_config.is_active = true;
    msg!("⚔️ [faction_war.initialize_faction_war_config_internal] is_active set=true");
    faction_war_config.faction_war_settle_cycle = 0;
    msg!("⚔️ [faction_war.initialize_faction_war_config_internal] faction_war_settle_cycle set=0");

    let mut initial_ranks = [0u8; NUM_FACTIONS];
    for (i, rank) in initial_ranks.iter_mut().enumerate().take(NUM_FACTIONS) {
        *rank = i as u8;
        msg!(
            "⚔️ [faction_war.initialize_faction_war_config_internal] init rank[{}]={}",
            i,
            i
        );
    }
    faction_war_config.prev_faction_war_ranks = initial_ranks;
    msg!(
        "🏆 [faction_war.initialize_faction_war_config_internal] prev_faction_war_ranks={:?}",
        initial_ranks
    );
    let current_faction_war_id = faction_war_config.current_faction_war_id;
    faction_war_config.reset_cycle_telemetry(current_faction_war_id);
    msg!("⚔️ [faction_war.initialize_faction_war_config_internal] reset_cycle_telemetry called for faction_war_id={}", current_faction_war_id);

    // Initialize dynamic mining multiplier defaults
    faction_war_config.mining_multiplier_bps = DEFAULT_MINING_MULTIPLIER_BPS;
    msg!(
        "🎯 [faction_war.initialize_faction_war_config_internal] mining_multiplier_bps={}",
        DEFAULT_MINING_MULTIPLIER_BPS
    );
    faction_war_config.multiplier_increase_bps = DEFAULT_MULTIPLIER_INCREASE_BPS;
    msg!(
        "🎯 [faction_war.initialize_faction_war_config_internal] multiplier_increase_bps={}",
        DEFAULT_MULTIPLIER_INCREASE_BPS
    );
    faction_war_config.multiplier_decrease_bps = DEFAULT_MULTIPLIER_DECREASE_BPS;
    msg!(
        "🎯 [faction_war.initialize_faction_war_config_internal] multiplier_decrease_bps={}",
        DEFAULT_MULTIPLIER_DECREASE_BPS
    );
    faction_war_config.multiplier_min_bps = DEFAULT_MULTIPLIER_MIN_BPS;
    msg!(
        "🎯 [faction_war.initialize_faction_war_config_internal] multiplier_min_bps={}",
        DEFAULT_MULTIPLIER_MIN_BPS
    );
    faction_war_config.multiplier_max_bps = DEFAULT_MULTIPLIER_MAX_BPS;
    msg!(
        "🎯 [faction_war.initialize_faction_war_config_internal] multiplier_max_bps={}",
        DEFAULT_MULTIPLIER_MAX_BPS
    );

    msg!("✅ [faction_war.initialize_faction_war_config_internal] FactionWarConfig initialized. Starting faction_war_id: 1");
    Ok(())
}

#[derive(Accounts)]
pub struct InitializeFactionWarConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = FactionWarConfig::LEN,
        seeds = [FACTION_WAR_CONFIG_SEED],
        bump,
    )]
    pub faction_war_config: Account<'info, FactionWarConfig>,

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

pub fn update_faction_war_config_internal(
    ctx: Context<UpdateFactionWarConfig>,
    is_active: Option<bool>,
) -> Result<()> {
    crate::log_fn!("faction_war", "update_faction_war_config_internal");
    let faction_war_config = &mut ctx.accounts.faction_war_config;

    if let Some(active) = is_active {
        msg!(
            "⚔️ [faction_war.update_faction_war_config_internal] updating is_active: {} -> {}",
            faction_war_config.is_active,
            active
        );
        faction_war_config.is_active = active;
    } else {
        msg!("⚔️ [faction_war.update_faction_war_config_internal] is_active=None, no change");
    }

    msg!("✅ [faction_war.update_faction_war_config_internal] done");
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateFactionWarConfig<'info> {
    #[account(
        mut,
        seeds = [FACTION_WAR_CONFIG_SEED],
        bump = faction_war_config.bump,
    )]
    pub faction_war_config: Account<'info, FactionWarConfig>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub authority: Signer<'info>,
}

// ========================================================================================
// ============================= FACTION_WAR SETTLEMENT ==========================================
// ========================================================================================

pub fn finalize_faction_war_settlement(
    faction_war_config: &mut FactionWarConfig,
    faction_war_state: &mut FactionWarState,
    tax_config: &mut TaxConfig,
    tuning: &GameplayTuningConfig,
) -> Result<()> {
    let active_factions = faction_war_state.active_faction_count as usize;
    msg!(
        "⚔️ [faction_war.finalize_faction_war_settlement] faction_war_id={} active_factions={}",
        faction_war_state.faction_war_id,
        active_factions
    );

    // Empty faction_war (no bets ever placed, e.g. seeded by init_if_needed in
    // EndRoundFactionRewards and never populated by a subsequent join_bets):
    // settle with no rewards and advance current_faction_war_id so the cycle can
    // keep moving. Without this, validate_active_faction_count reverts and
    // every subsequent LP burn can't advance past this faction_war.
    if active_factions == 0 {
        msg!(
            "⚔️ [faction_war.finalize_faction_war_settlement] FactionWar #{} has 0 active factions — settling empty and advancing",
            faction_war_state.faction_war_id
        );
        if faction_war_state.treasury_reward_base_amount > 0 {
            let old = tax_config.unassigned_faction_war_treasury_amount;
            tax_config.unassigned_faction_war_treasury_amount = tax_config
                .unassigned_faction_war_treasury_amount
                .checked_add(faction_war_state.treasury_reward_base_amount)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!(
                "💰 [faction_war.finalize_faction_war_settlement] Rolled forward treasury tax: old={} add={} new={}",
                old,
                faction_war_state.treasury_reward_base_amount,
                tax_config.unassigned_faction_war_treasury_amount
            );
            faction_war_state.treasury_reward_base_amount = 0;
            msg!("⚔️ [faction_war.finalize_faction_war_settlement] treasury_reward_base_amount reset=0");
        }
        faction_war_state.stage = 1;
        msg!("⚔️ [faction_war.finalize_faction_war_settlement] stage mutated: -> 1");
        faction_war_state.faction_war_mining_pool = 0;
        msg!("⚔️ [faction_war.finalize_faction_war_settlement] faction_war_mining_pool reset=0");
        faction_war_state.faction_reward_pools = [0u64; NUM_FACTIONS];
        faction_war_state.loyalty_reward_pools = [0u64; NUM_FACTIONS];
        faction_war_state.faction_doge_reward_pools = [0u64; NUM_FACTIONS];
        msg!("⚔️ [faction_war.finalize_faction_war_settlement] reward pools zeroed");
        let old_id = faction_war_config.current_faction_war_id;
        faction_war_config.current_faction_war_id = faction_war_config
            .current_faction_war_id
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "⚔️ [faction_war.finalize_faction_war_settlement] current_faction_war_id: {} -> {}",
            old_id,
            faction_war_config.current_faction_war_id
        );
        msg!("✅ [faction_war.finalize_faction_war_settlement] empty settlement done");
        return Ok(());
    }

    validate_active_faction_count(active_factions)?;

    msg!(
        "⚔️ [faction_war.finalize_faction_war_settlement] FactionWar #{}, {} factions, {} degenBTC mined",
        faction_war_state.faction_war_id,
        active_factions,
        faction_war_state.total_degenbtc_mined_in_faction_war
    );

    let total_gameplay_score: u64 = faction_war_state
        .faction_gameplay_scores
        .iter()
        .take(active_factions)
        .sum();
    msg!(
        "⚔️ [faction_war.finalize_faction_war_settlement] total_gameplay_score={}",
        total_gameplay_score
    );

    let has_bets = faction_war_state
        .faction_direction_totals
        .iter()
        .take(active_factions)
        .any(|row| row.iter().any(|&v| v > 0));
    msg!(
        "⚔️ [faction_war.finalize_faction_war_settlement] has_bets={} rpg_progression={}",
        has_bets,
        tuning.rpg_progression
    );

    if !has_bets || !tuning.rpg_progression {
        msg!(
            "⚔️ [faction_war.finalize_faction_war_settlement] FactionWar #{} non-operational (has_bets={}, rpg_progression={}) — rolling treasury forward and settling empty",
            faction_war_state.faction_war_id,
            has_bets,
            tuning.rpg_progression
        );
        if faction_war_state.treasury_reward_base_amount > 0 {
            let old = tax_config.unassigned_faction_war_treasury_amount;
            tax_config.unassigned_faction_war_treasury_amount = tax_config
                .unassigned_faction_war_treasury_amount
                .checked_add(faction_war_state.treasury_reward_base_amount)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!(
                "💰 [faction_war.finalize_faction_war_settlement] Rolled forward treasury tax: old={} add={} new={}",
                old,
                faction_war_state.treasury_reward_base_amount,
                tax_config.unassigned_faction_war_treasury_amount
            );
            faction_war_state.treasury_reward_base_amount = 0;
            msg!("⚔️ [faction_war.finalize_faction_war_settlement] treasury_reward_base_amount reset=0");
        }
        faction_war_state.faction_war_mining_pool = 0;
        msg!("⚔️ [faction_war.finalize_faction_war_settlement] faction_war_mining_pool reset=0");
        faction_war_state.faction_reward_pools = [0u64; NUM_FACTIONS];
        faction_war_state.loyalty_reward_pools = [0u64; NUM_FACTIONS];
        faction_war_state.faction_doge_reward_pools = [0u64; NUM_FACTIONS];
        msg!("⚔️ [faction_war.finalize_faction_war_settlement] reward pools zeroed");
        faction_war_state.stage = 1;
        msg!("⚔️ [faction_war.finalize_faction_war_settlement] stage mutated: -> 1");
    } else {
        let old_pool = faction_war_state.faction_war_mining_pool;
        faction_war_state.faction_war_mining_pool = apply_mining_multiplier(
            faction_war_state.total_degenbtc_mined_in_faction_war,
            faction_war_config.mining_multiplier_bps,
        )?;
        let _ = old_pool;

        let mut gameplay_scores_i64 = [0i64; NUM_FACTIONS];
        for (i, score) in gameplay_scores_i64
            .iter_mut()
            .enumerate()
            .take(active_factions)
        {
            *score = faction_war_state.faction_gameplay_scores[i] as i64;
        }
        let mut final_ranks = faction_war_state.final_ranks;
        compute_rankings_into(
            &gameplay_scores_i64,
            &faction_war_state.faction_round_wins,
            &faction_war_state.faction_sol_totals,
            active_factions,
            &mut final_ranks,
        )?;
        faction_war_state.final_ranks = final_ranks;

        for (faction_id, _) in final_ranks.iter().enumerate().take(active_factions) {
            let (direction, rank_delta) = resolve_direction_from_ranks(
                faction_war_state.start_ranks[faction_id],
                faction_war_state.final_ranks[faction_id],
            );
            faction_war_state.rank_deltas[faction_id] = rank_delta;
            faction_war_state.resolved_directions[faction_id] = direction.as_index() as u8;
        }

        compute_faction_reward_pools(faction_war_state, tuning)?;

        // --- MVP Bonus: distribute 5% of mining pool rank-weighted to all faction MVPs ---
        // #1 faction MVP: 40% of MVP pool | #2: 25% | #3: 15% | #4+: equal share of 20%
        let mvp_pool_total = pool_share_from_bps(
            faction_war_state.faction_war_mining_pool,
            tuning.faction_war_mvp_reward_bps,
        )?;

        if mvp_pool_total > 0 {
            let mut total_weight: u128 = 0;
            let mut eligible_count = 0usize;
            for fid in 0..active_factions {
                if faction_war_state.faction_mvp_user[fid] != Pubkey::default() {
                    let rank = faction_war_state.final_ranks[fid] as usize;
                    let weight_bps = match rank {
                        0 => 4000u64, // #1
                        1 => 2500u64, // #2
                        2 => 1500u64, // #3
                        _ => {
                            let lower_ranked_count =
                                active_factions.saturating_sub(3).max(1) as u64;
                            2000u64 / lower_ranked_count
                        }
                    };
                    total_weight += weight_bps as u128;
                    eligible_count += 1;
                }
            }

            if eligible_count > 0 && total_weight > 0 {
                let mut distributed = 0u64;
                let mut remaining = eligible_count;
                for fid in 0..active_factions {
                    if faction_war_state.faction_mvp_user[fid] == Pubkey::default() {
                        continue;
                    }
                    remaining -= 1;
                    let rank = faction_war_state.final_ranks[fid] as usize;
                    let weight_bps = match rank {
                        0 => 4000u64,
                        1 => 2500u64,
                        2 => 1500u64,
                        _ => {
                            let lower_ranked_count =
                                active_factions.saturating_sub(3).max(1) as u64;
                            2000u64 / lower_ranked_count
                        }
                    };

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

                    faction_war_state.faction_mvp_bonus[fid] = bonus;
                    distributed = distributed
                        .checked_add(bonus)
                        .ok_or(ErrorCode::ArithmeticOverflow)?;

                    emit!(crate::events::FactionWarMvp {
                        faction_war_id: faction_war_state.faction_war_id,
                        faction_id: fid as u8,
                        user: faction_war_state.faction_mvp_user[fid],
                        mvp_score: faction_war_state.faction_mvp_score[fid],
                        bonus_amount: bonus,
                        timestamp: Clock::get()?.unix_timestamp,
                    });
                }
            }
        }
        faction_war_state.stage = 1;

        faction_war_config.prev_faction_war_ranks = final_ranks;
    }

    let old_id = faction_war_config.current_faction_war_id;
    faction_war_config.current_faction_war_id = faction_war_config
        .current_faction_war_id
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "⚔️ [faction_war.finalize_faction_war_settlement] current_faction_war_id: {} -> {}",
        old_id,
        faction_war_config.current_faction_war_id
    );

    msg!(
        "✅ [faction_war.finalize_faction_war_settlement] FactionWar settled. Next faction_war_id: {}",
        faction_war_config.current_faction_war_id
    );
    Ok(())
}

pub fn settle_faction_war_internal(ctx: Context<SettleFactionWar>) -> Result<()> {
    crate::log_fn!("faction_war", "settle_faction_war_internal");
    msg!("⚔️ [faction_war.settle_faction_war_internal] Manual settlement crank");
    let faction_war_config = &mut *ctx.accounts.faction_war_config;
    let faction_war_state = &mut *ctx.accounts.faction_war_state;
    let tax_config = &mut *ctx.accounts.tax_config;
    let mining = &*ctx.accounts.mine_btc_mining;
    let tuning = &ctx.accounts.global_config.gameplay_tuning;

    msg!(
        "⚔️ [faction_war.settle_faction_war_internal] FactionWar #{}, stage={}, lp_ops={}, settle_cycle={}",
        faction_war_state.faction_war_id,
        faction_war_state.stage,
        mining.pol_stats.lp_operations_count,
        faction_war_config.faction_war_settle_cycle
    );

    require!(faction_war_state.stage == 0, ErrorCode::FactionWarNotActive);
    msg!("✅ [faction_war.settle_faction_war_internal] stage==0 check passed");
    require!(
        mining.pol_stats.lp_operations_count >= faction_war_config.faction_war_settle_cycle,
        ErrorCode::FactionWarNotEnded
    );
    msg!("✅ [faction_war.settle_faction_war_internal] lp_operations_count >= settle_cycle check passed");

    // Block external settlement while a round is mid-finalization (stage=1,
    // between end_round and end_round_faction_rewards). Otherwise this crank
    // can advance current_faction_war_id under end_round_faction_rewards'
    // feet, which is the exact brick the #35 init_if_needed patch mitigates
    // after-the-fact. Blocking the race at the source keeps
    // end_round_faction_rewards's auto-settle (the clean path, runs after
    // this round's mining has been tracked) as the only way the id advances
    // while a round is in play.
    require!(
        ctx.accounts.game_session.stage != 1,
        ErrorCode::RoundFinalizationPending
    );
    msg!("✅ [faction_war.settle_faction_war_internal] game_session.stage != 1 check passed");

    finalize_faction_war_settlement(faction_war_config, faction_war_state, tax_config, tuning)?;

    let clock = Clock::get()?;
    msg!("⚔️ [faction_war.settle_faction_war_internal] emitting FactionWarSettled: faction_war_id={} total_degenbtc_mined={} mining_pool={} timestamp={}",
        faction_war_state.faction_war_id,
        faction_war_state.total_degenbtc_mined_in_faction_war,
        faction_war_state.faction_war_mining_pool,
        clock.unix_timestamp
    );
    emit!(FactionWarSettled {
        faction_war_id: faction_war_state.faction_war_id,
        total_degenbtc_mined: faction_war_state.total_degenbtc_mined_in_faction_war,
        faction_war_mining_pool: faction_war_state.faction_war_mining_pool,
        start_ranks: faction_war_state.start_ranks,
        final_ranks: faction_war_state.final_ranks,
        rank_deltas: faction_war_state.rank_deltas,
        resolved_directions: faction_war_state.resolved_directions,
        faction_reward_pools: faction_war_state.faction_reward_pools,
        loyalty_reward_pools: faction_war_state.loyalty_reward_pools,
        faction_doge_reward_pools: faction_war_state.faction_doge_reward_pools,
        faction_round_wins: faction_war_state.faction_round_wins,
        faction_sol_totals: faction_war_state.faction_sol_totals,
        faction_gameplay_scores: faction_war_state.faction_gameplay_scores,
        timestamp: clock.unix_timestamp,
    });

    msg!("✅ [faction_war.settle_faction_war_internal] settlement complete");
    Ok(())
}

/// Fully permissionless -- all ranking inputs are already on-chain.
/// Anyone can crank settlement once the economy cycle's LP burn has completed.
#[derive(Accounts)]
pub struct SettleFactionWar<'info> {
    #[account(
        mut,
        seeds = [FACTION_WAR_CONFIG_SEED],
        bump,
    )]
    pub faction_war_config: Box<Account<'info, FactionWarConfig>>,

    #[account(
        mut,
        seeds = [FACTION_WAR_STATE_SEED, &faction_war_state.faction_war_id.to_le_bytes()],
        bump,
    )]
    pub faction_war_state: Box<Account<'info, FactionWarState>>,

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
    pub mine_btc_mining: Box<Account<'info, MineBtcMining>>,

    /// Needed to read `rpg_progression` for the no-mutation branch of
    /// `finalize_faction_war_settlement`.
    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// Needed to derive the current round's game_session PDA so the
    /// stage=1 guard below can see it.
    #[account(
        seeds = [GLOBAL_GAME_STATE_SEED],
        bump,
    )]
    pub global_game_state: Box<Account<'info, GlobalGameSate>>,

    /// Game session for the current round. Used to block this crank while
    /// stage=1 (the end_round → end_round_faction_rewards window).
    #[account(
        seeds = [GAME_SESSION_SEED, &global_game_state.current_round_id.to_le_bytes()],
        bump,
    )]
    pub game_session: Box<Account<'info, GameSession>>,

    /// Anyone can settle -- no authority check needed.
    pub cranker: Signer<'info>,
}

// ========================================================================================
// ============================= FACTION_WAR CLAIM ===============================================
// ========================================================================================

pub fn claim_faction_war_rewards_internal(
    ctx: Context<ClaimFactionWarRewards>,
    faction_war_id: u64,
) -> Result<()> {
    crate::log_fn!("faction_war", "claim_faction_war_rewards_internal");
    msg!(
        "⚔️ [faction_war.claim_faction_war_rewards_internal] FactionWar #{}, user={}",
        faction_war_id,
        ctx.accounts.user_faction_war_bets.owner
    );
    let faction_war_state = &ctx.accounts.faction_war_state;
    let user_faction_war_bets = &ctx.accounts.user_faction_war_bets;
    let player_data_key = ctx.accounts.player_data.key();
    let player_data = &mut ctx.accounts.player_data;
    let hodl_pool = &mut ctx.accounts.hodl_pool;
    let clock = Clock::get()?;
    let owner_key = user_faction_war_bets.owner;

    require!(
        faction_war_state.stage == 1,
        ErrorCode::FactionWarNotSettled
    );
    msg!("✅ [faction_war.claim_faction_war_rewards_internal] stage==1 check passed");
    require_keys_eq!(player_data.owner, owner_key, ErrorCode::InvalidOwner);
    msg!("✅ [faction_war.claim_faction_war_rewards_internal] owner check passed");

    let active_factions = faction_war_state.active_faction_count as usize;
    let player_faction_id = player_data.faction_id as usize;
    let mut base_reward_amount = 0u64;
    let mut loyalty_reward_amount = 0u64;
    let mut doge_bonus_amount = 0u64;
    let mut doge_mint = Pubkey::default();
    msg!("⚔️ [faction_war.claim_faction_war_rewards_internal] active_factions={} player_faction_id={}", active_factions, player_faction_id);

    if active_factions == 0 {
        msg!(
            "⚔️ [faction_war.claim_faction_war_rewards_internal] FactionWar {} settled with 0 active factions. Closing claim with 0 reward.",
            faction_war_id
        );
    } else {
        validate_active_faction_count(active_factions)?;
        msg!("✅ [faction_war.claim_faction_war_rewards_internal] validate_active_faction_count passed");

        for faction_id in 0..active_factions {
            let resolved_direction = faction_war_state.resolved_directions[faction_id] as usize;
            let user_bet = user_faction_war_bets.direction_bets[faction_id][resolved_direction];
            msg!("⚔️ [faction_war.claim_faction_war_rewards_internal] loop faction_id={} resolved_direction={} user_bet={}", faction_id, resolved_direction, user_bet);
            if user_bet == 0 {
                msg!("⚔️ [faction_war.claim_faction_war_rewards_internal] skip faction_id={} (user_bet==0)", faction_id);
                continue;
            }

            let total_bet =
                faction_war_state.faction_direction_totals[faction_id][resolved_direction];
            let faction_pool = faction_war_state.faction_reward_pools[faction_id];
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

            if faction_id == player_faction_id {
                msg!("⚔️ [faction_war.claim_faction_war_rewards_internal] loyalty/doge branch faction_id==player_faction_id={}", player_faction_id);
                let loyalty_total =
                    faction_war_state.loyalty_direction_totals[faction_id][resolved_direction];
                let loyalty_pool = faction_war_state.loyalty_reward_pools[faction_id];
                msg!("📊 [faction_war.claim_faction_war_rewards_internal] loyalty calc loyalty_total={} loyalty_pool={}", loyalty_total, loyalty_pool);
                if loyalty_total > 0 && loyalty_pool > 0 {
                    let reward_u128 = (loyalty_pool as u128)
                        .checked_mul(user_bet as u128)
                        .ok_or(ErrorCode::ArithmeticOverflow)?
                        .checked_div(loyalty_total as u128)
                        .ok_or(ErrorCode::ArithmeticOverflow)?;
                    let reward =
                        u64::try_from(reward_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;
                    let old_loyalty = loyalty_reward_amount;
                    loyalty_reward_amount = loyalty_reward_amount
                        .checked_add(reward)
                        .ok_or(ErrorCode::ArithmeticOverflow)?;
                    msg!("📊 [faction_war.claim_faction_war_rewards_internal] loyalty_reward: old={} add={} new={}", old_loyalty, reward, loyalty_reward_amount);
                } else {
                    msg!(
                        "📊 [faction_war.claim_faction_war_rewards_internal] loyalty calc skipped"
                    );
                }

                msg!(
                    "⚔️ [faction_war.claim_faction_war_rewards_internal] doge_bonus_eligible={}",
                    user_faction_war_bets.doge_bonus_eligible
                );
                if user_faction_war_bets.doge_bonus_eligible {
                    let doge_pool = faction_war_state.faction_doge_reward_pools[faction_id];
                    let eligible_total = faction_war_state.eligible_doge_direction_totals
                        [faction_id][resolved_direction];
                    msg!("📊 [faction_war.claim_faction_war_rewards_internal] doge calc doge_pool={} eligible_total={}", doge_pool, eligible_total);
                    if doge_pool > 0 && eligible_total > 0 {
                        let bonus_u128 = (doge_pool as u128)
                            .checked_mul(user_bet as u128)
                            .ok_or(ErrorCode::ArithmeticOverflow)?
                            .checked_div(eligible_total as u128)
                            .ok_or(ErrorCode::ArithmeticOverflow)?;
                        doge_bonus_amount =
                            u64::try_from(bonus_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;
                        msg!("📊 [faction_war.claim_faction_war_rewards_internal] doge_bonus_amount={}", doge_bonus_amount);

                        if doge_bonus_amount > 0 {
                            doge_mint = user_faction_war_bets.gameplay_doge;
                            msg!("⚔️ [faction_war.claim_faction_war_rewards_internal] doge_mint set={}", doge_mint);
                        }
                    } else {
                        msg!(
                            "📊 [faction_war.claim_faction_war_rewards_internal] doge calc skipped"
                        );
                    }
                }
            }
        }
    }

    let mut total_reward = base_reward_amount
        .checked_add(loyalty_reward_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "📊 [faction_war.claim_faction_war_rewards_internal] total_reward after base+loyalty={}",
        total_reward
    );

    // --- MVP Bonus: if this user is any faction's MVP, add their pre-computed bonus ---
    let mut mvp_bonus_amount = 0u64;
    for fid in 0..active_factions {
        if faction_war_state.faction_mvp_user[fid] == owner_key {
            mvp_bonus_amount = faction_war_state.faction_mvp_bonus[fid];
            msg!("🏆 [faction_war.claim_faction_war_rewards_internal] MVP match fid={} mvp_bonus_amount={}", fid, mvp_bonus_amount);
            if mvp_bonus_amount > 0 {
                let old_total = total_reward;
                total_reward = total_reward
                    .checked_add(mvp_bonus_amount)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
                msg!(
                    "🏆 [faction_war.claim_faction_war_rewards_internal] MVP Bonus claimed: faction={} rank={} bonus={} total_reward: {} -> {}",
                    fid,
                    faction_war_state.final_ranks[fid] + 1,
                    mvp_bonus_amount,
                    old_total,
                    total_reward
                );
            }
            break;
        }
    }

    // --- SOL Cycle Jackpot: proportional to base degenBTC reward share ---
    let mut sol_reward: u64 = 0;
    let sol_pool = faction_war_state.sol_reward_pool;
    msg!(
        "💰 [faction_war.claim_faction_war_rewards_internal] sol_pool={} base_reward_amount={}",
        sol_pool,
        base_reward_amount
    );
    if sol_pool > 0 && base_reward_amount > 0 {
        let total_base_pool = faction_war_state
            .faction_reward_pools
            .iter()
            .take(active_factions)
            .sum::<u64>();
        msg!(
            "📊 [faction_war.claim_faction_war_rewards_internal] total_base_pool={}",
            total_base_pool
        );
        if total_base_pool > 0 {
            let sol_u128 = (sol_pool as u128)
                .checked_mul(base_reward_amount as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(total_base_pool as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            sol_reward =
                u64::try_from(sol_u128).map_err(|_| error!(ErrorCode::ArithmeticOverflow))?;
            msg!(
                "💰 [faction_war.claim_faction_war_rewards_internal] sol_reward computed={}",
                sol_reward
            );
        } else {
            msg!("📊 [faction_war.claim_faction_war_rewards_internal] total_base_pool==0, sol_reward stays 0");
        }
    }
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

    let claim_won = total_reward > 0 || sol_reward > 0 || doge_bonus_amount > 0;
    let claim_mutation_type = process_faction_war_claim_doge_update(
        faction_war_id,
        owner_key,
        faction_war_state,
        user_faction_war_bets,
        player_data,
        &ctx.accounts.global_config.gameplay_tuning,
        ctx.accounts.doge_metadata.as_mut(),
        doge_bonus_amount,
        claim_won,
    )?;
    if claim_mutation_type > 0
        || (claim_won && user_faction_war_bets.gameplay_doge != Pubkey::default())
    {
        doge_mint = user_faction_war_bets.gameplay_doge;
    }

    msg!(
        "⚔️ [faction_war.claim_faction_war_rewards_internal] Player faction {}: base_reward={}, loyalty_reward={}, mvp_bonus={}, total_reward={}, doge_bonus={}, doge_eligible={}, sol_reward={}",
        player_faction_id,
        base_reward_amount,
        loyalty_reward_amount,
        mvp_bonus_amount,
        total_reward,
        doge_bonus_amount,
        user_faction_war_bets.doge_bonus_eligible,
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
            CLAIMABLE_MINEBTC_SOURCE_FACTION_WAR,
            faction_war_id,
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

    // ---- Lootbox eligibility flag ----
    // A claim is eligible for a lootbox roll if the player won at least
    // something on this cycle AND has an active gameplay Doge. We don't
    // gate on inventory pool depth here — the cranker re-checks it in
    // `process_lootbox_drops` at roll time. This avoids passing
    // `inventory_pool` into every claim.
    if claim_won
        && user_faction_war_bets.gameplay_doge != Pubkey::default()
        && player_data.pending_lootbox_roll.is_none()
    {
        // Build a deterministic per-cycle seed from settlement-stable fields.
        // The cranker can recompute this off-chain to verify; on-chain we
        // re-roll in `process_lootbox_drops` using slot-hash entropy mixed
        // with this seed and the player's pubkey.
        let cycle_seed = anchor_lang::solana_program::keccak::hashv(&[
            b"minebtc-lootbox-cycle-seed",
            &faction_war_id.to_le_bytes(),
            &faction_war_state.start_timestamp.to_le_bytes(),
            &faction_war_state.faction_war_mining_pool.to_le_bytes(),
            &faction_war_state.final_ranks,
        ])
        .to_bytes();

        player_data.pending_lootbox_roll = Some(LootboxRollClaim {
            faction_war_id,
            faction_id: player_data.faction_id,
            cycle_seed,
        });
        msg!(
            "🎁 [claim_faction_war_rewards_internal] Lootbox roll queued for {} (faction={})",
            owner_key,
            player_data.faction_id
        );
    } else {
        msg!(
            "🎁 [claim_faction_war_rewards_internal] No lootbox roll: claim_won={}, gameplay_doge_set={}, roll_already_pending={}",
            claim_won,
            user_faction_war_bets.gameplay_doge != Pubkey::default(),
            player_data.pending_lootbox_roll.is_some()
        );
    }

    msg!("⚔️ [faction_war.claim_faction_war_rewards_internal] emitting FactionWarRewardsClaimed: faction_war_id={} user={} reward_amount={} base={} loyalty={} mvp={} doge={} sol={} timestamp={}",
        faction_war_id,
        user_faction_war_bets.owner,
        total_reward,
        base_reward_amount,
        loyalty_reward_amount,
        mvp_bonus_amount,
        doge_bonus_amount,
        sol_reward,
        clock.unix_timestamp
    );
    emit!(FactionWarRewardsClaimed {
        faction_war_id,
        user: user_faction_war_bets.owner,
        reward_amount: total_reward,
        base_reward_amount,
        loyalty_reward_amount,
        mvp_bonus_amount,
        doge_bonus_amount,
        sol_reward_amount: sol_reward,
        doge_mint,
        timestamp: clock.unix_timestamp,
    });

    msg!("✅ [faction_war.claim_faction_war_rewards_internal] claim complete");
    Ok(())
}

#[derive(Accounts)]
#[instruction(faction_war_id: u64)]
pub struct ClaimFactionWarRewards<'info> {
    #[account(
        seeds = [FACTION_WAR_STATE_SEED, &faction_war_id.to_le_bytes()],
        bump,
    )]
    pub faction_war_state: Box<Account<'info, FactionWarState>>,

    #[account(
        mut,
        close = cranker,
        seeds = [USER_FACTION_WAR_BETS_SEED, user_faction_war_bets.owner.as_ref(), &faction_war_id.to_le_bytes()],
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
    pub doge_metadata: Option<Box<Account<'info, DogeMetadata>>>,

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

// ========================================================================================
// ============================== LOOTBOX DROP RESOLUTION =================================
// ========================================================================================

/// Cranker-driven lootbox drop. The cranker picks a candidate Doge from the
/// inventory pool (faction-matched, status=Lootbox) for a player whose claim
/// queued a `pending_lootbox_roll`. This handler runs the actual win/miss
/// roll on-chain so the cranker can't bias outcomes.
pub fn process_lootbox_drops_internal(ctx: Context<ProcessLootboxDrops>) -> Result<()> {
    crate::log_fn!("faction_war", "process_lootbox_drops_internal");

    let now = Clock::get()?.unix_timestamp;
    let asset_key = ctx.accounts.doge_asset.key();
    let winner_key = ctx.accounts.winner_wallet.key();

    require_keys_eq!(
        ctx.accounts.crank_authority.key(),
        ctx.accounts.inventory_pool.crank_authority,
        ErrorCode::InvalidCrankAuthority
    );

    require!(
        ctx.accounts.inventory_pool.lootbox_count >= MIN_LOOTBOX_POOL,
        ErrorCode::LootboxPoolTooSmall
    );

    let roll = ctx
        .accounts
        .winner_player_data
        .pending_lootbox_roll
        .ok_or(ErrorCode::NoLootboxRoll)?;

    let entry = &ctx.accounts.recycled_entry;
    require!(
        entry.status == RecycledStatus::Lootbox as u8,
        ErrorCode::InvalidRecycledStatus
    );
    require!(
        entry.faction_id == roll.faction_id,
        ErrorCode::LootboxFactionMismatch
    );

    let last_drop = ctx.accounts.winner_player_data.last_lootbox_drop_at;
    require!(
        last_drop == 0 || now.saturating_sub(last_drop) >= LOOTBOX_COOLDOWN_SECONDS,
        ErrorCode::LootboxCooldownActive
    );

    let metrics = &ctx.accounts.market_metrics;
    let threshold_bps = compute_drop_chance_bps(
        metrics.demand_index,
        ctx.accounts.inventory_pool.lootbox_count,
        entry.quality_score,
    );

    // On-chain entropy: mix the most recent slot hash with cycle seed and
    // per-roll context so the cranker can't pre-compute outcomes.
    let slot_hashes = ctx.accounts.slot_hashes.to_account_info();
    let slot_hashes_data = slot_hashes.try_borrow_data()?;
    require!(
        slot_hashes_data.len() >= 8 + 8 + 32,
        ErrorCode::InvalidAccount
    );
    let mut latest_slot_hash = [0u8; 32];
    latest_slot_hash.copy_from_slice(&slot_hashes_data[16..48]);
    drop(slot_hashes_data);

    let entropy = anchor_lang::solana_program::keccak::hashv(&[
        b"minebtc-lootbox-roll",
        &latest_slot_hash,
        &winner_key.to_bytes(),
        &roll.cycle_seed,
        &asset_key.to_bytes(),
    ])
    .to_bytes();
    let roll_value = u16::from_le_bytes([entropy[0], entropy[1]]) % 10_000;

    msg!(
        "🎲 [process_lootbox_drops] roll={} threshold={} (DI={} pool_lootbox={} quality={})",
        roll_value,
        threshold_bps,
        metrics.demand_index,
        ctx.accounts.inventory_pool.lootbox_count,
        entry.quality_score
    );

    let winner_won = roll_value < threshold_bps;

    if winner_won {
        // Recipient cap, approximated from staked_doges + active gameplay
        // doge. Tighter eligibility filtering happens off-chain in the
        // cranker which can scan the wallet directly.
        let approx_holdings = ctx
            .accounts
            .winner_player_data
            .staked_doges
            .len()
            .saturating_add(
                if ctx.accounts.winner_player_data.gameplay_doge != Pubkey::default() {
                    1
                } else {
                    0
                },
            );
        require!(
            approx_holdings < MAX_DOGES_PER_WALLET_FOR_DROP as usize,
            ErrorCode::LootboxRecipientCapped
        );

        // Transfer asset from inventory_pda → winner_wallet, signed by the
        // inventory PDA seeds.
        let pool_bump = ctx.accounts.inventory_pool.bump;
        let inventory_seeds_inner: &[&[u8]] = &[INVENTORY_POOL_SEED, &[pool_bump]];
        let inventory_signers: &[&[&[u8]]] = &[inventory_seeds_inner];
        crate::mpl_core_helpers::transfer_mpl_core_asset(
            &ctx.accounts.doge_asset.to_account_info(),
            ctx.accounts
                .doge_collection
                .as_ref()
                .map(|c| c.to_account_info())
                .as_ref(),
            &ctx.accounts.crank_authority.to_account_info(),
            &ctx.accounts.inventory_pda.to_account_info(),
            &ctx.accounts.winner_wallet.to_account_info(),
            &ctx.accounts.mpl_core_program.to_account_info(),
            Some(inventory_signers),
        )?;

        // Reset DogeMetadata for the new owner.
        let metadata = &mut ctx.accounts.doge_metadata;
        metadata.accumulated_val = 0;
        metadata.multiplier = BASE_MULTIPLIER;
        metadata.xp = 0;
        metadata.incubated_player_data = Pubkey::default();
        // dna, mom, dad, breed_count, faction_id, last_update_ts preserved.

        // Bump pool counters and close the entry by zeroing out so the
        // caller can reclaim its rent in a follow-up tx (we do NOT use
        // Anchor `close` here because miss-path leaves the entry alive).
        let pool = &mut ctx.accounts.inventory_pool;
        pool.lootbox_count = pool
            .lootbox_count
            .checked_sub(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        pool.total_count = pool
            .total_count
            .checked_sub(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        pool.total_dropped = pool
            .total_dropped
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        // Manually close the recycled entry, refunding rent to the winner.
        let entry_info = ctx.accounts.recycled_entry.to_account_info();
        let entry_lamports = entry_info.lamports();
        **ctx
            .accounts
            .winner_wallet
            .to_account_info()
            .try_borrow_mut_lamports()? = ctx
            .accounts
            .winner_wallet
            .lamports()
            .checked_add(entry_lamports)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        **entry_info.try_borrow_mut_lamports()? = 0;
        let mut entry_data = entry_info.try_borrow_mut_data()?;
        for byte in entry_data.iter_mut() {
            *byte = 0;
        }
        // Re-write the discriminator to a closed-account value so
        // anchor never deserializes this account again at this address
        // until reused. Anchor's standard "closed account discriminator"
        // is `[255; 8]`.
        entry_data[..8].copy_from_slice(&[255u8; 8]);
        drop(entry_data);

        // Mark winner cooldown.
        ctx.accounts.winner_player_data.last_lootbox_drop_at = now;

        emit!(LootboxNftWon {
            asset: asset_key,
            winner: winner_key,
            faction_id: entry.faction_id,
            roll_value,
            threshold_bps,
            timestamp: now,
        });

        msg!(
            "🎉 [process_lootbox_drops] WIN: asset {} -> {}",
            asset_key,
            winner_key
        );
    } else {
        emit!(LootboxRollMissed {
            winner: winner_key,
            roll_value,
            threshold_bps,
            timestamp: now,
        });
        msg!("❌ [process_lootbox_drops] MISS for {}", winner_key);
    }

    // Single-use: clear the pending roll regardless of outcome.
    ctx.accounts.winner_player_data.pending_lootbox_roll = None;

    Ok(())
}

#[derive(Accounts)]
pub struct ProcessLootboxDrops<'info> {
    #[account(mut)]
    pub crank_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    /// CHECK: Inventory custody PDA — same address as `inventory_pool`.
    /// Used only as the mpl-core authority during transfer; validated by
    /// seeds.
    #[account(
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pda: UncheckedAccount<'info>,

    #[account(
        seeds = [MARKET_METRICS_SEED],
        bump = market_metrics.bump,
    )]
    pub market_metrics: Box<Account<'info, MarketMetrics>>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, winner_player_data.owner.as_ref()],
        bump = winner_player_data.bump,
    )]
    pub winner_player_data: Box<Account<'info, PlayerData>>,

    /// CHECK: Recipient wallet for the dropped Doge.
    #[account(
        mut,
        constraint = winner_wallet.key() == winner_player_data.owner @ ErrorCode::InvalidOwner,
    )]
    pub winner_wallet: UncheckedAccount<'info>,

    /// On a winning roll, this account is closed manually inside the handler
    /// (rent refunded to winner). On a miss, it stays alive.
    #[account(
        mut,
        seeds = [RECYCLED_ENTRY_SEED, doge_asset.key().as_ref()],
        bump = recycled_entry.bump,
    )]
    pub recycled_entry: Box<Account<'info, RecycledEntry>>,

    /// CHECK: mpl-core asset; current owner is `inventory_pda`.
    #[account(mut)]
    pub doge_asset: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [DOGE_METADATA_SEED, doge_asset.key().as_ref()],
        bump = doge_metadata.bump,
        constraint = doge_metadata.mint == doge_asset.key() @ ErrorCode::InvalidAccount,
    )]
    pub doge_metadata: Account<'info, DogeMetadata>,

    /// CHECK: Doge collection account, required by mpl-core on transfer.
    pub doge_collection: Option<UncheckedAccount<'info>>,

    /// CHECK: Metaplex Core program.
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    /// CHECK: SlotHashes sysvar — used for entropy.
    #[account(address = anchor_lang::solana_program::sysvar::slot_hashes::ID)]
    pub slot_hashes: UncheckedAccount<'info>,

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
