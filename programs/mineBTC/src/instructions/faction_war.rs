//! Faction War (cycle) lifecycle, rankings, and reward distribution.
//!
//! A faction war is a multi-round "cycle" that determines country rankings
//! and pays out the big batched rewards (base dBTC, hashbeast bonus, MVP
//! bonus, plus their SOL mirrors). Individual rounds (see `game.rs`) feed
//! aggregates into the active cycle; once the LP-burn threshold is crossed
//! and the cycle's final round settles, the war is finalized and rewards
//! become claimable.
//!
//! # Cycle lifecycle
//!
//! ```text
//!   ┌────────────────────────────────────────────────────────────────────┐
//!   │ PHASE 0 — INIT CONFIG  (once, admin)                              │
//!   │   ix: initialize_war_config                                       │
//!   │   creates the singleton FactionWarConfig PDA                      │
//!   └────────────────────────────────────────────────────────────────────┘
//!                                  │
//!                                  ▼  (repeats every cycle)
//!   ┌────────────────────────────────────────────────────────────────────┐
//!   │ PHASE 1 — START WAR  (cranker)                                    │
//!   │   ix: initialize_faction_war(war_id)                              │
//!   │   creates FactionWarState + FactionWarSettlement PDAs for war_id  │
//!   │   sets settle_at_lp_op_count = lp_ops + 1                         │
//!   │   clears cycle_end_round_id ← unblocks start_round                │
//!   │   pulls unassigned-treasury SOL/dBTC forward as the war's seed    │
//!   └────────────────────────────────────────────────────────────────────┘
//!                                  │
//!                                  ▼
//!   ┌────────────────────────────────────────────────────────────────────┐
//!   │ PHASE 2 — ACTIVE  (many rounds, permissionless)                   │
//!   │   start_round → end_round → settle_round  (the 60s loop)          │
//!   │   settle_round calls track_war_round_completion() which folds     │
//!   │   game_session aggregates into FactionWarState (see game.rs)      │
//!   └────────────────────────────────────────────────────────────────────┘
//!                                  │
//!                                  ▼  (LP burn threshold crossed)
//!   ┌────────────────────────────────────────────────────────────────────┐
//!   │ PHASE 2b — CYCLE END SNAPSHOT                                     │
//!   │   add_lp_and_burn (in economy.rs) captures the current round_id   │
//!   │   into war_config.cycle_end_round_id when lp_ops crosses          │
//!   │   settle_at_lp_op_count. Once non-zero, start_round is blocked    │
//!   │   for the rest of this war.                                       │
//!   └────────────────────────────────────────────────────────────────────┘
//!                                  │
//!                                  ▼  (boundary round settles)
//!   ┌────────────────────────────────────────────────────────────────────┐
//!   │ PHASE 3 — SETTLE WAR  (cranker, permissionless)                   │
//!   │   ix: settle_war                                                  │
//!   │   require: last_processed_round_id == cycle_end_round_id          │
//!   │     (i.e. boundary round's aggregates are folded into war_state)  │
//!   │   ↓ finalize_war_settlement                                       │
//!   │     · compute final_ranks (gameplay_score, round_wins, faction_id)│
//!   │     · resolve direction (Up/Neutral/Down) per faction vs prev_ranks│
//!   │     · apply mining_multiplier → dbtc_mined_this_war               │
//!   │     · split dBTC + SOL into base / HB / MVP lanes by absolute      │
//!   │       rank weight; non-eligibles' slices stay unallocated         │
//!   │     · advance current_war_id; war_state.stage 0 → 1               │
//!   │   ↓ settle_war_internal then drains the SOL residual               │
//!   │     (war_settlement.undistributed_sol) to sol_treasury             │
//!   └────────────────────────────────────────────────────────────────────┘
//!                                  │
//!                                  ▼
//!   ┌────────────────────────────────────────────────────────────────────┐
//!   │ PHASE 4 — CLAIMS  (users + factions, permissionless)              │
//!   │   users: claim_faction_war_rewards (reads FactionWarSettlement)   │
//!   │   factions: claim_faction_treasury_for_faction_war (reads ranks)  │
//!   └────────────────────────────────────────────────────────────────────┘
//!                                  │
//!                                  ▼ (next cycle init unblocks rounds)
//!                          back to PHASE 1
//! ```
//!
//! # Ranking
//!
//! Final ranks are decided by `compute_rankings_into` with a 3-key sort:
//!   1. **gameplay_score** (descending) — accumulates from (a) round wins
//!      (winning country's wgtd_points on the winning direction) and (b)
//!      successful round-claim mutation rolls by users with a gameplay HB.
//!   2. **round_wins** (descending) — tiebreaker.
//!   3. **faction_id** (ascending) — final, deterministic tiebreaker.
//!
//! `resolve_direction_from_ranks` maps the delta against `prev_ranks` to
//! Up / Neutral / Down per faction.
//!
//! # Reward lanes (both dBTC and SOL — same shape, same bps)
//!
//! At settlement, the dBTC pool (= `total_dbtc_mined_in_rounds × mining_multiplier_bps`)
//! and the cycle's accumulated SOL pool are each split into 3 lanes:
//!
//! | Lane | Tuning bps | Eligibility | Per-user share formula |
//! |---|---|---|---|
//! | **base** | `war_base_reward_bps` (e.g. ~70%) | faction had bets on its resolved direction | `pool[fi] × user_wgtd[fi][resolved] / total_wgtd[fi][resolved]` summed across factions the user bet correctly on |
//! | **HB**   | leftover (~25%) | faction had any mutation_score this cycle (someone landed a mutation) | `pool[home] × user.mutation_score / faction_mutation_score[home]` — pure gameplay activity |
//! | **MVP**  | `war_mvp_reward_bps` (~5%) | faction has an MVP (highest-mutation-score user) | flat per-faction bonus, rank-weighted (`mvp_rank_weight_bps`: 30/20/14/10/8/6/5/4/share-3%) — only the named MVP user can claim |
//!
//! Per-faction allocation uses **absolute rank weights** (`FACTION_WAR_RANK_WEIGHT_BPS`
//! for base/HB, `mvp_rank_weight_bps` for MVP). Each rank's slot gets its
//! rank-weighted share of the lane; if the faction at that rank doesn't
//! qualify for the lane, that slot's share stays **unallocated** — no
//! cascade to other lanes, no over-allocation to eligibles. Unallocated
//! dBTC stays in the mining vault. Unallocated SOL is summed into
//! `war_settlement.undistributed_sol` and drained to `sol_treasury` at the
//! end of `settle_war_internal`.
//!
//! Per-user SOL across the 3 lanes is scaled by their dBTC share of the
//! same lane: `user_sol_<lane> = sol_<lane>_pool × user_dbtc_<lane> /
//! total_dbtc_<lane>_pool`. Same proportions as dBTC, lossless modulo
//! per-user rounding.
//!
//! # HashBeast progression hooks
//!
//! Rewards do not require RPG progression to be enabled. The base, HB, MVP,
//! and SOL lanes settle whenever there are eligible bets. The
//! `rpg_progression` flag only controls whether HashBeast DNA / XP /
//! multiplier mutation rolls can fire.
//!
//! Round-claim hook: when a user claims a winning round with a gameplay
//! HashBeast active, the claim path can roll for a mutation while the cycle is
//! still active. NFT-level effects apply regardless of which country won. The
//! cycle score accounting is split based on whether the winning faction is
//! the user's home country:
//!
//! **Home win** (winner == player's home faction) — full reward:
//!   - country's `gameplay_scores[winner]` += bonus
//!   - country's `faction_mutation_score[winner]` += bonus
//!   - user's `user_war_bets.mutation_score` += bonus
//!   - user surpassing current MVP becomes new `mvp_user[winner]`
//!
//! **Foreign win** ("mercenary mutation") — partial reward:
//!   - country's `gameplay_scores[winner]` += bonus / 2 (50% mercenary penalty)
//!   - HB-bonus pool, MVP candidacy, and user's `mutation_score` all stay
//!     unchanged. Mercenaries push the foreign country up the leaderboard
//!     but don't earn that country's HashBeast or MVP lanes.
//!
//! This split keeps the HB-bonus math safe (`user.mutation_score ≤
//! faction_mutation_score[home]` always) while still giving foreign-faction
//! bettors visible leaderboard impact — important for the "every action
//! matters" feedback loop Solana degens expect.
//!
//! Late rolls (cycle already settled, `war_state.stage != 0`) are silently
//! skipped — the bonus is dropped and nothing on the war state moves.
//!
//! War-claim hook: after settlement, `claim_war_rewards_internal` can sync the
//! user's active HashBeast with cycle reward value and, when RPG progression is
//! enabled, roll a separate cycle-accuracy mutation. This updates only the
//! player's HashBeast state; rankings and reward pools are already final.
//!
//! # Edge cases handled
//!
//! - **Empty cycle (zero bets)**: treasury seed is rolled forward into
//!   `tax_config.unassigned_war_treasury_amount` for the next cycle's seed.
//!   The full cycle SOL pool is marked undistributed and drained to
//!   `sol_treasury`. War advances normally.
//! - **No mutators in a faction**: that faction's HB and MVP slices stay
//!   unallocated (dBTC in mining vault, SOL → treasury).
//! - **No mutators in any faction this cycle**: HB and MVP lanes globally
//!   unallocated. Base lane unaffected.
//! - **RPG progression disabled**: prediction rewards, SOL claims, and ranks
//!   still settle; only HashBeast mutation/evolution rolls are skipped.
//! - **Late round-claim mutation roll after settle**: dropped silently
//!   (consistent with late-claim semantics).
//! - **Cycle boundary stuck-state**: `cycle_end_round_id` stays non-zero
//!   across the `finalize_war_settlement → initialize_war_internal` window,
//!   blocking `start_round` until the next war's PDA is created. This
//!   prevents rounds from being orphaned with a `war_id_when_played` whose
//!   FactionWarState doesn't exist yet.
//!
//! # File layout
//!
//! 1. **Helpers** — pure/computation functions (rankings, reward pools, etc.)
//! 2. **Lifecycle functions** — ordered by call sequence:
//!    `initialize_war_config_internal`, `initialize_war_internal`,
//!    `finalize_war_settlement`, `settle_war_internal`, and
//!    `claim_war_rewards_internal`.
//! 3. **Account structs** — all `#[derive(Accounts)]` grouped at the end,
//!    same order as the handlers above.

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
    scores: &[u64; NUM_FACTIONS],
    round_wins: &[u16; NUM_FACTIONS],
    active_factions: usize,
    ranks: &mut [u8; NUM_FACTIONS],
) -> Result<()> {
    require!(active_factions <= NUM_FACTIONS, ErrorCode::InvalidState);
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
            total = total.saturating_add(war_state.faction_direction_totals[faction_id][direction]);
        }
    }
    total
}

fn user_correct_cycle_sol(
    user_war_bets: &UserFactionWarBets,
    war_settlement: &FactionWarSettlement,
    active_factions: usize,
) -> u64 {
    let mut total = 0u64;
    for faction_id in 0..active_factions {
        let resolved = war_settlement.resolved_directions[faction_id] as usize;
        if resolved < PredictionDirection::COUNT {
            total = total.saturating_add(user_war_bets.sol_direction_bets[faction_id][resolved]);
        }
    }
    total
}

