use anchor_lang::prelude::*;

use crate::errors::ErrorCode;
use crate::events::*;
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

/// Apply the dynamic mining multiplier to the raw dogeBTC mined in a cycle.
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
    for faction_id in ordered_factions
        .iter()
        .take(active_factions)
        .copied()
    {
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

    for faction_id in ordered_factions
        .iter()
        .take(active_factions)
        .copied()
    {
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
/// - Doge rewards: only mutated/evolved gameplay doges on the resolved home-country outcome
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
    // it into the base pool instead of stranding the dogeBTC in the mining
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
    faction_war_config.prev_faction_war_mutation_ranks = initial_ranks;
    msg!("🏆 [faction_war.initialize_faction_war_config_internal] prev_faction_war_mutation_ranks={:?}", initial_ranks);
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
        "⚔️ [faction_war.finalize_faction_war_settlement] FactionWar #{}, {} factions, {} dogeBTC mined",
        faction_war_state.faction_war_id,
        active_factions,
        faction_war_state.total_dogebtc_mined_in_faction_war
    );

    let total_mutation_score: u64 = faction_war_state
        .faction_mutation_scores
        .iter()
        .take(active_factions)
        .sum();
    msg!(
        "⚔️ [faction_war.finalize_faction_war_settlement] total_mutation_score={}",
        total_mutation_score
    );

    if total_mutation_score == 0 {
        // Distinguish "faction war never got real gameplay" from
        // "real gameplay happened, ranks just didn't move this cycle".
        //   - No own-faction bets OR rpg_progression disabled → treat as
        //     non-operational: roll treasury forward to the next war (like
        //     the active_factions == 0 branch above) and settle empty.
        //   - Bets + progression enabled, just zero mutations → ranks stay
        //     equal to start_ranks (Neutral wins every faction). Distribute
        //     the mining pool to users who bet Neutral on their faction.
        let has_bets = faction_war_state
            .faction_direction_totals
            .iter()
            .take(active_factions)
            .any(|row| row.iter().any(|&v| v > 0));
        msg!("⚔️ [faction_war.finalize_faction_war_settlement] total_mutation_score==0 has_bets={} rpg_progression={}", has_bets, tuning.rpg_progression);

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
            msg!(
                "⚔️ [faction_war.finalize_faction_war_settlement] faction_war_mining_pool reset=0"
            );
            faction_war_state.faction_reward_pools = [0u64; NUM_FACTIONS];
            faction_war_state.loyalty_reward_pools = [0u64; NUM_FACTIONS];
            faction_war_state.faction_doge_reward_pools = [0u64; NUM_FACTIONS];
            msg!("⚔️ [faction_war.finalize_faction_war_settlement] reward pools zeroed");
            faction_war_state.stage = 1;
            msg!("⚔️ [faction_war.finalize_faction_war_settlement] stage mutated: -> 1");
        } else {
            let old_pool = faction_war_state.faction_war_mining_pool;
            faction_war_state.faction_war_mining_pool = apply_mining_multiplier(
                faction_war_state.total_dogebtc_mined_in_faction_war,
                faction_war_config.mining_multiplier_bps,
            )?;
            let _ = old_pool;
            // No rank change: final_ranks = start_ranks, deltas all zero, every
            // faction resolves to Neutral.
            faction_war_state.final_ranks = faction_war_state.start_ranks;
            faction_war_state.rank_deltas = [0i8; NUM_FACTIONS];
            for i in 0..active_factions {
                faction_war_state.resolved_directions[i] =
                    PredictionDirection::Neutral.as_index() as u8;
            }
            compute_faction_reward_pools(faction_war_state, tuning)?;
            faction_war_state.stage = 1;
            msg!("⚔️ [faction_war.finalize_faction_war_settlement] stage mutated: -> 1");
        }
    } else {
        let old_pool = faction_war_state.faction_war_mining_pool;
        faction_war_state.faction_war_mining_pool = apply_mining_multiplier(
            faction_war_state.total_dogebtc_mined_in_faction_war,
            faction_war_config.mining_multiplier_bps,
        )?;
        let _ = old_pool;

        let mut mutation_scores_i64 = [0i64; NUM_FACTIONS];
        for (i, score) in mutation_scores_i64
            .iter_mut()
            .enumerate()
            .take(active_factions)
        {
            *score = faction_war_state.faction_mutation_scores[i] as i64;
        }
        let mut final_ranks = faction_war_state.final_ranks;
        compute_rankings_into(
            &mutation_scores_i64,
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

        faction_war_config.prev_faction_war_mutation_ranks = final_ranks;
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
    msg!("⚔️ [faction_war.settle_faction_war_internal] emitting FactionWarSettled: faction_war_id={} total_dogebtc_mined={} mining_pool={} timestamp={}",
        faction_war_state.faction_war_id,
        faction_war_state.total_dogebtc_mined_in_faction_war,
        faction_war_state.faction_war_mining_pool,
        clock.unix_timestamp
    );
    emit!(FactionWarSettled {
        faction_war_id: faction_war_state.faction_war_id,
        total_dogebtc_mined: faction_war_state.total_dogebtc_mined_in_faction_war,
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
        faction_mutation_scores: faction_war_state.faction_mutation_scores,
        timestamp: clock.unix_timestamp,
    });

    msg!("✅ [faction_war.settle_faction_war_internal] settlement complete");
    Ok(())
}

/// Fully permissionless -- all inputs (mutation scores) are already on-chain.
/// Anyone can crank settlement once the economy cycle's LP burn has completed.
#[derive(Accounts)]
pub struct SettleFactionWar<'info> {
    #[account(
        mut,
        seeds = [FACTION_WAR_CONFIG_SEED],
        bump = faction_war_config.bump,
    )]
    pub faction_war_config: Box<Account<'info, FactionWarConfig>>,

    #[account(
        mut,
        seeds = [FACTION_WAR_STATE_SEED, &faction_war_state.faction_war_id.to_le_bytes()],
        bump = faction_war_state.bump,
    )]
    pub faction_war_state: Box<Account<'info, FactionWarState>>,

    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED],
        bump = tax_config.bump,
    )]
    pub tax_config: Box<Account<'info, TaxConfig>>,

    #[account(
        seeds = [MINE_BTC_MINING_SEED],
        bump = mine_btc_mining.bump,
    )]
    pub mine_btc_mining: Box<Account<'info, MineBtcMining>>,

    /// Needed to read `rpg_progression` for the no-mutation branch of
    /// `finalize_faction_war_settlement`.
    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// Needed to derive the current round's game_session PDA so the
    /// stage=1 guard below can see it.
    #[account(
        seeds = [GLOBAL_GAME_STATE_SEED],
        bump = global_game_state.bump,
    )]
    pub global_game_state: Box<Account<'info, GlobalGameSate>>,

    /// Game session for the current round. Used to block this crank while
    /// stage=1 (the end_round → end_round_faction_rewards window).
    #[account(
        seeds = [GAME_SESSION_SEED, &global_game_state.current_round_id.to_le_bytes()],
        bump = game_session.bump,
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
                            let doge_metadata = ctx
                                .accounts
                                .doge_metadata
                                .as_mut()
                                .ok_or(ErrorCode::DogeMetadataNotFound)?;
                            require_keys_eq!(
                                doge_metadata.mint,
                                doge_mint,
                                ErrorCode::InvalidAccount
                            );
                            let old_accum = doge_metadata.accumulated_val;
                            doge_metadata.accumulated_val = doge_metadata
                                .accumulated_val
                                .checked_add(doge_bonus_amount)
                                .ok_or(ErrorCode::ArithmeticOverflow)?;
                            msg!("⚔️ [faction_war.claim_faction_war_rewards_internal] doge_metadata.accumulated_val: old={} add={} new={}", old_accum, doge_bonus_amount, doge_metadata.accumulated_val);
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

    // --- SOL Cycle Jackpot: proportional to base dogeBTC reward share ---
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
        bump = faction_war_state.bump,
    )]
    pub faction_war_state: Box<Account<'info, FactionWarState>>,

    #[account(
        mut,
        close = cranker,
        seeds = [USER_FACTION_WAR_BETS_SEED, user_faction_war_bets.owner.as_ref(), &faction_war_id.to_le_bytes()],
        bump = user_faction_war_bets.bump,
    )]
    pub user_faction_war_bets: Box<Account<'info, UserFactionWarBets>>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, user_faction_war_bets.owner.as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user_faction_war_bets.owner @ ErrorCode::InvalidOwner,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    #[account(
        mut,
        seeds = [HODL_POOL_SEED],
        bump,
    )]
    pub hodl_pool: Box<Account<'info, HodlPool>>,

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
