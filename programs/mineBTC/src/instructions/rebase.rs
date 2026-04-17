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

/// Compute how the rebase mining pool is split across factions.
///
/// Each faction's share is proportional to its winning-direction bet weight
/// relative to the sum of all factions' winning-direction bet weights.
///
/// `country_pool[f] = epoch_pool × winning_bets[f] / total_winning_bets`
///
/// Factions where more players bet the correct direction capture a bigger slice,
/// while correct bets on unpopular factions have a higher per-SOL return.
pub fn compute_faction_reward_pools(rebase_state: &mut RebaseState) -> Result<()> {
    let active_factions = rebase_state.active_faction_count as usize;
    validate_active_faction_count(active_factions)?;

    let pool = rebase_state.rebase_mining_pool;
    if pool == 0 {
        rebase_state.faction_reward_pools = [0u64; NUM_FACTIONS];
        rebase_state.faction_doge_reward_pools = [0u64; NUM_FACTIONS];
        return Ok(());
    }

    let mut faction_winning_weights = [0u64; NUM_FACTIONS];
    let mut total_winning_weight: u128 = 0;

    for f in 0..active_factions {
        let winning_dir = rebase_state.resolved_directions[f] as usize;
        let w = rebase_state.faction_direction_totals[f][winning_dir];
        faction_winning_weights[f] = w;
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
        let resolved_direction = rebase_state.resolved_directions[faction_id] as usize;
        let eligible_total =
            rebase_state.eligible_doge_direction_totals[faction_id][resolved_direction];
        let raw_pool = reward_pools[faction_id];

        if raw_pool == 0 || eligible_total == 0 {
            user_reward_pools[faction_id] = raw_pool;
            continue;
        }

        let doge_pool = (raw_pool as u128)
            .checked_mul(REBASE_DOGE_REWARD_SHARE_BPS as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(BASIS_POINTS_DENOMINATOR as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        let doge_pool = u64::try_from(doge_pool).map_err(|_| ErrorCode::ArithmeticOverflow)?;

        doge_reward_pools[faction_id] = doge_pool;
        user_reward_pools[faction_id] = raw_pool
            .checked_sub(doge_pool)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    rebase_state.faction_reward_pools = user_reward_pools;
    rebase_state.faction_doge_reward_pools = doge_reward_pools;
    Ok(())
}

// ========================================================================================
// ============================= REBASE CONFIG ==============================================
// ========================================================================================

pub fn initialize_rebase_config_internal(ctx: Context<InitializeRebaseConfig>) -> Result<()> {
    msg!("🔄 [initialize_rebase_config] Initializing rebase system");
    let rebase_config = &mut ctx.accounts.rebase_config;
    rebase_config.bump = ctx.bumps.rebase_config;
    rebase_config.current_rebase_id = 1;
    rebase_config.is_active = true;
    rebase_config.rebase_settle_cycle = 0;

    let mut initial_ranks = [0u8; NUM_FACTIONS];
    for i in 0..NUM_FACTIONS {
        initial_ranks[i] = i as u8;
    }
    rebase_config.prev_rebase_mutation_ranks = initial_ranks;

    msg!("   ✅ RebaseConfig initialized. Starting rebase_id: 1");
    Ok(())
}

#[derive(Accounts)]
pub struct InitializeRebaseConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = RebaseConfig::LEN,
        seeds = [REBASE_CONFIG_SEED],
        bump,
    )]
    pub rebase_config: Account<'info, RebaseConfig>,

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

pub fn update_rebase_config_internal(
    ctx: Context<UpdateRebaseConfig>,
    is_active: Option<bool>,
) -> Result<()> {
    let rebase_config = &mut ctx.accounts.rebase_config;

    if let Some(active) = is_active {
        rebase_config.is_active = active;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateRebaseConfig<'info> {
    #[account(
        mut,
        seeds = [REBASE_CONFIG_SEED],
        bump = rebase_config.bump,
    )]
    pub rebase_config: Account<'info, RebaseConfig>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub authority: Signer<'info>,
}

// ========================================================================================
// ============================= REBASE SETTLEMENT ==========================================
// ========================================================================================