#[allow(clippy::too_many_arguments)]
fn process_war_claim_hashbeast_update<'info>(
    war_id: u64,
    owner_key: Pubkey,
    war_state: &FactionWarState,
    war_settlement: &FactionWarSettlement,
    user_war_bets: &UserFactionWarBets,
    player_data: &mut PlayerData,
    tuning: &GameplayTuningConfig,
    hashbeast_metadata: Option<&mut Box<Account<'info, HashBeastMetadata>>>,
    hashbeast_bonus_amount: u64,
    claim_won: bool,
) -> Result<u8> {
    if !claim_won
        || user_war_bets.gameplay_hashbeast == Pubkey::default()
        || user_war_bets.gameplay_hashbeast != player_data.gameplay_hashbeast
        || player_data.gameplay_hashbeast == Pubkey::default()
    {
        return Ok(0);
    }

    let active_factions = war_state.faction_count as usize;
    require!(active_factions <= NUM_FACTIONS, ErrorCode::InvalidState);

    let player_faction_id = player_data.faction_id as usize;
    if player_faction_id >= active_factions {
        return Ok(0);
    }

    let resolved_direction = war_settlement.resolved_directions[player_faction_id] as usize;
    if resolved_direction >= PredictionDirection::COUNT {
        return Ok(0);
    }

    let home_correct_sol = user_war_bets.sol_direction_bets[player_faction_id][resolved_direction];
    let all_correct_sol = user_correct_cycle_sol(user_war_bets, war_settlement, active_factions);
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
        // volume_factor uses the cycle's total SOL as the denominator,
        // regardless of which country the user backed correctly.
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
            war_state.start_timestamp.saturating_add(war_id),
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
        let hashbeast_metadata = hashbeast_metadata.ok_or(ErrorCode::HashBeastMetadataNotFound)?;
        require_keys_eq!(
            hashbeast_metadata.mint,
            user_war_bets.gameplay_hashbeast,
            ErrorCode::HashBeastMetadataNotFound
        );
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

fn pool_share_from_bps(pool: u64, bps: u16) -> Result<u64> {
    let share = (pool as u128)
        .checked_mul(bps as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(BASIS_POINTS_DENOMINATOR as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    u64::try_from(share).map_err(|_| ErrorCode::ArithmeticOverflow.into())
}

/// Distribute `pool` across factions by **absolute** rank weight
/// (FACTION_WAR_RANK_WEIGHT_BPS). The denominator is the sum of rank weights
/// across all `active_factions`, NOT just eligibles. Eligible factions get
/// their rank-weighted share; non-eligible factions' slices stay unallocated
/// and are returned as the residual. Caller decides what to do with the
/// residual (dBTC: leave in vault; SOL: drain to sol_treasury).
fn distribute_rank_weighted_absolute(
    pool: u64,
    final_ranks: &[u8; NUM_FACTIONS],
    eligible: &[bool; NUM_FACTIONS],
    active_factions: usize,
    pools_out: &mut [u64; NUM_FACTIONS],
) -> Result<u64> {
    *pools_out = [0u64; NUM_FACTIONS];
    require!(active_factions <= NUM_FACTIONS, ErrorCode::InvalidState);
    if pool == 0 || active_factions == 0 {
        return Ok(pool);
    }
    let mut total_weight: u128 = 0;
    for fid in 0..active_factions {
        let rank = final_ranks[fid] as usize;
        require!(rank < active_factions, ErrorCode::InvalidState);
        total_weight = total_weight
            .checked_add(FACTION_WAR_RANK_WEIGHT_BPS[final_ranks[fid] as usize] as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    if total_weight == 0 {
        return Ok(pool);
    }
    let mut allocated = 0u64;
    for fid in 0..active_factions {
        if !eligible[fid] {
            continue;
        }
        let rank = final_ranks[fid] as usize;
        require!(rank < active_factions, ErrorCode::InvalidState);
        let w = FACTION_WAR_RANK_WEIGHT_BPS[rank] as u128;
        let share_u128 = (pool as u128)
            .checked_mul(w)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(total_weight)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        let share = u64::try_from(share_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;
        pools_out[fid] = share;
        allocated = allocated
            .checked_add(share)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    Ok(pool.saturating_sub(allocated))
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

/// Allocate the MVP dBTC pool across MVP-eligible factions by absolute MVP
/// rank weight. Non-eligible factions' rank-slot shares stay unallocated
/// (dBTC remains in the mining vault). Last eligible MVP absorbs integer
/// division rounding so the total allocated equals the "target" exactly
/// (sub-bps drift never falls through to the mining vault).
///
/// Writes per-faction bonuses into `bonuses_out`. Returns the residual
/// (allocated to non-MVP rank slots) plus a list of MVP faction ids in
/// allocation order — caller uses the list to emit per-MVP events.
fn distribute_mvp_pool(
    mvp_pool_total: u64,
    mvp_user: &[Pubkey; NUM_FACTIONS],
    final_ranks: &[u8; NUM_FACTIONS],
    active_factions: usize,
    bonuses_out: &mut [u64; NUM_FACTIONS],
) -> Result<u64> {
    *bonuses_out = [0u64; NUM_FACTIONS];
    require!(active_factions <= NUM_FACTIONS, ErrorCode::InvalidState);
    if mvp_pool_total == 0 || active_factions == 0 {
        return Ok(mvp_pool_total);
    }

    let mut total_mvp_weight: u128 = 0;
    let mut eligible_mvp_weight: u128 = 0;
    let mut eligible_count: usize = 0;
    for fid in 0..active_factions {
        let rank = final_ranks[fid] as usize;
        require!(rank < active_factions, ErrorCode::InvalidState);
        let w = mvp_rank_weight_bps(rank, active_factions) as u128;
        total_mvp_weight = total_mvp_weight
            .checked_add(w)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        if mvp_user[fid] != Pubkey::default() {
            eligible_mvp_weight = eligible_mvp_weight
                .checked_add(w)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            eligible_count += 1;
        }
    }

    if total_mvp_weight == 0 || eligible_mvp_weight == 0 {
        // No MVPs anywhere → full pool is residual (dBTC stays in vault).
        return Ok(mvp_pool_total);
    }

    // Target = pool × eligible_weight / total_weight. Last MVP absorbs
    // rounding so we never under-allocate vs. that target.
    let target_dbtc_allocated = u64::try_from(
        (mvp_pool_total as u128)
            .checked_mul(eligible_mvp_weight)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(total_mvp_weight)
            .ok_or(ErrorCode::ArithmeticOverflow)?,
    )
    .map_err(|_| ErrorCode::ArithmeticOverflow)?;

    let mut allocated = 0u64;
    let mut remaining = eligible_count;
    for fid in 0..active_factions {
        if mvp_user[fid] == Pubkey::default() {
            continue;
        }
        remaining -= 1;
        let rank = final_ranks[fid] as usize;
        require!(rank < active_factions, ErrorCode::InvalidState);
        let weight_bps = mvp_rank_weight_bps(rank, active_factions);
        let bonus = if remaining == 0 {
            target_dbtc_allocated.saturating_sub(allocated)
        } else {
            let computed = (mvp_pool_total as u128)
                .checked_mul(weight_bps as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(total_mvp_weight)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            u64::try_from(computed).map_err(|_| ErrorCode::ArithmeticOverflow)?
        };
        bonuses_out[fid] = bonus;
        allocated = allocated
            .checked_add(bonus)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    Ok(mvp_pool_total.saturating_sub(allocated))
}

/// Compute the SOL MVP eligible share + residual. The "eligible share" is
/// the fraction of `sol_mvp_total` that mirrors the eligible-MVP fraction of
/// the rank-weight pie; the rest is residual (drained to sol_treasury at
/// settle). Per-user SOL MVP payout at claim time then scales this eligible
/// share by the user's dBTC MVP share.
///
/// Returns `(sol_eligible_share, sol_residual)` where their sum == `sol_mvp_total`.
fn split_sol_mvp_residual(
    sol_mvp_total: u64,
    mvp_user: &[Pubkey; NUM_FACTIONS],
    final_ranks: &[u8; NUM_FACTIONS],
    active_factions: usize,
) -> Result<(u64, u64)> {
    require!(active_factions <= NUM_FACTIONS, ErrorCode::InvalidState);
    if sol_mvp_total == 0 || active_factions == 0 {
        return Ok((0, sol_mvp_total));
    }

    let mut total_mvp_weight: u128 = 0;
    let mut eligible_mvp_weight: u128 = 0;
    for fid in 0..active_factions {
        let rank = final_ranks[fid] as usize;
        require!(rank < active_factions, ErrorCode::InvalidState);
        let w = mvp_rank_weight_bps(rank, active_factions) as u128;
        total_mvp_weight = total_mvp_weight
            .checked_add(w)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        if mvp_user[fid] != Pubkey::default() {
            eligible_mvp_weight = eligible_mvp_weight
                .checked_add(w)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
    }
    if total_mvp_weight == 0 {
        return Ok((0, sol_mvp_total));
    }

    let sol_eligible_share = u64::try_from(
        (sol_mvp_total as u128)
            .checked_mul(eligible_mvp_weight)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(total_mvp_weight)
            .ok_or(ErrorCode::ArithmeticOverflow)?,
    )
    .map_err(|_| ErrorCode::ArithmeticOverflow)?;

    Ok((
        sol_eligible_share,
        sol_mvp_total.saturating_sub(sol_eligible_share),
    ))
}

fn split_sol_mvp_residual_with_dbtc_guard(
    sol_mvp_total: u64,
    mvp_dbtc_allocated: u64,
    mvp_user: &[Pubkey; NUM_FACTIONS],
    final_ranks: &[u8; NUM_FACTIONS],
    active_factions: usize,
) -> Result<(u64, u64)> {
    if mvp_dbtc_allocated == 0 {
        return Ok((0, sol_mvp_total));
    }
    split_sol_mvp_residual(sol_mvp_total, mvp_user, final_ranks, active_factions)
}

/// Per-user SOL share for a lane. The claim path uses this to scale the
/// lane's SOL pool by the user's dBTC share of the same lane:
///   `user_sol = sol_lane_pool × user_dbtc_lane / total_dbtc_lane`
/// Returns 0 if any input is 0 (so an empty dBTC denominator never panics —
/// stranded SOL handling lives in compute_base_reward_pools instead).
#[inline]
fn scale_user_sol_lane(
    sol_lane_pool: u64,
    user_dbtc_lane: u64,
    total_dbtc_lane: u64,
) -> Result<u64> {
    if sol_lane_pool == 0 || user_dbtc_lane == 0 || total_dbtc_lane == 0 {
        return Ok(0);
    }
    let s = (sol_lane_pool as u128)
        .checked_mul(user_dbtc_lane as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(total_dbtc_lane as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    u64::try_from(s).map_err(|_| ErrorCode::ArithmeticOverflow.into())
}

fn native_vault_payable_amount(vault: &AccountInfo<'_>, amount: u64) -> Result<u64> {
    let rent_floor = Rent::get()?.minimum_balance(vault.data_len());
    Ok(amount.min(vault.lamports().saturating_sub(rent_floor)))
}

/// Compute how the faction_war mining pool is split across factions.
///
/// The total pool is first split into three lanes:
/// - base rewards: anyone correct on a country's resolved direction
/// - mvp rewards: top contributor per faction (distributed at settlement by rank)
/// - HashBeast rewards: gameplay HashBeasts backing the resolved home-country outcome
///
/// Each lane is distributed by absolute final-rank weight across all active
/// factions. Ineligible factions receive 0 and their rank-slot share remains
/// unallocated.
/// Compute base + HB reward pools (dBTC and SOL) using **absolute** rank-weight
/// distribution. Each faction's slice is determined by its rank weight relative
/// to the sum of rank weights across all active factions. Non-eligible factions'
/// slices stay unallocated.
///
/// Returns the SOL residual (base + HB lanes) so the caller can drain it to
/// `sol_treasury`. The dBTC residual is implicit — uneligible factions get 0
/// in the settlement, so claim path never pays from those slots and the dBTC
/// stays in the mining vault.
pub fn compute_base_reward_pools(
    war_state: &FactionWarState,
    war_settlement: &mut FactionWarSettlement,
    tuning: &GameplayTuningConfig,
) -> Result<u64> {
    let active_factions = war_state.faction_count as usize;
    require!(active_factions <= NUM_FACTIONS, ErrorCode::InvalidState);
    require!(
        war_settlement.war_id == war_state.war_id,
        ErrorCode::InvalidState
    );

    let pool = war_state.dbtc_mined_this_war;
    let sol_pool = war_state.sol_reward_pool;
    if pool == 0 {
        // No dBTC pool → no scaling denominator exists for SOL at claim time
        // (`user_sol_<lane> = sol_<lane>_pool * user_dbtc_<lane> / total_dbtc_<lane>`).
        // Drain the entire SOL pool to sol_treasury instead of leaving it
        // stranded in the war SOL vault. Eligible-everything case still funnels
        // through `undistributed_sol`.
        war_settlement.base_reward_pools = [0u64; NUM_FACTIONS];
        war_settlement.hashbeast_reward_pools = [0u64; NUM_FACTIONS];
        war_settlement.mvp_bonus = [0u64; NUM_FACTIONS];
        war_settlement.sol_base_pool = 0;
        war_settlement.sol_hb_pool = 0;
        war_settlement.sol_mvp_pool = 0;
        return Ok(sol_pool);
    }

    // dBTC lane totals.
    let base_pool_total = pool_share_from_bps(pool, tuning.war_base_reward_bps)?;
    let mvp_pool_total = pool_share_from_bps(pool, tuning.war_mvp_reward_bps)?;
    let hashbeast_pool_total = pool
        .checked_sub(base_pool_total)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_sub(mvp_pool_total)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // SOL lane totals — same bps. Per-user claim scales each lane's SOL pool
    // by user_dbtc_lane / total_dbtc_lane, so distributions stay in sync.
    let sol_base_total = pool_share_from_bps(sol_pool, tuning.war_base_reward_bps)?;
    let sol_mvp_total = pool_share_from_bps(sol_pool, tuning.war_mvp_reward_bps)?;
    let sol_hb_total = sol_pool
        .checked_sub(sol_base_total)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_sub(sol_mvp_total)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    let mut eligible_base = [false; NUM_FACTIONS];
    let mut eligible_hashbeast = [false; NUM_FACTIONS];

    for f in 0..active_factions {
        let winning_dir = war_settlement.resolved_directions[f] as usize;
        require!(
            winning_dir < PredictionDirection::COUNT,
            ErrorCode::InvalidState
        );
        eligible_base[f] = war_state.faction_direction_totals[f][winning_dir] > 0;
        eligible_hashbeast[f] = war_state.faction_mutation_score[f] > 0;
    }

    // dBTC base + HB: absolute rank-weighted distribution to eligibles only.
    // Non-eligibles' slices stay 0 in settlement → claim never pays them out
    // → dBTC simply remains in the mining vault. No cascade.
    let base_dbtc_residual = distribute_rank_weighted_absolute(
        base_pool_total,
        &war_settlement.final_ranks,
        &eligible_base,
        active_factions,
        &mut war_settlement.base_reward_pools,
    )?;
    let hb_dbtc_residual = distribute_rank_weighted_absolute(
        hashbeast_pool_total,
        &war_settlement.final_ranks,
        &eligible_hashbeast,
        active_factions,
        &mut war_settlement.hashbeast_reward_pools,
    )?;
    let base_dbtc_allocated = base_pool_total.saturating_sub(base_dbtc_residual);
    let hb_dbtc_allocated = hashbeast_pool_total.saturating_sub(hb_dbtc_residual);

    // SOL base + HB: same shape. Capture residuals — caller drains to treasury.
    // We don't persist per-faction SOL arrays since the claim path scales each
    // SOL lane by the user's share of the corresponding dBTC lane sum, which
    // already excludes non-eligible factions.
    let mut sol_base_tmp = [0u64; NUM_FACTIONS];
    let mut sol_hb_tmp = [0u64; NUM_FACTIONS];
    let sol_base_residual = if base_dbtc_allocated == 0 {
        sol_base_total
    } else {
        distribute_rank_weighted_absolute(
            sol_base_total,
            &war_settlement.final_ranks,
            &eligible_base,
            active_factions,
            &mut sol_base_tmp,
        )?
    };
    let sol_hb_residual = if hb_dbtc_allocated == 0 {
        sol_hb_total
    } else {
        distribute_rank_weighted_absolute(
            sol_hb_total,
            &war_settlement.final_ranks,
            &eligible_hashbeast,
            active_factions,
            &mut sol_hb_tmp,
        )?
    };

    war_settlement.sol_base_pool = sol_base_total.saturating_sub(sol_base_residual);
    war_settlement.sol_hb_pool = sol_hb_total.saturating_sub(sol_hb_residual);
    war_settlement.sol_mvp_pool = sol_mvp_total; // MVP residual computed by caller

    let total_sol_residual = sol_base_residual
        .checked_add(sol_hb_residual)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    Ok(total_sol_residual)
}

// ========================================================================================
// ============================= LIFECYCLE — ADMIN CONFIG ===================================
// ========================================================================================

pub fn initialize_war_config_internal(ctx: Context<InitializeFactionWarConfig>) -> Result<()> {
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
pub fn initialize_war_internal(ctx: Context<InitializeFactionWar>, war_id: u64) -> Result<()> {
    crate::log_fn!("faction_war", "initialize_war_internal");
    msg!("🪖 [initialize_faction_war] war={}", war_id);

    let war_config = &mut ctx.accounts.war_config;
    let war_state = &mut ctx.accounts.war_state;
    let tax_config = &mut ctx.accounts.tax_config;
    let global_config = &ctx.accounts.global_config;

    require!(
        war_config.current_war_id == war_id,
        ErrorCode::InvalidParameters
    );

    require!(
        global_config.supported_factions.len() == NUM_FACTIONS,
        ErrorCode::InvalidFactionId
    );
    let faction_count = NUM_FACTIONS as u8;
    let start_ranks = war_config.prev_ranks;
    let unassigned = tax_config.unassigned_war_treasury_amount;

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

    let war_close_state = &mut ctx.accounts.war_close_state;
    war_close_state.bump = ctx.bumps.war_close_state;
    war_close_state.war_id = war_id;
    war_close_state.rent_payer = ctx.accounts.authority.key();
    war_close_state.open_game_session_count = 0;
    war_close_state.pending_war_claim_count = 0;

    tax_config.unassigned_war_treasury_amount = 0;

    let lp_ops = ctx.accounts.dbtc_mining.pol_stats.lp_operations_count;
    let settle_cycle = lp_ops.checked_add(1).ok_or(ErrorCode::ArithmeticOverflow)?;
    war_config.settle_at_lp_op_count = settle_cycle;
    war_config.reset_cycle_round_tracking();
    // Clear the previous cycle's boundary marker. start_round was blocked
    // until this point; once we return, the next cycle's rounds can begin.
    war_config.cycle_end_round_id = 0;

    emit!(FactionWarStarted {
        war_id,
        faction_count,
        start_timestamp: war_state.start_timestamp,
        prev_ranks: start_ranks,
        settle_at_lp_op_count: settle_cycle,
        treasury_reward_base_amount: unassigned,
    });
    emit!(FactionWarCloseStateInitialized {
        war_id,
        close_state: war_close_state.key(),
        rent_payer: ctx.accounts.authority.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

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
    require!(active_factions <= NUM_FACTIONS, ErrorCode::InvalidState);
    require!(
        war_config.current_war_id == war_state.war_id,
        ErrorCode::InvalidState
    );
    require!(
        war_settlement.war_id == war_state.war_id,
        ErrorCode::InvalidState
    );
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
            tax_config.unassigned_war_treasury_amount = tax_config
                .unassigned_war_treasury_amount
                .checked_add(war_state.treasury_reward_base_amount)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            war_state.treasury_reward_base_amount = 0;
        }
        war_state.stage = 1;
        war_state.dbtc_mined_this_war = 0;
        war_settlement.base_reward_pools = [0u64; NUM_FACTIONS];
        war_settlement.hashbeast_reward_pools = [0u64; NUM_FACTIONS];
        // Nothing claimable for this cycle → drain the full SOL pool to
        // sol_treasury via settle_war_internal.
        war_settlement.undistributed_sol = war_state.sol_reward_pool;
        war_config.current_war_id = war_config
            .current_war_id
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        // cycle_end_round_id stays — cleared by init_war for the next cycle so
        // that start_round remains blocked until next war's PDA is created.
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
        .copied()
        .try_fold(0u64, |acc, v| acc.checked_add(v))
        .ok_or(ErrorCode::ArithmeticOverflow)?;
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
            tax_config.unassigned_war_treasury_amount = tax_config
                .unassigned_war_treasury_amount
                .checked_add(war_state.treasury_reward_base_amount)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            war_state.treasury_reward_base_amount = 0;
        }
        war_state.dbtc_mined_this_war = 0;
        war_settlement.base_reward_pools = [0u64; NUM_FACTIONS];
        war_settlement.hashbeast_reward_pools = [0u64; NUM_FACTIONS];
        // Nothing claimable for this cycle → drain the full SOL pool to
        // sol_treasury via settle_war_internal.
        war_settlement.undistributed_sol = war_state.sol_reward_pool;
        war_state.stage = 1;
    } else {
        war_state.dbtc_mined_this_war = apply_mining_multiplier(
            war_state.total_dbtc_mined_in_rounds,
            war_config.mining_multiplier_bps,
        )?;

        let mut final_ranks = [0u8; NUM_FACTIONS];
        compute_rankings_into(
            &war_state.gameplay_scores,
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

        let base_hb_sol_residual = compute_base_reward_pools(war_state, war_settlement, tuning)?;

        // --- MVP Bonus: absolute rank-weight distribution ---
        // Curve: 30/20/14/10/8/6/5/4/share-3% (ALGS-style top-heavy with a
        // meaningful tail). See `mvp_rank_weight_bps`. The denominator is
        // the sum across ALL active factions — not just MVP-eligibles —
        // so non-eligible rank slots stay unallocated (dBTC in mining vault,
        // SOL drains to sol_treasury).
        let mvp_pool_total =
            pool_share_from_bps(war_state.dbtc_mined_this_war, tuning.war_mvp_reward_bps)?;

        // dBTC MVP allocation (per-faction bonuses written into settlement).
        let mvp_dbtc_residual = distribute_mvp_pool(
            mvp_pool_total,
            &war_state.mvp_user,
            &final_ranks,
            active_factions,
            &mut war_settlement.mvp_bonus,
        )?;
        let mvp_dbtc_allocated = mvp_pool_total.saturating_sub(mvp_dbtc_residual);

        // SOL MVP — eligible share stays on settlement; residual drains.
        // `sol_mvp_pool` was set by `compute_base_reward_pools` to the full
        // pre-eligibility-shrink slice; replace it with the eligible amount
        // here so claim-time scaling stays self-consistent.
        let sol_mvp_total = war_settlement.sol_mvp_pool;
        let (sol_mvp_eligible, mvp_sol_residual) = split_sol_mvp_residual_with_dbtc_guard(
            sol_mvp_total,
            mvp_dbtc_allocated,
            &war_state.mvp_user,
            &final_ranks,
            active_factions,
        )?;
        war_settlement.sol_mvp_pool = sol_mvp_eligible;

        // Total SOL residual across base + HB + MVP lanes — drained to
        // sol_treasury by settle_war_internal after this function returns.
        war_settlement.undistributed_sol = base_hb_sol_residual
            .checked_add(mvp_sol_residual)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        war_state.stage = 1;
        war_config.prev_ranks = final_ranks;
    }

    war_config.current_war_id = war_config
        .current_war_id
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    // NOTE: `cycle_end_round_id` is intentionally NOT cleared here. It stays
    // non-zero through this gap so `start_round` remains blocked until the
    // next war's PDA is initialized by `initialize_war_internal`. Otherwise a
    // round could start with `game_session.war_id_when_played = new_war_id`
    // before the corresponding war_state PDA exists, and `settle_round` would
    // fail on PDA seed validation — leaving the round stuck in stage 1.

    msg!(
        "✅ [faction_war.finalize_war_settlement] FactionWar settled. Next war_id: {} (cycle_end_round_id stays {} until init_war clears it)",
        war_config.current_war_id,
        war_config.cycle_end_round_id
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
        war_config.current_war_id == war_state.war_id,
        ErrorCode::InvalidState
    );
    require!(
        ctx.accounts.war_settlement.war_id == war_state.war_id,
        ErrorCode::InvalidState
    );
    require!(
        (war_state.faction_count as usize) <= NUM_FACTIONS,
        ErrorCode::InvalidState
    );
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
            && war_config.last_processed_round_id == war_config.cycle_end_round_id,
        ErrorCode::RoundFinalizationPending
    );
    msg!(
        "✅ [faction_war.settle_war_internal] cycle-boundary check passed (cycle_end_round_id={})",
        war_config.cycle_end_round_id
    );

    // war_state.sol_reward_pool is maintained per-round via the
    // game_session.cycle_sol_pool → war_state.sol_reward_pool fold in
    // track_war_round_completion, so settlement just reads it directly.
    let war_settlement = &mut *ctx.accounts.war_settlement;
    finalize_war_settlement(war_config, war_state, war_settlement, tax_config, tuning)?;

    // Drain SOL that no eligible claimant can claim → sol_treasury. The amount
    // was computed inside finalize_war_settlement (per-lane residuals summed,
    // plus the full sol pool when no bets exist for the cycle).
    let undistributed_sol = war_settlement.undistributed_sol;
    if undistributed_sol > 0 {
        let drain_sol = native_vault_payable_amount(
            &ctx.accounts.faction_war_sol_vault.to_account_info(),
            undistributed_sol,
        )?;
        if drain_sol > 0 {
            let vault_seeds: &[&[u8]] = &[
                FACTION_WAR_SOL_VAULT_SEED,
                &[war_config.rewards_sol_vault_bump],
            ];
            let signer: &[&[&[u8]]] = &[vault_seeds];
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.faction_war_sol_vault.to_account_info(),
                        to: ctx.accounts.sol_treasury.to_account_info(),
                    },
                    signer,
                ),
                drain_sol,
            )?;
            msg!(
                "💸 [faction_war.settle_war_internal] drained undistributed SOL {} lamports → sol_treasury",
                drain_sol
            );
        } else {
            msg!("⚠️ [faction_war.settle_war_internal] no withdrawable undistributed SOL after rent reserve");
        }
    }

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
        sol_base_pool: war_settlement.sol_base_pool,
        sol_hb_pool: war_settlement.sol_hb_pool,
        sol_mvp_pool: war_settlement.sol_mvp_pool,
        undistributed_sol: war_settlement.undistributed_sol,
        mvp_bonus: war_settlement.mvp_bonus,
        mvp_user: war_state.mvp_user,
        mvp_score: war_state.mvp_score,
        faction_mutation_scores: war_state.faction_mutation_score,
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

pub fn claim_war_rewards_internal(ctx: Context<ClaimFactionWarRewards>, war_id: u64) -> Result<()> {
    crate::log_fn!("faction_war", "claim_war_rewards_internal");
    msg!(
        "⚔️ [faction_war.claim_war_rewards_internal] FactionWar #{}, user={}",
        war_id,
        ctx.accounts.user_war_bets.owner
    );
    let war_state = &ctx.accounts.war_state;
    let war_settlement = &ctx.accounts.war_settlement;
    let user_war_bets = &ctx.accounts.user_war_bets;
    let player_data_key = ctx.accounts.player_data.key();
    let player_data = &mut ctx.accounts.player_data;
    let hodl_pool = &mut ctx.accounts.hodl_pool;
    let clock = Clock::get()?;
    let owner_key = user_war_bets.owner;

    require!(war_state.war_id == war_id, ErrorCode::InvalidState);
    require!(war_settlement.war_id == war_id, ErrorCode::InvalidState);
    require!(user_war_bets.war_id == war_id, ErrorCode::InvalidState);
    require!(
        ctx.accounts.war_close_state.war_id == war_id,
        ErrorCode::InvalidCloseState
    );
    require!(war_state.stage == 1, ErrorCode::FactionWarNotSettled);
    msg!("✅ [faction_war.claim_war_rewards_internal] stage==1 check passed");
    require_keys_eq!(player_data.owner, owner_key, ErrorCode::InvalidOwner);
    require_keys_eq!(
        ctx.accounts.user_war_bet_close_state.owner,
        owner_key,
        ErrorCode::InvalidCloseState
    );
    require!(
        ctx.accounts.user_war_bet_close_state.war_id == war_id,
        ErrorCode::InvalidCloseState
    );
    require_keys_eq!(
        ctx.accounts.user_war_bet_rent_payer.key(),
        ctx.accounts.user_war_bet_close_state.rent_payer,
        ErrorCode::InvalidCloseState
    );
    require!(
        ctx.accounts.user_war_bet_close_state.open_round_claim_count == 0,
        ErrorCode::PendingClaimsRemaining
    );
    msg!("✅ [faction_war.claim_war_rewards_internal] owner check passed");

    let active_factions = war_state.faction_count as usize;
    require!(active_factions <= NUM_FACTIONS, ErrorCode::InvalidState);
    let player_faction_id = player_data.faction_id as usize;
    let mut base_reward_amount = 0u64;
    let mut hashbeast_bonus_amount = 0u64;
    let mut hashbeast_mint = Pubkey::default();
    msg!(
        "⚔️ [faction_war.claim_war_rewards_internal] active_factions={} player_faction_id={}",
        active_factions,
        player_faction_id
    );

    if active_factions == 0 {
        msg!(
            "⚔️ [faction_war.claim_war_rewards_internal] FactionWar {} settled with 0 active factions. Closing claim with 0 reward.",
            war_id
        );
    } else {
        msg!("✅ [faction_war.claim_war_rewards_internal] validation skipped");

        // --- Base reward: per-faction, requires correct direction. ---
        for faction_id in 0..active_factions {
            let resolved_direction = war_settlement.resolved_directions[faction_id] as usize;
            require!(
                resolved_direction < PredictionDirection::COUNT,
                ErrorCode::InvalidState
            );
            let user_bet = user_war_bets.direction_bets[faction_id][resolved_direction];
            msg!("⚔️ [faction_war.claim_war_rewards_internal] loop faction_id={} resolved_direction={} user_bet={}", faction_id, resolved_direction, user_bet);
            if user_bet == 0 {
                msg!(
                    "⚔️ [faction_war.claim_war_rewards_internal] skip faction_id={} (user_bet==0)",
                    faction_id
                );
                continue;
            }

            let total_bet = war_state.faction_direction_totals[faction_id][resolved_direction];
            let faction_pool = war_settlement.base_reward_pools[faction_id];
            msg!("📊 [faction_war.claim_war_rewards_internal] base calc faction_id={} total_bet={} faction_pool={}", faction_id, total_bet, faction_pool);

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
                msg!(
                    "📊 [faction_war.claim_war_rewards_internal] base_reward: old={} add={} new={}",
                    old_base,
                    reward,
                    base_reward_amount
                );
            } else {
                msg!("📊 [faction_war.claim_war_rewards_internal] base calc skipped total_bet={} faction_pool={}", total_bet, faction_pool);
            }
        }

        // --- HB bonus: pure gameplay lane.
        // Numerator   = user's cumulative mutation_score on their home faction
        //               (incremented each time a round-claim mutation roll lands).
        // Denominator = country's total mutation_score across all HB players.
        // No mutations → no HB-bonus, regardless of how much SOL was bet. The
        // base + SOL-base lanes already pay the active-bettor case.
        if user_war_bets.mutation_score > 0 && player_faction_id < active_factions {
            let hashbeast_pool = war_settlement.hashbeast_reward_pools[player_faction_id];
            let faction_mutation_total = war_state.faction_mutation_score[player_faction_id];
            let user_mutation_score = user_war_bets.mutation_score;
            msg!("📊 [faction_war.claim_war_rewards_internal] hashbeast calc hashbeast_pool={} faction_mutation_total={} user_mutation_score={}",
                hashbeast_pool, faction_mutation_total, user_mutation_score);
            if hashbeast_pool > 0 && faction_mutation_total > 0 {
                require!(
                    user_mutation_score <= faction_mutation_total,
                    ErrorCode::InvalidState
                );
                let bonus_u128 = (hashbeast_pool as u128)
                    .checked_mul(user_mutation_score as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?
                    .checked_div(faction_mutation_total as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
                hashbeast_bonus_amount =
                    u64::try_from(bonus_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;
                msg!(
                    "📊 [faction_war.claim_war_rewards_internal] hashbeast_bonus_amount={}",
                    hashbeast_bonus_amount
                );
                if hashbeast_bonus_amount > 0 {
                    hashbeast_mint = user_war_bets.gameplay_hashbeast;
                    msg!(
                        "⚔️ [faction_war.claim_war_rewards_internal] hashbeast_mint set={}",
                        hashbeast_mint
                    );
                }
            } else {
                msg!("📊 [faction_war.claim_war_rewards_internal] hashbeast calc skipped");
            }
        }
    }

    let mut total_reward = base_reward_amount;
    msg!(
        "📊 [faction_war.claim_war_rewards_internal] total_reward after base={}",
        total_reward
    );

    // --- MVP Bonus: if this user is any faction's MVP, add their pre-computed bonus ---
    let mut mvp_bonus_amount = 0u64;
    for fid in 0..active_factions {
        if war_state.mvp_user[fid] == owner_key {
            let faction_mvp_bonus = war_settlement.mvp_bonus[fid];
            msg!(
                "🏆 [faction_war.claim_war_rewards_internal] MVP match fid={} mvp_bonus_amount={}",
                fid,
                faction_mvp_bonus
            );
            if faction_mvp_bonus > 0 {
                mvp_bonus_amount = mvp_bonus_amount
                    .checked_add(faction_mvp_bonus)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
                let old_total = total_reward;
                total_reward = total_reward
                    .checked_add(faction_mvp_bonus)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
                msg!(
                    "🏆 [faction_war.claim_war_rewards_internal] MVP Bonus claimed: faction={} rank={} bonus={} total_reward: {} -> {}",
                    fid,
                    war_settlement.final_ranks[fid] + 1,
                    faction_mvp_bonus,
                    old_total,
                    total_reward
                );
            }
        }
    }

    // --- SOL rewards: mirror the dBTC 3-lane split (base / HB / MVP).
    // For each lane: user_sol = sol_<lane>_pool * user_dbtc_<lane> / total_dbtc_<lane>_pool.
    // Distributing SOL by the same proportions preserves identical relative
    // payouts to dBTC across the cohort while only paying out where dBTC was
    // also paid (skips orphan/zero lanes naturally).
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

    let sol_base_share = scale_user_sol_lane(
        war_settlement.sol_base_pool,
        base_reward_amount,
        total_dbtc_base,
    )?;
    let sol_hb_share = scale_user_sol_lane(
        war_settlement.sol_hb_pool,
        hashbeast_bonus_amount,
        total_dbtc_hb,
    )?;
    let sol_mvp_share = scale_user_sol_lane(
        war_settlement.sol_mvp_pool,
        mvp_bonus_amount,
        total_dbtc_mvp,
    )?;

    let sol_reward: u64 = sol_base_share
        .checked_add(sol_hb_share)
        .and_then(|s| s.checked_add(sol_mvp_share))
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    msg!(
        "💰 [faction_war.claim_war_rewards_internal] sol shares: base={} hb={} mvp={} total={}",
        sol_base_share,
        sol_hb_share,
        sol_mvp_share,
        sol_reward
    );
    let mut sol_reward_paid = 0u64;
    if sol_reward > 0 {
        sol_reward_paid =
            native_vault_payable_amount(&ctx.accounts.war_sol_vault.to_account_info(), sol_reward)?;
        msg!(
            "💰 [faction_war.claim_war_rewards_internal] Transferring SOL cycle reward: {} lamports ({} SOL)",
            sol_reward_paid,
            sol_reward_paid as f64 / 1e9
        );
        if sol_reward_paid > 0 {
            let vault_seeds = &[
                FACTION_WAR_SOL_VAULT_SEED.as_ref(),
                &[ctx.bumps.war_sol_vault],
            ];
            let signer = &[&vault_seeds[..]];
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.war_sol_vault.to_account_info(),
                        to: ctx.accounts.player.to_account_info(),
                    },
                    signer,
                ),
                sol_reward_paid,
            )?;
            msg!("💰 [faction_war.claim_war_rewards_internal] SOL transfer complete");
        }
    }
    let (sol_base_paid, sol_hb_paid, sol_mvp_paid) =
        if sol_reward > 0 && sol_reward_paid < sol_reward {
            let base = u64::try_from(
                (sol_base_share as u128)
                    .checked_mul(sol_reward_paid as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?
                    .checked_div(sol_reward as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?,
            )
            .map_err(|_| ErrorCode::ArithmeticOverflow)?;
            let hb = u64::try_from(
                (sol_hb_share as u128)
                    .checked_mul(sol_reward_paid as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?
                    .checked_div(sol_reward as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?,
            )
            .map_err(|_| ErrorCode::ArithmeticOverflow)?;
            let mvp = sol_reward_paid
                .checked_sub(base)
                .and_then(|r| r.checked_sub(hb))
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            (base, hb, mvp)
        } else {
            (sol_base_share, sol_hb_share, sol_mvp_share)
        };

    let claim_won = total_reward > 0 || sol_reward > 0 || hashbeast_bonus_amount > 0;
    let claim_mutation_type = process_war_claim_hashbeast_update(
        war_id,
        owner_key,
        war_state,
        war_settlement,
        user_war_bets,
        player_data,
        &ctx.accounts.global_config.gameplay_tuning,
        ctx.accounts.hashbeast_metadata.as_mut(),
        hashbeast_bonus_amount,
        claim_won,
    )?;
    if claim_mutation_type > 0
        || (claim_won && user_war_bets.gameplay_hashbeast != Pubkey::default())
    {
        hashbeast_mint = user_war_bets.gameplay_hashbeast;
    }

    msg!(
        "⚔️ [faction_war.claim_war_rewards_internal] Player faction {}: base_reward={}, mvp_bonus={}, total_reward={}, hashbeast_bonus={}, user_mutation_score={}, sol_reward={}",
        player_faction_id,
        base_reward_amount,
        mvp_bonus_amount,
        total_reward,
        hashbeast_bonus_amount,
        user_war_bets.mutation_score,
        sol_reward_paid
    );

    if total_reward > 0 {
        msg!(
            "⚔️ [faction_war.claim_war_rewards_internal] adding to total_claimable total_reward={}",
            total_reward
        );
        helper::add_to_total_claimable(
            hodl_pool,
            player_data,
            total_reward,
            owner_key,
            player_data_key,
            CLAIMABLE_DBTC_SOURCE_FACTION_WAR,
            war_id,
        )?;
        msg!("✅ [faction_war.claim_war_rewards_internal] add_to_total_claimable done");
    } else {
        msg!("⚔️ [faction_war.claim_war_rewards_internal] total_reward==0, skipping add_to_total_claimable");
    }

    let old_pending = player_data.pending_war_claims;
    player_data.pending_war_claims = player_data
        .pending_war_claims
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    ctx.accounts.war_close_state.pending_war_claim_count = ctx
        .accounts
        .war_close_state
        .pending_war_claim_count
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "⚔️ [faction_war.claim_war_rewards_internal] pending_war_claims: {} -> {}",
        old_pending,
        player_data.pending_war_claims
    );

    // Note: lootbox rolls fire on round-claim for losing players, not on
    // cycle-claim. See `claim_round_rewards` in `user.rs`.

    msg!("⚔️ [faction_war.claim_war_rewards_internal] emitting FactionWarRewardsClaimed: war_id={} user={} reward_amount={} base={} mvp={} hashbeast={} sol={} timestamp={}",
        war_id,
        user_war_bets.owner,
        total_reward,
        base_reward_amount,
        mvp_bonus_amount,
        hashbeast_bonus_amount,
        sol_reward_paid,
        clock.unix_timestamp
    );
    emit!(FactionWarRewardsClaimed {
        war_id,
        user: user_war_bets.owner,
        reward_amount: total_reward,
        base_reward_amount,
        mvp_bonus_amount,
        hashbeast_bonus_amount,
        sol_reward_amount: sol_reward_paid,
        sol_base_amount: sol_base_paid,
        sol_hb_amount: sol_hb_paid,
        sol_mvp_amount: sol_mvp_paid,
        hashbeast_mint,
        timestamp: clock.unix_timestamp,
    });

    msg!("✅ [faction_war.claim_war_rewards_internal] claim complete");
    Ok(())
}

/// Close a fully-settled faction-war cycle after all per-user war claims,
/// child game sessions, and faction treasury claims are closed.
pub fn close_faction_war_accounts_internal(
    ctx: Context<CloseFactionWarAccounts>,
    war_id: u64,
) -> Result<()> {
    crate::log_fn!("faction_war", "close_faction_war_accounts_internal");
    let war_state = &ctx.accounts.war_state;
    let war_settlement = &ctx.accounts.war_settlement;
    let war_close_state = &ctx.accounts.war_close_state;

    require!(war_state.war_id == war_id, ErrorCode::InvalidState);
    require!(war_settlement.war_id == war_id, ErrorCode::InvalidState);
    require!(
        war_close_state.war_id == war_id,
        ErrorCode::InvalidCloseState
    );
    require!(war_state.stage == 1, ErrorCode::FactionWarNotSettled);
    require!(
        ctx.accounts.war_config.current_war_id > war_id,
        ErrorCode::FactionWarNotSettled
    );
    require_keys_eq!(
        ctx.accounts.rent_payer.key(),
        war_close_state.rent_payer,
        ErrorCode::InvalidCloseState
    );
    require!(
        war_close_state.open_game_session_count == 0,
        ErrorCode::PendingClaimsRemaining
    );
    require!(
        war_close_state.pending_war_claim_count == 0,
        ErrorCode::PendingClaimsRemaining
    );

    let active_factions = war_state.faction_count as usize;
    require!(active_factions <= NUM_FACTIONS, ErrorCode::InvalidState);
    let required_mask: u16 = if active_factions == 0 {
        0
    } else {
        ((1u32 << active_factions) - 1) as u16
    };
    require!(
        (war_settlement.treasury_claimed_bitmap & required_mask) == required_mask,
        ErrorCode::TreasuryClaimsPending
    );

    emit!(FactionWarAccountsClosed {
        war_id,
        rent_payer: ctx.accounts.rent_payer.key(),
        caller: ctx.accounts.caller.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// One-time admin cleanup for legacy `UserGameBet` PDAs that predate close
/// sidecars. This is intentionally not part of the steady-state lifecycle: use
/// it only in the migration upgrade, then remove the instruction.
pub fn admin_close_legacy_user_game_bet_internal(
    ctx: Context<AdminCloseLegacyUserGameBet>,
    round_id: u64,
) -> Result<()> {
    crate::log_fn!("faction_war", "admin_close_legacy_user_game_bet_internal");
    require!(
        ctx.accounts.global_config.ext_authority == ctx.accounts.authority.key(),
        ErrorCode::Unauthorized
    );
    require!(
        ctx.accounts.user_game_bet.round_id == round_id,
        ErrorCode::InvalidRound
    );
    require_keys_eq!(
        ctx.accounts.player_data.owner,
        ctx.accounts.user_game_bet.owner,
        ErrorCode::InvalidOwner
    );

    if ctx.accounts.player_data.pending_round_claims > 0 {
        ctx.accounts.player_data.pending_round_claims = ctx
            .accounts
            .player_data
            .pending_round_claims
            .checked_sub(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    Ok(())
}

/// One-time admin cleanup for legacy `UserFactionWarBets` PDAs that predate
/// close sidecars. Use only during the migration upgrade.
pub fn admin_close_legacy_user_war_bets_internal(
    ctx: Context<AdminCloseLegacyUserWarBets>,
    war_id: u64,
) -> Result<()> {
    crate::log_fn!("faction_war", "admin_close_legacy_user_war_bets_internal");
    require!(
        ctx.accounts.global_config.ext_authority == ctx.accounts.authority.key(),
        ErrorCode::Unauthorized
    );
    require!(
        ctx.accounts.user_war_bets.war_id == war_id,
        ErrorCode::InvalidState
    );
    require_keys_eq!(
        ctx.accounts.player_data.owner,
        ctx.accounts.user_war_bets.owner,
        ErrorCode::InvalidOwner
    );

    if ctx.accounts.player_data.pending_war_claims > 0 {
        ctx.accounts.player_data.pending_war_claims = ctx
            .accounts
            .player_data
            .pending_war_claims
            .checked_sub(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    Ok(())
}

/// One-time admin cleanup for settled legacy faction-war state + settlement
/// PDAs. This bypasses the sidecar counters because legacy cycles did not have
/// them. Only settled historical wars can be swept.
pub fn admin_close_legacy_faction_war_accounts_internal(
    ctx: Context<AdminCloseLegacyFactionWarAccounts>,
    war_id: u64,
) -> Result<()> {
    crate::log_fn!(
        "faction_war",
        "admin_close_legacy_faction_war_accounts_internal"
    );
    require!(
        ctx.accounts.global_config.ext_authority == ctx.accounts.authority.key(),
        ErrorCode::Unauthorized
    );
    require!(
        ctx.accounts.war_state.war_id == war_id,
        ErrorCode::InvalidState
    );
    require!(
        ctx.accounts.war_settlement.war_id == war_id,
        ErrorCode::InvalidState
    );
    require!(
        ctx.accounts.war_state.stage == 1,
        ErrorCode::FactionWarNotSettled
    );
    require!(
        ctx.accounts.war_config.current_war_id > war_id,
        ErrorCode::FactionWarNotSettled
    );

    emit!(FactionWarAccountsClosed {
        war_id,
        rent_payer: ctx.accounts.rent_recipient.key(),
        caller: ctx.accounts.authority.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

pub fn admin_init_legacy_faction_war_close_state_internal(
    ctx: Context<AdminInitLegacyFactionWarCloseState>,
    war_id: u64,
    open_game_session_count: u64,
    pending_war_claim_count: u64,
    rent_payer: Pubkey,
) -> Result<()> {
    crate::log_fn!(
        "faction_war",
        "admin_init_legacy_faction_war_close_state_internal"
    );
    require!(
        ctx.accounts.global_config.ext_authority == ctx.accounts.authority.key(),
        ErrorCode::Unauthorized
    );
    require!(
        ctx.accounts.war_state.war_id == war_id,
        ErrorCode::InvalidState
    );
    require!(
        ctx.accounts.war_settlement.war_id == war_id,
        ErrorCode::InvalidState
    );

    let close_state = &mut ctx.accounts.war_close_state;
    close_state.bump = ctx.bumps.war_close_state;
    close_state.war_id = war_id;
    close_state.rent_payer = rent_payer;
    close_state.open_game_session_count = open_game_session_count;
    close_state.pending_war_claim_count = pending_war_claim_count;

    emit!(FactionWarCloseStateInitialized {
        war_id,
        close_state: close_state.key(),
        rent_payer,
        timestamp: Clock::get()?.unix_timestamp,
    });

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
        init,
        payer = authority,
        space = FactionWarCloseState::LEN,
        seeds = [FACTION_WAR_CLOSE_STATE_SEED, &war_id.to_le_bytes()],
        bump,
    )]
    pub war_close_state: Box<Account<'info, FactionWarCloseState>>,

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

    /// CHECK: Faction-war SOL vault. Source of the undistributed-SOL drain to
    /// sol_treasury at settle. Seeds validated implicitly via the cached bump
    /// on `war_config.rewards_sol_vault_bump` used for the CPI signer.
    #[account(
        mut,
        seeds = [FACTION_WAR_SOL_VAULT_SEED],
        bump = war_config.rewards_sol_vault_bump,
    )]
    pub faction_war_sol_vault: UncheckedAccount<'info>,

    /// CHECK: Protocol SOL treasury. Destination of undistributed-SOL drain.
    /// Same PDA that receives protocol-fee SOL from user bets.
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED],
        bump,
    )]
    pub sol_treasury: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,

    /// Anyone can settle — no authority check needed.
    pub cranker: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(war_id: u64)]
pub struct CloseFactionWarAccounts<'info> {
    #[account(
        seeds = [FACTION_WAR_CONFIG_SEED],
        bump = war_config.bump,
    )]
    pub war_config: Box<Account<'info, FactionWarConfig>>,

    #[account(
        mut,
        close = rent_payer,
        seeds = [FACTION_WAR_STATE_SEED, &war_id.to_le_bytes()],
        bump = war_state.bump,
    )]
    pub war_state: Box<Account<'info, FactionWarState>>,

    #[account(
        mut,
        close = rent_payer,
        seeds = [FACTION_WAR_SETTLEMENT_SEED, &war_id.to_le_bytes()],
        bump = war_settlement.bump,
    )]
    pub war_settlement: Box<Account<'info, FactionWarSettlement>>,

    #[account(
        mut,
        close = rent_payer,
        seeds = [FACTION_WAR_CLOSE_STATE_SEED, &war_id.to_le_bytes()],
        bump = war_close_state.bump,
    )]
    pub war_close_state: Box<Account<'info, FactionWarCloseState>>,

    /// CHECK: Must equal `war_close_state.rent_payer`; handler validates.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,

    #[account(mut)]
    pub caller: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(round_id: u64)]
pub struct AdminCloseLegacyUserGameBet<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, user_game_bet.owner.as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user_game_bet.owner @ ErrorCode::InvalidOwner,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    #[account(
        mut,
        close = rent_recipient,
        seeds = [USER_GAME_BET_SEED, user_game_bet.owner.as_ref(), &round_id.to_le_bytes()],
        bump = user_game_bet.bump,
    )]
    pub user_game_bet: Box<Account<'info, UserGameBet>>,

    /// CHECK: Receives legacy account rent during the one-time admin sweep.
    #[account(mut)]
    pub rent_recipient: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(war_id: u64)]
pub struct AdminCloseLegacyUserWarBets<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, user_war_bets.owner.as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user_war_bets.owner @ ErrorCode::InvalidOwner,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    #[account(
        mut,
        close = rent_recipient,
        seeds = [USER_FACTION_WAR_BETS_SEED, user_war_bets.owner.as_ref(), &war_id.to_le_bytes()],
        bump = user_war_bets.bump,
    )]
    pub user_war_bets: Box<Account<'info, UserFactionWarBets>>,

    /// CHECK: Receives legacy account rent during the one-time admin sweep.
    #[account(mut)]
    pub rent_recipient: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(war_id: u64)]
pub struct AdminCloseLegacyFactionWarAccounts<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        seeds = [FACTION_WAR_CONFIG_SEED],
        bump = war_config.bump,
    )]
    pub war_config: Box<Account<'info, FactionWarConfig>>,

    #[account(
        mut,
        close = rent_recipient,
        seeds = [FACTION_WAR_STATE_SEED, &war_id.to_le_bytes()],
        bump = war_state.bump,
    )]
    pub war_state: Box<Account<'info, FactionWarState>>,

    #[account(
        mut,
        close = rent_recipient,
        seeds = [FACTION_WAR_SETTLEMENT_SEED, &war_id.to_le_bytes()],
        bump = war_settlement.bump,
    )]
    pub war_settlement: Box<Account<'info, FactionWarSettlement>>,

    /// CHECK: Receives legacy account rent during the one-time admin sweep.
    #[account(mut)]
    pub rent_recipient: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(war_id: u64)]
