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

pub fn compute_rankings(
    scores: &[i64; NUM_FACTIONS],
    active_factions: usize,
) -> Result<([u8; NUM_FACTIONS], [u8; NUM_FACTIONS])> {
    validate_active_faction_count(active_factions)?;

    let mut ordered = [0u8; NUM_FACTIONS];
    for (idx, slot) in ordered.iter_mut().enumerate() {
        *slot = idx as u8;
    }

    ordered[..active_factions].sort_by(|a, b| {
        scores[*b as usize]
            .cmp(&scores[*a as usize])
            .then_with(|| a.cmp(b))
    });

    let mut ranks = [0u8; NUM_FACTIONS];
    for (rank, faction_id) in ordered[..active_factions].iter().enumerate() {
        ranks[*faction_id as usize] = rank as u8;
    }

    Ok((ranks, ordered))
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

/// Compute how the faction_war mining pool is split across factions.
///
/// Each faction's share is proportional to its winning-direction bet weight
/// relative to the sum of all factions' winning-direction bet weights.
///
/// `country_pool[f] = faction_war_pool × winning_bets[f] / total_winning_bets`
///
/// Factions where more players bet the correct direction capture a bigger slice,
/// while correct bets on unpopular factions have a higher per-SOL return.
pub fn compute_faction_reward_pools(faction_war_state: &mut FactionWarState) -> Result<()> {
    let active_factions = faction_war_state.active_faction_count as usize;
    validate_active_faction_count(active_factions)?;

    let pool = faction_war_state.faction_war_mining_pool;
    if pool == 0 {
        faction_war_state.faction_reward_pools = [0u64; NUM_FACTIONS];
        faction_war_state.faction_doge_reward_pools = [0u64; NUM_FACTIONS];
        return Ok(());
    }

    let mut faction_winning_weights = [0u64; NUM_FACTIONS];
    let mut total_winning_weight: u128 = 0;

    for (f, winning_weight) in faction_winning_weights
        .iter_mut()
        .enumerate()
        .take(active_factions)
    {
        let winning_dir = faction_war_state.resolved_directions[f] as usize;
        let w = faction_war_state.faction_direction_totals[f][winning_dir];
        *winning_weight = w;
        total_winning_weight = total_winning_weight
            .checked_add(w as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    let mut reward_pools = [0u64; NUM_FACTIONS];
    if total_winning_weight > 0 {
        for f in 0..active_factions {
            let share = (pool as u128)
                .checked_mul(faction_winning_weights[f] as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(total_winning_weight)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            reward_pools[f] =
                u64::try_from(share).map_err(|_| error!(ErrorCode::ArithmeticOverflow))?;
        }
    }

    let mut user_reward_pools = [0u64; NUM_FACTIONS];
    let mut doge_reward_pools = [0u64; NUM_FACTIONS];

    for faction_id in 0..active_factions {
        let resolved_direction = faction_war_state.resolved_directions[faction_id] as usize;
        let eligible_total =
            faction_war_state.eligible_doge_direction_totals[faction_id][resolved_direction];
        let raw_pool = reward_pools[faction_id];

        if raw_pool == 0 || eligible_total == 0 {
            user_reward_pools[faction_id] = raw_pool;
            continue;
        }

        let doge_pool = (raw_pool as u128)
            .checked_mul(FACTION_WAR_DOGE_REWARD_SHARE_BPS as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(BASIS_POINTS_DENOMINATOR as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        let doge_pool = u64::try_from(doge_pool).map_err(|_| ErrorCode::ArithmeticOverflow)?;

        doge_reward_pools[faction_id] = doge_pool;
        user_reward_pools[faction_id] = raw_pool
            .checked_sub(doge_pool)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    faction_war_state.faction_reward_pools = user_reward_pools;
    faction_war_state.faction_doge_reward_pools = doge_reward_pools;
    Ok(())
}

// ========================================================================================
// ============================= FACTION_WAR CONFIG ==============================================
// ========================================================================================

pub fn initialize_faction_war_config_internal(
    ctx: Context<InitializeFactionWarConfig>,
) -> Result<()> {
    msg!("🔄 [initialize_faction_war_config] Initializing faction_war system");
    let faction_war_config = &mut ctx.accounts.faction_war_config;
    faction_war_config.bump = ctx.bumps.faction_war_config;
    faction_war_config.current_faction_war_id = 1;
    faction_war_config.is_active = true;
    faction_war_config.faction_war_settle_cycle = 0;

    let mut initial_ranks = [0u8; NUM_FACTIONS];
    for (i, rank) in initial_ranks.iter_mut().enumerate().take(NUM_FACTIONS) {
        *rank = i as u8;
    }
    faction_war_config.prev_faction_war_mutation_ranks = initial_ranks;

    msg!("   ✅ FactionWarConfig initialized. Starting faction_war_id: 1");
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
    let faction_war_config = &mut ctx.accounts.faction_war_config;

    if let Some(active) = is_active {
        faction_war_config.is_active = active;
    }

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
    rpg_progression: bool,
) -> Result<()> {
    let active_factions = faction_war_state.active_faction_count as usize;

    // Empty faction_war (no bets ever placed, e.g. seeded by init_if_needed in
    // EndRoundFactionRewards and never populated by a subsequent join_bets):
    // settle with no rewards and advance current_faction_war_id so the cycle can
    // keep moving. Without this, validate_active_faction_count reverts and
    // every subsequent LP burn can't advance past this faction_war.
    if active_factions == 0 {
        msg!(
            "   ⚠️ FactionWar #{} has 0 active factions — settling empty and advancing",
            faction_war_state.faction_war_id
        );
        if faction_war_state.treasury_reward_base_amount > 0 {
            tax_config.unassigned_faction_war_treasury_amount = tax_config
                .unassigned_faction_war_treasury_amount
                .checked_add(faction_war_state.treasury_reward_base_amount)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!(
                "   ↩️ Rolled forward {} treasury tax because this faction war never got real participants",
                faction_war_state.treasury_reward_base_amount
            );
            faction_war_state.treasury_reward_base_amount = 0;
        }
        faction_war_state.stage = 1;
        faction_war_state.faction_war_mining_pool = 0;
        faction_war_state.faction_reward_pools = [0u64; NUM_FACTIONS];
        faction_war_state.faction_doge_reward_pools = [0u64; NUM_FACTIONS];
        faction_war_config.current_faction_war_id = faction_war_config
            .current_faction_war_id
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        return Ok(());
    }

    validate_active_faction_count(active_factions)?;

    msg!(
        "🔄 [finalize_faction_war_settlement] FactionWar #{}, {} factions, {} dogeBTC mined",
        faction_war_state.faction_war_id,
        active_factions,
        faction_war_state.total_dogebtc_mined_in_faction_war
    );

    let total_mutation_score: u64 = faction_war_state
        .faction_mutation_scores
        .iter()
        .take(active_factions)
        .sum();

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

        if !has_bets || !rpg_progression {
            msg!(
                "   ⚠️ FactionWar #{} non-operational (has_bets={}, rpg_progression={}) — rolling treasury forward and settling empty",
                faction_war_state.faction_war_id,
                has_bets,
                rpg_progression
            );
            if faction_war_state.treasury_reward_base_amount > 0 {
                tax_config.unassigned_faction_war_treasury_amount = tax_config
                    .unassigned_faction_war_treasury_amount
                    .checked_add(faction_war_state.treasury_reward_base_amount)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
                msg!(
                    "   ↩️ Rolled forward {} treasury tax (non-operational faction war)",
                    faction_war_state.treasury_reward_base_amount
                );
                faction_war_state.treasury_reward_base_amount = 0;
            }
            faction_war_state.faction_war_mining_pool = 0;
            faction_war_state.faction_reward_pools = [0u64; NUM_FACTIONS];
            faction_war_state.faction_doge_reward_pools = [0u64; NUM_FACTIONS];
            faction_war_state.stage = 1;
        } else {
            msg!(
                "   ⚠️ No mutations this faction_war but gameplay happened — distributing rewards with no rank change (Neutral wins everywhere)"
            );
            faction_war_state.faction_war_mining_pool =
                faction_war_state.total_dogebtc_mined_in_faction_war;
            // No rank change: final_ranks = start_ranks, deltas all zero, every
            // faction resolves to Neutral.
            faction_war_state.final_ranks = faction_war_state.start_ranks;
            faction_war_state.rank_deltas = [0i8; NUM_FACTIONS];
            for i in 0..active_factions {
                faction_war_state.resolved_directions[i] =
                    PredictionDirection::Neutral.as_index() as u8;
            }
            compute_faction_reward_pools(faction_war_state)?;
            faction_war_state.stage = 1;
        }
    } else {
        faction_war_state.faction_war_mining_pool =
            faction_war_state.total_dogebtc_mined_in_faction_war;
        msg!(
            "   💰 FactionWar mining pool: {} dogeBTC",
            faction_war_state.faction_war_mining_pool
        );

        let mut mutation_scores_i64 = [0i64; NUM_FACTIONS];
        for (i, score) in mutation_scores_i64
            .iter_mut()
            .enumerate()
            .take(active_factions)
        {
            *score = faction_war_state.faction_mutation_scores[i] as i64;
        }
        let (final_ranks, _) = compute_rankings(&mutation_scores_i64, active_factions)?;
        faction_war_state.final_ranks = final_ranks;

        for (faction_id, _) in final_ranks.iter().enumerate().take(active_factions) {
            let (direction, rank_delta) = resolve_direction_from_ranks(
                faction_war_state.start_ranks[faction_id],
                faction_war_state.final_ranks[faction_id],
            );
            faction_war_state.rank_deltas[faction_id] = rank_delta;
            faction_war_state.resolved_directions[faction_id] = direction.as_index() as u8;

            let dir_str = match direction {
                PredictionDirection::Up => "Up",
                PredictionDirection::Down => "Down",
                PredictionDirection::Neutral => "Neutral",
            };
            msg!(
                "   🏴 Faction {}: score={}, rank {} → {}, delta={}, dir={}",
                faction_id,
                faction_war_state.faction_mutation_scores[faction_id],
                faction_war_state.start_ranks[faction_id],
                final_ranks[faction_id],
                rank_delta,
                dir_str
            );
        }

        compute_faction_reward_pools(faction_war_state)?;
        faction_war_state.stage = 1;

        faction_war_config.prev_faction_war_mutation_ranks = final_ranks;
    }

    faction_war_config.current_faction_war_id = faction_war_config
        .current_faction_war_id
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    msg!(
        "   ✅ FactionWar settled. Next faction_war_id: {}",
        faction_war_config.current_faction_war_id
    );
    Ok(())
}

pub fn settle_faction_war_internal(ctx: Context<SettleFactionWar>) -> Result<()> {
    msg!("🔄 [settle_faction_war] Manual settlement crank");
    let faction_war_config = &mut *ctx.accounts.faction_war_config;
    let faction_war_state = &mut *ctx.accounts.faction_war_state;
    let tax_config = &mut *ctx.accounts.tax_config;
    let mining = &*ctx.accounts.mine_btc_mining;
    let rpg_progression = ctx.accounts.global_config.rpg_progression;

    msg!(
        "   FactionWar #{}, stage={}, lp_ops={}, settle_cycle={}",
        faction_war_state.faction_war_id,
        faction_war_state.stage,
        mining.pol_stats.lp_operations_count,
        faction_war_config.faction_war_settle_cycle
    );

    require!(faction_war_state.stage == 0, ErrorCode::FactionWarNotActive);
    require!(
        mining.pol_stats.lp_operations_count >= faction_war_config.faction_war_settle_cycle,
        ErrorCode::FactionWarNotEnded
    );

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

    finalize_faction_war_settlement(
        faction_war_config,
        faction_war_state,
        tax_config,
        rpg_progression,
    )?;

    let clock = Clock::get()?;
    emit!(FactionWarSettled {
        faction_war_id: faction_war_state.faction_war_id,
        total_dogebtc_mined: faction_war_state.total_dogebtc_mined_in_faction_war,
        faction_war_mining_pool: faction_war_state.faction_war_mining_pool,
        start_ranks: faction_war_state.start_ranks,
        final_ranks: faction_war_state.final_ranks,
        rank_deltas: faction_war_state.rank_deltas,
        resolved_directions: faction_war_state.resolved_directions,
        faction_reward_pools: faction_war_state.faction_reward_pools,
        faction_doge_reward_pools: faction_war_state.faction_doge_reward_pools,
        faction_mutation_scores: faction_war_state.faction_mutation_scores,
        timestamp: clock.unix_timestamp,
    });

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
    msg!(
        "🎁 [claim_faction_war_rewards] FactionWar #{}, user={}",
        faction_war_id,
        ctx.accounts.user_faction_war_bets.owner
    );
    let faction_war_state = &ctx.accounts.faction_war_state;
    let user_faction_war_bets = &ctx.accounts.user_faction_war_bets;
    let player_data_key = ctx.accounts.player_data.key();
    let player_data = &mut ctx.accounts.player_data;
    let unrefined_rewards = &mut ctx.accounts.unrefined_rewards;
    let clock = Clock::get()?;
    let owner_key = user_faction_war_bets.owner;

    helper::validate_reward_claim_caller(
        ctx.accounts.cranker.key(),
        owner_key,
        player_data.allow_bots_to_claim,
    )?;

    require!(
        faction_war_state.stage == 1,
        ErrorCode::FactionWarNotSettled
    );
    require_keys_eq!(player_data.owner, owner_key, ErrorCode::InvalidOwner);

    let active_factions = faction_war_state.active_faction_count as usize;
    let mut total_reward = 0u64;
    let mut doge_bonus_amount = 0u64;
    let mut doge_mint = Pubkey::default();
    let faction_id = player_data.faction_id as usize;

    if active_factions == 0 {
        msg!(
            "   ⚠️ FactionWar {} settled with 0 active factions. Closing claim with 0 reward.",
            faction_war_id
        );
    } else {
        validate_active_faction_count(active_factions)?;

        // Only own-faction bets count for faction_war rewards.
        if faction_id < active_factions {
            let resolved_direction = faction_war_state.resolved_directions[faction_id] as usize;
            let user_bet = user_faction_war_bets.direction_bets[faction_id][resolved_direction];
            let total_bet =
                faction_war_state.faction_direction_totals[faction_id][resolved_direction];
            let faction_pool = faction_war_state.faction_reward_pools[faction_id];
            let doge_pool = faction_war_state.faction_doge_reward_pools[faction_id];
            let eligible_total =
                faction_war_state.eligible_doge_direction_totals[faction_id][resolved_direction];

            if user_bet > 0 && total_bet > 0 && faction_pool > 0 {
                let reward_u128 = (faction_pool as u128)
                    .checked_mul(user_bet as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?
                    .checked_div(total_bet as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
                total_reward =
                    u64::try_from(reward_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;
            }

            if user_faction_war_bets.doge_bonus_eligible
                && user_bet > 0
                && doge_pool > 0
                && eligible_total > 0
            {
                let bonus_u128 = (doge_pool as u128)
                    .checked_mul(user_bet as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?
                    .checked_div(eligible_total as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
                doge_bonus_amount =
                    u64::try_from(bonus_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;

                if doge_bonus_amount > 0 {
                    doge_mint = user_faction_war_bets.gameplay_doge;
                    let doge_metadata = ctx
                        .accounts
                        .doge_metadata
                        .as_mut()
                        .ok_or(ErrorCode::DogeMetadataNotFound)?;
                    require_keys_eq!(doge_metadata.mint, doge_mint, ErrorCode::InvalidAccount);
                    doge_metadata.accumulated_val = doge_metadata
                        .accumulated_val
                        .checked_add(doge_bonus_amount)
                        .ok_or(ErrorCode::ArithmeticOverflow)?;
                }
            }
        }
    }

    msg!(
        "   Faction {}: reward={}, doge_bonus={}, doge_eligible={}",
        faction_id,
        total_reward,
        doge_bonus_amount,
        user_faction_war_bets.doge_bonus_eligible
    );

    if total_reward > 0 {
        helper::add_to_total_claimable(
            unrefined_rewards,
            player_data,
            total_reward,
            owner_key,
            player_data_key,
            CLAIMABLE_MINEBTC_SOURCE_FACTION_WAR,
            faction_war_id,
        )?;
    }

    player_data.pending_faction_war_claims = player_data
        .pending_faction_war_claims
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    emit!(FactionWarRewardsClaimed {
        faction_war_id,
        user: user_faction_war_bets.owner,
        reward_amount: total_reward,
        doge_bonus_amount,
        doge_mint,
        timestamp: clock.unix_timestamp,
    });

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
        seeds = [UNREFINED_REWARDS_SEED],
        bump,
    )]
    pub unrefined_rewards: Box<Account<'info, UnrefinedRewards>>,

    #[account(mut)]
    pub doge_metadata: Option<Box<Account<'info, DogeMetadata>>>,

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