pub fn finalize_rebase_settlement(
    rebase_config: &mut RebaseConfig,
    rebase_state: &mut RebaseState,
) -> Result<()> {
    let active_factions = rebase_state.active_faction_count as usize;

    // Empty rebase (no bets ever placed, e.g. seeded by init_if_needed in
    // EndRoundFactionRewards and never populated by a subsequent join_bets):
    // settle with no rewards and advance current_rebase_id so the cycle can
    // keep moving. Without this, validate_active_faction_count reverts and
    // every subsequent LP burn can't advance past this rebase.
    if active_factions == 0 {
        msg!(
            "   ⚠️ Rebase #{} has 0 active factions — settling empty and advancing",
            rebase_state.rebase_id
        );
        rebase_state.stage = 1;
        rebase_state.rebase_mining_pool = 0;
        rebase_state.faction_reward_pools = [0u64; NUM_FACTIONS];
        rebase_state.faction_doge_reward_pools = [0u64; NUM_FACTIONS];
        rebase_config.current_rebase_id = rebase_config
            .current_rebase_id
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        return Ok(());
    }

    validate_active_faction_count(active_factions)?;

    msg!(
        "🔄 [finalize_rebase_settlement] Rebase #{}, {} factions, {} dogeBTC mined",
        rebase_state.rebase_id,
        active_factions,
        rebase_state.total_dogebtc_mined_in_rebase
    );

    let total_mutation_score: u64 = rebase_state
        .faction_mutation_scores
        .iter()
        .take(active_factions)
        .sum();

    if total_mutation_score == 0 {
        msg!("   ⚠️ No mutations this rebase — no rewards distributed");
        rebase_state.rebase_mining_pool = 0;
        rebase_state.faction_reward_pools = [0u64; NUM_FACTIONS];
        rebase_state.faction_doge_reward_pools = [0u64; NUM_FACTIONS];
        rebase_state.stage = 1;
    } else {
        rebase_state.rebase_mining_pool = rebase_state.total_dogebtc_mined_in_rebase;
        msg!(
            "   💰 Rebase mining pool: {} dogeBTC",
            rebase_state.rebase_mining_pool
        );

        let mut mutation_scores_i64 = [0i64; NUM_FACTIONS];
        for i in 0..active_factions {
            mutation_scores_i64[i] = rebase_state.faction_mutation_scores[i] as i64;
        }
        let (final_ranks, _) = compute_rankings(&mutation_scores_i64, active_factions)?;
        rebase_state.final_ranks = final_ranks;

        for faction_id in 0..active_factions {
            let (direction, rank_delta) = resolve_direction_from_ranks(
                rebase_state.start_ranks[faction_id],
                rebase_state.final_ranks[faction_id],
            );
            rebase_state.rank_deltas[faction_id] = rank_delta;
            rebase_state.resolved_directions[faction_id] = direction.as_index() as u8;

            let dir_str = match direction {
                PredictionDirection::Up => "Up",
                PredictionDirection::Down => "Down",
                PredictionDirection::Neutral => "Neutral",
            };
            msg!(
                "   🏴 Faction {}: score={}, rank {} → {}, delta={}, dir={}",
                faction_id,
                rebase_state.faction_mutation_scores[faction_id],
                rebase_state.start_ranks[faction_id],
                final_ranks[faction_id],
                rank_delta,
                dir_str
            );
        }

        compute_faction_reward_pools(rebase_state)?;
        rebase_state.stage = 1;

        rebase_config.prev_rebase_mutation_ranks = final_ranks;
    }

    rebase_config.current_rebase_id = rebase_config
        .current_rebase_id
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    msg!(
        "   ✅ Rebase settled. Next rebase_id: {}",
        rebase_config.current_rebase_id
    );
    Ok(())
}

pub fn settle_rebase_internal(ctx: Context<SettleRebase>) -> Result<()> {
    msg!("🔄 [settle_rebase] Manual settlement crank");
    let rebase_config = &mut ctx.accounts.rebase_config;
    let rebase_state = &mut ctx.accounts.rebase_state;
    let mining = &ctx.accounts.mine_btc_mining;

    msg!(
        "   Rebase #{}, stage={}, lp_ops={}, settle_cycle={}",
        rebase_state.rebase_id,
        rebase_state.stage,
        mining.pol_stats.lp_operations_count,
        rebase_config.rebase_settle_cycle
    );

    require!(rebase_state.stage == 0, ErrorCode::RebaseNotActive);
    require!(
        mining.pol_stats.lp_operations_count >= rebase_config.rebase_settle_cycle,
        ErrorCode::RebaseNotEnded
    );

    finalize_rebase_settlement(rebase_config, rebase_state)?;

    let clock = Clock::get()?;
    emit!(RebaseSettled {
        rebase_id: rebase_state.rebase_id,
        total_dogebtc_mined: rebase_state.total_dogebtc_mined_in_rebase,
        rebase_mining_pool: rebase_state.rebase_mining_pool,
        start_ranks: rebase_state.start_ranks,
        final_ranks: rebase_state.final_ranks,
        rank_deltas: rebase_state.rank_deltas,
        resolved_directions: rebase_state.resolved_directions,
        faction_reward_pools: rebase_state.faction_reward_pools,
        faction_doge_reward_pools: rebase_state.faction_doge_reward_pools,
        faction_mutation_scores: rebase_state.faction_mutation_scores,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

/// Fully permissionless -- all inputs (mutation scores) are already on-chain.
/// Anyone can crank settlement once the economy cycle's LP burn has completed.
#[derive(Accounts)]
pub struct SettleRebase<'info> {
    #[account(
        mut,
        seeds = [REBASE_CONFIG_SEED],
        bump = rebase_config.bump,
    )]
    pub rebase_config: Account<'info, RebaseConfig>,

    #[account(
        mut,
        seeds = [REBASE_STATE_SEED, &rebase_state.rebase_id.to_le_bytes()],
        bump = rebase_state.bump,
    )]
    pub rebase_state: Account<'info, RebaseState>,

    #[account(
        seeds = [MINE_BTC_MINING_SEED],
        bump = mine_btc_mining.bump,
    )]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

    /// Anyone can settle -- no authority check needed.
    pub cranker: Signer<'info>,
}