pub struct AdminInitLegacyFactionWarCloseState<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        seeds = [FACTION_WAR_STATE_SEED, &war_id.to_le_bytes()],
        bump = war_state.bump,
    )]
    pub war_state: Box<Account<'info, FactionWarState>>,

    #[account(
        seeds = [FACTION_WAR_SETTLEMENT_SEED, &war_id.to_le_bytes()],
        bump = war_settlement.bump,
    )]
    pub war_settlement: Box<Account<'info, FactionWarSettlement>>,

    #[account(
        init,
        payer = authority,
        space = FactionWarCloseState::LEN,
        seeds = [FACTION_WAR_CLOSE_STATE_SEED, &war_id.to_le_bytes()],
        bump,
    )]
    pub war_close_state: Box<Account<'info, FactionWarCloseState>>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
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
        seeds = [FACTION_WAR_CLOSE_STATE_SEED, &war_id.to_le_bytes()],
        bump = war_close_state.bump,
        constraint = war_close_state.war_id == war_id @ ErrorCode::InvalidCloseState,
    )]
    pub war_close_state: Box<Account<'info, FactionWarCloseState>>,

    /// CHECK: Receives the `UserFactionWarBets` rent refund; handler validates.
    #[account(mut)]
    pub user_war_bet_rent_payer: UncheckedAccount<'info>,

    #[account(
        mut,
        close = user_war_bet_rent_payer,
        seeds = [USER_FACTION_WAR_BETS_SEED, user_war_bets.owner.as_ref(), &war_id.to_le_bytes()],
        bump,
    )]
    pub user_war_bets: Box<Account<'info, UserFactionWarBets>>,

    #[account(
        mut,
        close = user_war_bet_rent_payer,
        seeds = [USER_FACTION_WAR_BET_CLOSE_STATE_SEED, user_war_bets.owner.as_ref(), &war_id.to_le_bytes()],
        bump = user_war_bet_close_state.bump,
        constraint = user_war_bet_close_state.owner == user_war_bets.owner @ ErrorCode::InvalidCloseState,
        constraint = user_war_bet_close_state.war_id == war_id @ ErrorCode::InvalidCloseState,
    )]
    pub user_war_bet_close_state: Box<Account<'info, UserFactionWarBetCloseState>>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, user_war_bets.owner.as_ref()],
        bump,
        constraint = player_data.owner == user_war_bets.owner @ ErrorCode::InvalidOwner,
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
    pub war_sol_vault: UncheckedAccount<'info>,

    /// CHECK: Validated by constraint that player.key() == user_war_bets.owner
    #[account(
        mut,
        constraint = player.key() == user_war_bets.owner @ ErrorCode::InvalidOwner,
    )]
    pub player: AccountInfo<'info>,

    #[account(mut)]
    pub cranker: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // compute_rankings_into
    // ------------------------------------------------------------------------

    #[test]
    fn rankings_basic_order() {
        let mut ranks = [0u8; NUM_FACTIONS];
        let scores = [100u64, 200, 50, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let round_wins = [1u16, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        compute_rankings_into(&scores, &round_wins, 3, &mut ranks).unwrap();
        // Sorted by score desc: 200 (idx 1), 100 (idx 0), 50 (idx 2)
        assert_eq!(ranks[0], 1);
        assert_eq!(ranks[1], 0);
        assert_eq!(ranks[2], 2);
    }

    #[test]
    fn rankings_tiebreak_by_round_wins() {
        let mut ranks = [0u8; NUM_FACTIONS];
        let scores = [100u64, 100, 100, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let round_wins = [1u16, 3, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        compute_rankings_into(&scores, &round_wins, 3, &mut ranks).unwrap();
        assert_eq!(ranks[0], 2);
        assert_eq!(ranks[1], 0);
        assert_eq!(ranks[2], 1);
    }

    #[test]
    fn rankings_tiebreak_by_faction_id() {
        let mut ranks = [0u8; NUM_FACTIONS];
        let scores = [100u64; NUM_FACTIONS];
        let round_wins = [1u16; NUM_FACTIONS];
        compute_rankings_into(&scores, &round_wins, 3, &mut ranks).unwrap();
        assert_eq!(ranks[0], 0);
        assert_eq!(ranks[1], 1);
        assert_eq!(ranks[2], 2);
    }

    #[test]
    fn rankings_handles_scores_above_i64_max() {
        let mut ranks = [0u8; NUM_FACTIONS];
        let scores = [
            i64::MAX as u64 + 1,
            u64::MAX - 1,
            u64::MAX,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let round_wins = [0u16; NUM_FACTIONS];
        compute_rankings_into(&scores, &round_wins, 3, &mut ranks).unwrap();
        assert_eq!(ranks[0], 2);
        assert_eq!(ranks[1], 1);
        assert_eq!(ranks[2], 0);
    }

    // ------------------------------------------------------------------------
    // resolve_direction_from_ranks
    // ------------------------------------------------------------------------

    #[test]
    fn direction_up() {
        let (dir, delta) = resolve_direction_from_ranks(2, 0);
        assert_eq!(dir, PredictionDirection::Up);
        assert_eq!(delta, 2);
    }

    #[test]
    fn direction_down() {
        let (dir, delta) = resolve_direction_from_ranks(0, 2);
        assert_eq!(dir, PredictionDirection::Down);
        assert_eq!(delta, -2);
    }

    #[test]
    fn direction_neutral() {
        let (dir, delta) = resolve_direction_from_ranks(1, 1);
        assert_eq!(dir, PredictionDirection::Neutral);
        assert_eq!(delta, 0);
    }

    // ------------------------------------------------------------------------
    // apply_mining_multiplier
    // ------------------------------------------------------------------------

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

    #[test]
    fn mining_multiplier_exact() {
        assert_eq!(
            apply_mining_multiplier(1_000_000, 10_000).unwrap(),
            1_000_000
        );
        assert_eq!(apply_mining_multiplier(1_000_000, 5_000).unwrap(), 500_000);
    }

    #[test]
    fn mining_multiplier_zero_input() {
        assert_eq!(apply_mining_multiplier(0, 15_000).unwrap(), 0);
    }

    // ------------------------------------------------------------------------
    // mutation_type_to_u8
    // ------------------------------------------------------------------------

    #[test]
    fn mutation_type_mapping() {
        assert_eq!(mutation_type_to_u8(MutationType::Evolution), 1);
        assert_eq!(mutation_type_to_u8(MutationType::Power), 2);
        assert_eq!(mutation_type_to_u8(MutationType::Trait), 3);
    }

    // ------------------------------------------------------------------------
    // checked_bps_mul
    // ------------------------------------------------------------------------

    #[test]
    fn bps_mul_basic() {
        assert_eq!(checked_bps_mul(10_000, 5_000), 5_000);
        assert_eq!(checked_bps_mul(10_000, 10_000), 10_000);
        assert_eq!(checked_bps_mul(0, 10_000), 0);
    }

    // ------------------------------------------------------------------------
    // pool_share_from_bps
    // ------------------------------------------------------------------------

    #[test]
    fn pool_share_basic() {
        assert_eq!(pool_share_from_bps(1_000_000, 5_000).unwrap(), 500_000);
        assert_eq!(pool_share_from_bps(1_000_000, 10_000).unwrap(), 1_000_000);
        assert_eq!(pool_share_from_bps(0, 5_000).unwrap(), 0);
    }

    // ------------------------------------------------------------------------
    // distribute_rank_weighted_absolute
    // ------------------------------------------------------------------------

    #[test]
    fn distribute_all_eligible() {
        let mut pools = [0u64; NUM_FACTIONS];
        let final_ranks = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let eligible = [true; NUM_FACTIONS];
        let residual =
            distribute_rank_weighted_absolute(10_000, &final_ranks, &eligible, 12, &mut pools)
                .unwrap();
        assert_eq!(residual + pools.iter().sum::<u64>(), 10_000);
        assert!(pools[0] > pools[1]); // rank 0 > rank 1
        assert!(pools[1] > pools[11]); // rank 1 > rank 11
    }

    #[test]
    fn distribute_some_ineligible() {
        let mut pools = [0u64; NUM_FACTIONS];
        let final_ranks = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let mut eligible = [false; NUM_FACTIONS];
        eligible[0] = true;
        eligible[1] = true;
        let residual =
            distribute_rank_weighted_absolute(10_000, &final_ranks, &eligible, 12, &mut pools)
                .unwrap();
        assert!(residual > 0);
        assert_eq!(pools[0] + pools[1] + residual, 10_000);
        assert!(pools.iter().skip(2).all(|&p| p == 0));
    }

    #[test]
    fn distribute_zero_pool() {
        let mut pools = [0u64; NUM_FACTIONS];
        let eligible = [true; NUM_FACTIONS];
        let residual =
            distribute_rank_weighted_absolute(0, &[0u8; NUM_FACTIONS], &eligible, 12, &mut pools)
                .unwrap();
        assert_eq!(residual, 0);
        assert!(pools.iter().all(|&p| p == 0));
    }

    // ------------------------------------------------------------------------
    // mvp_rank_weight_bps
    // ------------------------------------------------------------------------

    #[test]
    fn mvp_weights_top_ranks() {
        assert_eq!(mvp_rank_weight_bps(0, 12), 3000);
        assert_eq!(mvp_rank_weight_bps(1, 12), 2000);
        assert_eq!(mvp_rank_weight_bps(2, 12), 1400);
        assert_eq!(mvp_rank_weight_bps(3, 12), 1000);
        assert_eq!(mvp_rank_weight_bps(4, 12), 800);
        assert_eq!(mvp_rank_weight_bps(5, 12), 600);
        assert_eq!(mvp_rank_weight_bps(6, 12), 500);
        assert_eq!(mvp_rank_weight_bps(7, 12), 400);
    }

    #[test]
    fn mvp_weights_tail_split() {
        let tail_count = 12usize.saturating_sub(8).max(1) as u64;
        assert_eq!(mvp_rank_weight_bps(8, 12), 300 / tail_count);
        assert_eq!(mvp_rank_weight_bps(11, 12), 300 / tail_count);
    }

    #[test]
    fn mvp_weights_small_faction_count() {
        assert_eq!(mvp_rank_weight_bps(0, 4), 3000);
        assert_eq!(mvp_rank_weight_bps(3, 4), 1000); // rank 3 is explicit, not tail
    }

    // ------------------------------------------------------------------------
    // total_cycle_weighted_volume
    // ------------------------------------------------------------------------

    #[test]
    fn cycle_volume_sums_all_directions() {
        let mut war_state = FactionWarState::blank();
        war_state.faction_direction_totals[0][0] = 100;
        war_state.faction_direction_totals[0][1] = 200;
        war_state.faction_direction_totals[0][2] = 300;
        war_state.faction_direction_totals[1][0] = 400;
        assert_eq!(total_cycle_weighted_volume(&war_state, 2), 1000);
    }

    #[test]
    fn cycle_volume_saturation() {
        let mut war_state = FactionWarState::blank();
        war_state.faction_direction_totals[0][0] = u64::MAX;
        war_state.faction_direction_totals[0][1] = 1;
        let vol = total_cycle_weighted_volume(&war_state, 1);
        assert_eq!(vol, u64::MAX);
    }

    // ------------------------------------------------------------------------
    // user_correct_cycle_sol
    // ------------------------------------------------------------------------

    #[test]
    fn user_correct_sol_only_resolved_directions() {
        let mut user_bets = UserFactionWarBets::blank();
        user_bets.sol_direction_bets[0][0] = 100; // Down
        user_bets.sol_direction_bets[0][1] = 200; // Neutral
        user_bets.sol_direction_bets[0][2] = 300; // Up
        user_bets.sol_direction_bets[1][0] = 400; // Down

        let mut settlement = FactionWarSettlement::blank();
        settlement.resolved_directions[0] = PredictionDirection::Up.as_index() as u8; // 2
        settlement.resolved_directions[1] = PredictionDirection::Down.as_index() as u8; // 0

        assert_eq!(user_correct_cycle_sol(&user_bets, &settlement, 2), 700); // 300 + 400
    }

    #[test]
    fn user_correct_sol_ignores_invalid_direction() {
        let mut user_bets = UserFactionWarBets::blank();
        user_bets.sol_direction_bets[0][2] = 300;

        let mut settlement = FactionWarSettlement::blank();
        settlement.resolved_directions[0] = 255; // invalid

        assert_eq!(user_correct_cycle_sol(&user_bets, &settlement, 1), 0);
    }

    // ------------------------------------------------------------------------
    // compute_base_reward_pools
    // ------------------------------------------------------------------------

    #[test]
    fn base_reward_pools_sums_correctly() {
        let mut war_state = FactionWarState::blank();
        war_state.faction_count = 3;
        war_state.dbtc_mined_this_war = 1_000_000;
        war_state.sol_reward_pool = 100_000;
        war_state.faction_direction_totals[0][2] = 100; // eligible base
        war_state.faction_direction_totals[1][1] = 200; // eligible base
        war_state.faction_mutation_score[0] = 50; // eligible HB

        let mut settlement = FactionWarSettlement::blank();
        settlement.final_ranks = [0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        settlement.resolved_directions[0] = PredictionDirection::Up.as_index() as u8;
        settlement.resolved_directions[1] = PredictionDirection::Neutral.as_index() as u8;

        let tuning = GameplayTuningConfig {
            rpg_progression: true,
            max_evolution_stage_unlocked: 3,
            war_base_reward_bps: 5000,
            war_mvp_reward_bps: 2000,
            war_hashbeast_reward_bps: 3000,
            base_mutation_chance_bps: 100,
            mutation_chance_floor_bps: 50,
            mutation_chance_cap_bps: 5000,
            faction_volume_threshold_lamports: 1_000_000,
            extra_volume_threshold_per_mutation_lamports: 100_000,
            target_mutations_per_cycle: 10,
            target_rounds_per_cycle: 20,
            pacing_max_adjustment_bps: 2000,
        };

        let residual = compute_base_reward_pools(&war_state, &mut settlement, &tuning).unwrap();

        let base_total: u64 = settlement.base_reward_pools.iter().sum();
        let hb_total: u64 = settlement.hashbeast_reward_pools.iter().sum();

        assert!(base_total <= 500_000);
        assert!(hb_total <= 300_000);
        assert_eq!(
            settlement.sol_base_pool + settlement.sol_hb_pool + settlement.sol_mvp_pool + residual,
            100_000
        );
    }

    #[test]
    fn base_reward_pools_zero_pool_returns_zero() {
        let mut war_state = FactionWarState::blank();
        war_state.faction_count = 3;
        let mut settlement = FactionWarSettlement::blank();
        let tuning = default_test_tuning();
        let residual = compute_base_reward_pools(&war_state, &mut settlement, &tuning).unwrap();
        assert_eq!(residual, 0);
        assert!(settlement.base_reward_pools.iter().all(|&p| p == 0));
        assert!(settlement.hashbeast_reward_pools.iter().all(|&p| p == 0));
        assert_eq!(settlement.sol_base_pool, 0);
        assert_eq!(settlement.sol_hb_pool, 0);
        assert_eq!(settlement.sol_mvp_pool, 0);
    }

    /// Regression: when dBTC pool is 0 but SOL pool > 0, the claim path's
    /// `scale_sol_lane` divides by `total_dbtc_<lane>` (= 0) and the guard
    /// returns 0 for every user. Without the early-drain, SOL would sit
    /// stranded on `sol_*_pool` settlement fields with no one able to claim.
    /// Expected: full SOL pool flows back as residual → drained to treasury
    /// by settle_war_internal.
    #[test]
    fn base_reward_pools_zero_dbtc_nonzero_sol_drains_all_sol() {
        let mut war_state = FactionWarState::blank();
        war_state.faction_count = 3;
        war_state.dbtc_mined_this_war = 0;
        war_state.sol_reward_pool = 500_000;
        war_state.faction_direction_totals[0][2] = 100; // would be base-eligible
        war_state.faction_mutation_score[0] = 50; // would be HB-eligible

        let mut settlement = FactionWarSettlement::blank();
        settlement.final_ranks = [0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        settlement.resolved_directions[0] = PredictionDirection::Up.as_index() as u8;

        let tuning = default_test_tuning();
        let residual = compute_base_reward_pools(&war_state, &mut settlement, &tuning).unwrap();

        assert_eq!(residual, 500_000, "full SOL pool must flow to residual");
        assert_eq!(settlement.sol_base_pool, 0);
        assert_eq!(settlement.sol_hb_pool, 0);
        assert_eq!(settlement.sol_mvp_pool, 0);
        assert!(settlement.base_reward_pools.iter().all(|&p| p == 0));
        assert!(settlement.hashbeast_reward_pools.iter().all(|&p| p == 0));
    }

    #[test]
    fn base_reward_pools_drain_sol_lane_when_dbtc_lane_rounds_to_zero() {
        let mut war_state = FactionWarState::blank();
        war_state.faction_count = 3;
        war_state.dbtc_mined_this_war = 1;
        war_state.sol_reward_pool = 1_000_000;
        war_state.faction_direction_totals[0][2] = 100;

        let mut settlement = FactionWarSettlement::blank();
        settlement.final_ranks = [0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        settlement.resolved_directions[0] = PredictionDirection::Up.as_index() as u8;

        let tuning = default_test_tuning();
        let residual = compute_base_reward_pools(&war_state, &mut settlement, &tuning).unwrap();

        assert_eq!(settlement.base_reward_pools.iter().sum::<u64>(), 0);
        assert_eq!(settlement.sol_base_pool, 0);
        assert_eq!(settlement.sol_hb_pool, 0);
        assert_eq!(settlement.sol_mvp_pool, 200_000);
        assert_eq!(residual, 800_000);
    }

    #[test]
    fn mvp_sol_drains_when_mvp_dbtc_rounds_to_zero() {
        let mut mvp_user = no_mvps();
        mvp_user[0] = user(42);
        let final_ranks = [0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        let (eligible, residual) =
            split_sol_mvp_residual_with_dbtc_guard(200_000, 0, &mvp_user, &final_ranks, 3).unwrap();

        assert_eq!(eligible, 0);
        assert_eq!(residual, 200_000);
    }

    /// Per-faction non-eligibility produces a SOL residual.
    /// Setup: 3 active factions, only faction 0 has correct-direction bets
    /// (base-eligible). Factions 1 and 2's rank-weighted base shares should
    /// stay unallocated → end up in residual.
    #[test]
    fn base_reward_pools_partial_eligibility_residual() {
        let mut war_state = FactionWarState::blank();
        war_state.faction_count = 3;
        war_state.dbtc_mined_this_war = 1_000_000;
        war_state.sol_reward_pool = 1_000_000;
        war_state.faction_direction_totals[0][2] = 100; // only faction 0 is base-eligible
        war_state.faction_mutation_score[0] = 50;
        war_state.faction_mutation_score[1] = 50;
        war_state.faction_mutation_score[2] = 50;

        let mut settlement = FactionWarSettlement::blank();
        settlement.final_ranks = [0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        settlement.resolved_directions[0] = PredictionDirection::Up.as_index() as u8;

        let tuning = default_test_tuning();
        let residual = compute_base_reward_pools(&war_state, &mut settlement, &tuning).unwrap();

        // Faction 0 should have a non-zero base pool (it's the only eligible).
        assert!(settlement.base_reward_pools[0] > 0);
        assert_eq!(settlement.base_reward_pools[1], 0);
        assert_eq!(settlement.base_reward_pools[2], 0);
        // SOL base also goes only to faction 0; the rest is residual.
        let sol_total_distributed =
            settlement.sol_base_pool + settlement.sol_hb_pool + settlement.sol_mvp_pool;
        assert_eq!(sol_total_distributed + residual, 1_000_000);
        // Some residual must exist because not all factions are base-eligible.
        assert!(residual > 0);
    }

    /// `distribute_rank_weighted_absolute` must conserve lamports:
    /// `sum(pools_out) + residual == pool_total` for any input.
    #[test]
    fn distribute_conserves_lamports_under_rounding() {
        let mut pools = [0u64; NUM_FACTIONS];
        let final_ranks = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let mut eligible = [false; NUM_FACTIONS];
        eligible[0] = true;
        eligible[3] = true;
        eligible[7] = true;
        eligible[11] = true;
        // Pick a value that doesn't divide cleanly by the weight sum.
        let pool: u64 = 123_457;
        let residual =
            distribute_rank_weighted_absolute(pool, &final_ranks, &eligible, 12, &mut pools)
                .unwrap();
        let allocated: u64 = pools.iter().sum();
        assert_eq!(allocated + residual, pool, "lamports must be conserved");
        // Only eligibles got non-zero.
        for (fid, p) in pools.iter().enumerate() {
            if eligible[fid] {
                assert!(*p > 0, "eligible faction {} got 0", fid);
            } else {
                assert_eq!(*p, 0, "non-eligible faction {} got non-zero", fid);
            }
        }
    }

    /// When NO factions are eligible (e.g. nobody had a winning direction OR
    /// any mutations), the entire pool is returned as residual.
    #[test]
    fn distribute_zero_eligibles_residual_is_full_pool() {
        let mut pools = [0u64; NUM_FACTIONS];
        let final_ranks = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let eligible = [false; NUM_FACTIONS];
        let residual =
            distribute_rank_weighted_absolute(50_000, &final_ranks, &eligible, 12, &mut pools)
                .unwrap();
        assert_eq!(residual, 50_000);
        assert!(pools.iter().all(|&p| p == 0));
    }

    /// MVP rank-weight curve sums to 10_000 bps across 12 factions.
    /// This is what makes the absolute distribution math line up.
    #[test]
    fn mvp_weights_sum_to_10000_at_full_field() {
        let total: u64 = (0..12).map(|r| mvp_rank_weight_bps(r, 12)).sum();
        assert_eq!(
            total, 10_000,
            "MVP weights should sum to 100% at 12 factions"
        );
    }

    /// MVP rank-weight curve at smaller fields uses explicit weights for the
    /// top 8 ranks. When `active_factions < 8`, ranks 8+ are not reachable, so
    /// only the explicit slice matters — verify a few sums.
    #[test]
    fn mvp_weights_partial_field_uses_explicit_weights() {
        // 4 factions: top 4 explicit = 3000+2000+1400+1000 = 7400
        let total: u64 = (0..4).map(|r| mvp_rank_weight_bps(r, 4)).sum();
        assert_eq!(total, 7400);
        // 8 factions: top 8 explicit = 9700
        let total: u64 = (0..8).map(|r| mvp_rank_weight_bps(r, 8)).sum();
        assert_eq!(total, 9700);
    }

    /// `FACTION_WAR_RANK_WEIGHT_BPS` curve sums consistently — used as the
    /// denominator for base + HB rank-weighted distribution.
    #[test]
    fn faction_war_rank_weights_consistent() {
        let total: u64 = FACTION_WAR_RANK_WEIGHT_BPS.iter().map(|&w| w as u64).sum();
        // Just verify it's a sensible positive number; the constant is the
        // source of truth — this guards against accidental zeroing.
        assert!(total > 0, "rank weights should sum > 0");
        // Monotonic decreasing (or non-increasing): rank 0 ≥ rank 1 ≥ ... ≥ rank 11.
        for window in FACTION_WAR_RANK_WEIGHT_BPS.windows(2) {
            assert!(
                window[0] >= window[1],
                "weights must be monotonic non-increasing: {} < {}",
                window[0],
                window[1]
            );
        }
    }

    // ------------------------------------------------------------------------
    // distribute_mvp_pool — dBTC MVP lane
    // ------------------------------------------------------------------------

    fn user(seed: u8) -> Pubkey {
        Pubkey::new_from_array([seed; 32])
    }

    fn no_mvps() -> [Pubkey; NUM_FACTIONS] {
        [Pubkey::default(); NUM_FACTIONS]
    }

    /// 4 factions, all have MVPs at ranks 0..3. Expected slice ratios:
    ///   rank 0 = 3000, rank 1 = 2000, rank 2 = 1400, rank 3 = 1000 bps
    ///   total weight = 7400; eligible_weight = 7400 → target = full pool
    /// Lamports must sum to exactly the pool (last eligible absorbs rounding).
    #[test]
    fn mvp_distribute_all_eligible_top_4() {
        let pool: u64 = 740_000;
        let mut mvp_user = no_mvps();
        for (fid, slot) in mvp_user.iter_mut().enumerate().take(4) {
            *slot = user(fid as u8 + 1);
        }
        // Final ranks: faction id == rank for simplicity
        let final_ranks = [0, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut bonuses = [0u64; NUM_FACTIONS];
        let residual = distribute_mvp_pool(pool, &mvp_user, &final_ranks, 4, &mut bonuses).unwrap();
        // No residual since all 4 ranks are eligible (target == pool when
        // eligible_weight == total_weight).
        assert_eq!(residual, 0);
        let sum: u64 = bonuses.iter().sum();
        assert_eq!(sum, pool);
        // Ratios should follow rank weights (with some rounding tolerance):
        //   #1 ≈ 30%, #2 ≈ 20%, #3 ≈ 14%, #4 ≈ 10% of pool
        // Pool = 740_000 → #1 ≈ 300_000, #2 ≈ 200_000, #3 ≈ 140_000, #4 ≈ 100_000
        assert!(bonuses[0] >= 299_000 && bonuses[0] <= 301_000);
        assert!(bonuses[1] >= 199_000 && bonuses[1] <= 201_000);
        assert!(bonuses[2] >= 139_000 && bonuses[2] <= 141_000);
        // bonuses[3] absorbs rounding, so check vs the target it's been given:
        // exact value depends on iteration order — just verify it's the
        // remainder of pool - others.
        assert_eq!(bonuses[3], pool - (bonuses[0] + bonuses[1] + bonuses[2]));
        assert!(bonuses[0] > bonuses[1]);
        assert!(bonuses[1] > bonuses[2]);
    }

    /// No MVPs anywhere → full pool flows back as residual, bonuses all zero.
    #[test]
    fn mvp_distribute_no_eligibles_returns_full_residual() {
        let mut bonuses = [0u64; NUM_FACTIONS];
        let final_ranks = [0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let residual =
            distribute_mvp_pool(500_000, &no_mvps(), &final_ranks, 3, &mut bonuses).unwrap();
        assert_eq!(residual, 500_000);
        assert!(bonuses.iter().all(|&b| b == 0));
    }

    /// Only some factions have MVPs (ranks 0 and 5 of 12). Eligible weight =
    /// 3000 + 600 = 3600 bps. Total weight = 10_000 (full curve). Target =
    /// pool × 3600 / 10_000 = 36% of pool. The rest stays as residual.
    #[test]
    fn mvp_distribute_partial_eligibles_residual() {
        let mut mvp_user = no_mvps();
        mvp_user[0] = user(1);
        mvp_user[5] = user(2);
        let final_ranks = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let mut bonuses = [0u64; NUM_FACTIONS];
        let pool: u64 = 1_000_000;
        let residual =
            distribute_mvp_pool(pool, &mvp_user, &final_ranks, 12, &mut bonuses).unwrap();
        // Lamports conserved.
        assert_eq!(bonuses.iter().sum::<u64>() + residual, pool);
        // Eligibles got non-zero, others zero.
        assert!(bonuses[0] > 0);
        assert!(bonuses[5] > 0);
        for fid in [1usize, 2, 3, 4, 6, 7, 8, 9, 10, 11] {
            assert_eq!(bonuses[fid], 0);
        }
        // Target eligible share = pool × 3600 / 10000 = 360_000.
        // Residual ≈ 640_000 (with up to ±1 rounding).
        let allocated: u64 = bonuses.iter().sum();
        assert!((359_999..=360_001).contains(&allocated));
        assert!((639_999..=640_001).contains(&residual));
        // rank 0 > rank 5 (#1 weight > #5 weight)
        assert!(bonuses[0] > bonuses[5]);
    }

    /// Zero pool → zero everything, residual zero.
    #[test]
    fn mvp_distribute_zero_pool() {
        let mut mvp_user = no_mvps();
        mvp_user[0] = user(1);
        let final_ranks = [0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut bonuses = [0u64; NUM_FACTIONS];
        let residual = distribute_mvp_pool(0, &mvp_user, &final_ranks, 3, &mut bonuses).unwrap();
        assert_eq!(residual, 0);
        assert!(bonuses.iter().all(|&b| b == 0));
    }

    /// Rank 0's MVP gets the biggest slice (regression: #1 must dominate).
    #[test]
    fn mvp_distribute_rank_zero_gets_most() {
        let mut mvp_user = no_mvps();
        for (fid, slot) in mvp_user.iter_mut().enumerate().take(3) {
            *slot = user(fid as u8 + 1);
        }
        let final_ranks = [0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut bonuses = [0u64; NUM_FACTIONS];
        distribute_mvp_pool(1_000_000, &mvp_user, &final_ranks, 3, &mut bonuses).unwrap();
        assert!(bonuses[0] > bonuses[1]);
        assert!(bonuses[1] > bonuses[2]);
    }

    /// Faction id ≠ rank: if rank 0 belongs to faction 7, faction 7's bonus
    /// should be the biggest.
    #[test]
    fn mvp_distribute_uses_final_rank_not_faction_id() {
        let mut mvp_user = no_mvps();
        mvp_user[7] = user(1); // rank 0
        mvp_user[3] = user(2); // rank 1
        let mut final_ranks = [0u8; NUM_FACTIONS];
        final_ranks[7] = 0;
        final_ranks[3] = 1;
        // others at rank 2..
        for (fid, rank) in final_ranks.iter_mut().enumerate().take(NUM_FACTIONS) {
            if fid != 7 && fid != 3 {
                *rank = (fid as u8 + 2).min(11);
            }
        }
        let mut bonuses = [0u64; NUM_FACTIONS];
        distribute_mvp_pool(1_000_000, &mvp_user, &final_ranks, 12, &mut bonuses).unwrap();
        assert!(
            bonuses[7] > bonuses[3],
            "rank-0 faction should outpay rank-1"
        );
    }

    // ------------------------------------------------------------------------
    // split_sol_mvp_residual — SOL MVP lane
    // ------------------------------------------------------------------------

    #[test]
    fn split_sol_mvp_all_eligible_no_residual() {
        let mut mvp_user = no_mvps();
        for (fid, slot) in mvp_user.iter_mut().enumerate().take(4) {
            *slot = user(fid as u8 + 1);
        }
        let final_ranks = [0, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0];
        let (eligible, residual) =
            split_sol_mvp_residual(740_000, &mvp_user, &final_ranks, 4).unwrap();
        // All MVPs eligible → eligible_weight == total_weight → eligible_share == pool.
        assert_eq!(eligible, 740_000);
        assert_eq!(residual, 0);
    }

    #[test]
    fn split_sol_mvp_no_eligibles_drains_all() {
        let final_ranks = [0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let (eligible, residual) =
            split_sol_mvp_residual(500_000, &no_mvps(), &final_ranks, 3).unwrap();
        assert_eq!(eligible, 0);
        assert_eq!(residual, 500_000);
    }

    #[test]
    fn split_sol_mvp_partial_eligibles_conserves() {
        let mut mvp_user = no_mvps();
        mvp_user[0] = user(1);
        mvp_user[5] = user(2);
        let final_ranks = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let (eligible, residual) =
            split_sol_mvp_residual(1_000_000, &mvp_user, &final_ranks, 12).unwrap();
        // Lamports conserved.
        assert_eq!(eligible + residual, 1_000_000);
        // Eligible weight = 3000 + 600 = 3600 of 10_000 total → ~36%.
        assert!((359_999..=360_001).contains(&eligible));
    }

    #[test]
    fn split_sol_mvp_zero_pool() {
        let mut mvp_user = no_mvps();
        mvp_user[0] = user(1);
        let final_ranks = [0u8; NUM_FACTIONS];
        let (eligible, residual) = split_sol_mvp_residual(0, &mvp_user, &final_ranks, 12).unwrap();
        assert_eq!(eligible, 0);
        assert_eq!(residual, 0);
    }

    // ------------------------------------------------------------------------
    // scale_user_sol_lane — per-user SOL scaling at claim time
    // ------------------------------------------------------------------------

    #[test]
    fn scale_user_sol_basic_proportional() {
        // sol_lane_pool = 100, user_dbtc = 250, total_dbtc = 1000
        // user_sol = 100 × 250 / 1000 = 25
        assert_eq!(scale_user_sol_lane(100, 250, 1000).unwrap(), 25);
    }

    #[test]
    fn scale_user_sol_user_gets_full_share() {
        // User has 100% of the dBTC lane → 100% of the SOL lane.
        assert_eq!(scale_user_sol_lane(500, 1000, 1000).unwrap(), 500);
    }

    #[test]
    fn scale_user_sol_zero_inputs_return_zero() {
        // Each zero short-circuits to 0; prevents div-by-zero.
        assert_eq!(scale_user_sol_lane(0, 100, 1000).unwrap(), 0);
        assert_eq!(scale_user_sol_lane(100, 0, 1000).unwrap(), 0);
        assert_eq!(scale_user_sol_lane(100, 100, 0).unwrap(), 0);
    }

    #[test]
    fn scale_user_sol_truncates_down() {
        // 100 × 1 / 3 = 33.33 → 33 (integer division truncates)
        assert_eq!(scale_user_sol_lane(100, 1, 3).unwrap(), 33);
    }

    #[test]
    fn scale_user_sol_large_values_use_u128() {
        // u64::MAX × u64::MAX would overflow u64 but fits in u128 intermediate.
        // Use a realistic large value to verify no overflow.
        let big_pool = 1_000_000_000_000u64; // 1000 SOL
        let user_share = 1_000_000_000u64; // 1 SOL of dBTC
        let total_share = 1_000_000_000_000u64; // 1000 SOL of dBTC
                                                // 1000 SOL × 1/1000 = 1 SOL = 1e9 lamports
        assert_eq!(
            scale_user_sol_lane(big_pool, user_share, total_share).unwrap(),
            1_000_000_000
        );
    }

    /// Three users splitting a SOL pool by their dBTC shares should sum
    /// (modulo rounding) to ≤ the pool — never over.
    #[test]
    fn scale_user_sol_sum_of_shares_never_exceeds_pool() {
        let sol_pool: u64 = 1_000_000;
        let total_dbtc: u64 = 100_000;
        let user_a_dbtc: u64 = 33_333;
        let user_b_dbtc: u64 = 33_333;
        let user_c_dbtc: u64 = 33_334; // sums to 100_000

        let sol_a = scale_user_sol_lane(sol_pool, user_a_dbtc, total_dbtc).unwrap();
        let sol_b = scale_user_sol_lane(sol_pool, user_b_dbtc, total_dbtc).unwrap();
        let sol_c = scale_user_sol_lane(sol_pool, user_c_dbtc, total_dbtc).unwrap();
        let sum = sol_a + sol_b + sol_c;
        assert!(
            sum <= sol_pool,
            "summed shares ({}) must not exceed pool ({})",
            sum,
            sol_pool
        );
        // Drift should be tiny (sub-lamport per user).
        assert!(sol_pool - sum <= 3);
    }

    /// Shared tuning fixture for tests that don't care about specific values.
    fn default_test_tuning() -> GameplayTuningConfig {
        GameplayTuningConfig {
            rpg_progression: true,
            max_evolution_stage_unlocked: 3,
            war_base_reward_bps: 5000,
            war_mvp_reward_bps: 2000,
            war_hashbeast_reward_bps: 3000,
            base_mutation_chance_bps: 100,
            mutation_chance_floor_bps: 50,
            mutation_chance_cap_bps: 5000,
            faction_volume_threshold_lamports: 1_000_000,
            extra_volume_threshold_per_mutation_lamports: 100_000,
            target_mutations_per_cycle: 10,
            target_rounds_per_cycle: 20,
            pacing_max_adjustment_bps: 2000,
        }
    }
}