// ========================================================================================
// ============================= REBASE CLAIM ===============================================
// ========================================================================================

pub fn claim_rebase_rewards_internal(
    ctx: Context<ClaimRebaseRewards>,
    rebase_id: u64,
) -> Result<()> {
    msg!(
        "🎁 [claim_rebase_rewards] Rebase #{}, user={}",
        rebase_id,
        ctx.accounts.user_rebase_bets.owner
    );
    let rebase_state = &ctx.accounts.rebase_state;
    let user_rebase_bets = &ctx.accounts.user_rebase_bets;
    let player_data_key = ctx.accounts.player_data.key();
    let player_data = &mut ctx.accounts.player_data;
    let unrefined_rewards = &mut ctx.accounts.unrefined_rewards;
    let clock = Clock::get()?;
    let owner_key = user_rebase_bets.owner;

    helper::validate_reward_claim_caller(
        ctx.accounts.cranker.key(),
        owner_key,
        player_data.allow_bots_to_claim,
    )?;

    require!(rebase_state.stage == 1, ErrorCode::RebaseNotSettled);
    require!(rebase_state.rebase_mining_pool > 0, ErrorCode::InvalidState);
    require_keys_eq!(player_data.owner, owner_key, ErrorCode::InvalidOwner);

    let active_factions = rebase_state.active_faction_count as usize;
    validate_active_faction_count(active_factions)?;

    // Only own-faction bets count for rebase rewards.
    let faction_id = player_data.faction_id as usize;
    let mut total_reward = 0u64;
    let mut doge_bonus_amount = 0u64;
    let mut doge_mint = Pubkey::default();

    if faction_id < active_factions {
        let resolved_direction = rebase_state.resolved_directions[faction_id] as usize;
        let user_bet = user_rebase_bets.direction_bets[faction_id][resolved_direction];
        let total_bet = rebase_state.faction_direction_totals[faction_id][resolved_direction];
        let faction_pool = rebase_state.faction_reward_pools[faction_id];
        let doge_pool = rebase_state.faction_doge_reward_pools[faction_id];
        let eligible_total =
            rebase_state.eligible_doge_direction_totals[faction_id][resolved_direction];

        if user_bet > 0 && total_bet > 0 && faction_pool > 0 {
            let reward_u128 = (faction_pool as u128)
                .checked_mul(user_bet as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(total_bet as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            total_reward = u64::try_from(reward_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;
        }

        if user_rebase_bets.doge_bonus_eligible
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
                doge_mint = user_rebase_bets.gameplay_doge;
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

    msg!(
        "   Faction {}: reward={}, doge_bonus={}, doge_eligible={}",
        faction_id,
        total_reward,
        doge_bonus_amount,
        user_rebase_bets.doge_bonus_eligible
    );

    if total_reward > 0 {
        helper::add_to_total_claimable(
            unrefined_rewards,
            player_data,
            total_reward,
            owner_key,
            player_data_key,
            CLAIMABLE_MINEBTC_SOURCE_REBASE,
            rebase_id,
        )?;
    }

    player_data.pending_rebase_claims = player_data
        .pending_rebase_claims
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    emit!(RebaseRewardsClaimed {
        rebase_id,
        user: user_rebase_bets.owner,
        reward_amount: total_reward,
        doge_bonus_amount,
        doge_mint,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(rebase_id: u64)]
pub struct ClaimRebaseRewards<'info> {
    #[account(
        seeds = [REBASE_STATE_SEED, &rebase_id.to_le_bytes()],
        bump = rebase_state.bump,
    )]
    pub rebase_state: Box<Account<'info, RebaseState>>,

    #[account(
        mut,
        close = cranker,
        seeds = [USER_REBASE_BETS_SEED, user_rebase_bets.owner.as_ref(), &rebase_id.to_le_bytes()],
        bump = user_rebase_bets.bump,
    )]
    pub user_rebase_bets: Box<Account<'info, UserRebaseBets>>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, user_rebase_bets.owner.as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user_rebase_bets.owner @ ErrorCode::InvalidOwner,
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

    /// CHECK: Validated by constraint that player.key() == user_rebase_bets.owner
    #[account(
        mut,
        constraint = player.key() == user_rebase_bets.owner @ ErrorCode::InvalidOwner,
    )]
    pub player: AccountInfo<'info>,

    #[account(mut)]
    pub cranker: Signer<'info>,

    pub system_program: Program<'info, System>,
}
