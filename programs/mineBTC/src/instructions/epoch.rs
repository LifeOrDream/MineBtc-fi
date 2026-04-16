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

/// Compute how the epoch mining pool is split across factions.
///
/// Each faction's share is proportional to its winning-direction bet weight
/// relative to the sum of all factions' winning-direction bet weights.
///
/// `country_pool[f] = epoch_pool × winning_bets[f] / total_winning_bets`
///
/// Factions where more players bet the correct direction capture a bigger slice,
/// while correct bets on unpopular factions have a higher per-SOL return.
pub fn compute_faction_reward_pools(epoch_state: &mut EpochState) -> Result<()> {
    let active_factions = epoch_state.active_faction_count as usize;
    validate_active_faction_count(active_factions)?;

    let pool = epoch_state.epoch_mining_pool;
    if pool == 0 {
        epoch_state.faction_reward_pools = [0u64; NUM_FACTIONS];
        epoch_state.faction_doge_reward_pools = [0u64; NUM_FACTIONS];
        return Ok(());
    }

    let mut faction_winning_weights = [0u64; NUM_FACTIONS];
    let mut total_winning_weight: u128 = 0;

    for f in 0..active_factions {
        let winning_dir = epoch_state.resolved_directions[f] as usize;
        let w = epoch_state.faction_direction_totals[f][winning_dir];
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
        let resolved_direction = epoch_state.resolved_directions[faction_id] as usize;
        let eligible_total =
            epoch_state.eligible_doge_direction_totals[faction_id][resolved_direction];
        let raw_pool = reward_pools[faction_id];

        if raw_pool == 0 || eligible_total == 0 {
            user_reward_pools[faction_id] = raw_pool;
            continue;
        }

        let doge_pool = (raw_pool as u128)
            .checked_mul(EPOCH_DOGE_REWARD_SHARE_BPS as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(BASIS_POINTS_DENOMINATOR as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        let doge_pool = u64::try_from(doge_pool).map_err(|_| ErrorCode::ArithmeticOverflow)?;

        doge_reward_pools[faction_id] = doge_pool;
        user_reward_pools[faction_id] = raw_pool
            .checked_sub(doge_pool)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    epoch_state.faction_reward_pools = user_reward_pools;
    epoch_state.faction_doge_reward_pools = doge_reward_pools;
    Ok(())
}

// ========================================================================================
// ============================= EPOCH CONFIG ==============================================
// ========================================================================================

pub fn initialize_epoch_config_internal(ctx: Context<InitializeEpochConfig>) -> Result<()> {
    let epoch_config = &mut ctx.accounts.epoch_config;
    epoch_config.bump = ctx.bumps.epoch_config;
    epoch_config.current_epoch_id = 1;
    epoch_config.is_active = true;
    epoch_config.epoch_settle_cycle = 0;

    // Sequential starting ranks so the first epoch has a sensible baseline.
    let mut initial_ranks = [0u8; NUM_FACTIONS];
    for i in 0..NUM_FACTIONS {
        initial_ranks[i] = i as u8;
    }
    epoch_config.prev_epoch_mutation_ranks = initial_ranks;

    Ok(())
}

#[derive(Accounts)]
pub struct InitializeEpochConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = EpochConfig::LEN,
        seeds = [EPOCH_CONFIG_SEED],
        bump,
    )]
    pub epoch_config: Account<'info, EpochConfig>,

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

pub fn update_epoch_config_internal(
    ctx: Context<UpdateEpochConfig>,
    is_active: Option<bool>,
) -> Result<()> {
    let epoch_config = &mut ctx.accounts.epoch_config;

    if let Some(active) = is_active {
        epoch_config.is_active = active;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateEpochConfig<'info> {
    #[account(
        mut,
        seeds = [EPOCH_CONFIG_SEED],
        bump = epoch_config.bump,
    )]
    pub epoch_config: Account<'info, EpochConfig>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub authority: Signer<'info>,
}

// ========================================================================================
// ============================= EPOCH SETTLEMENT ==========================================
// ========================================================================================

pub fn finalize_epoch_settlement(
    epoch_config: &mut EpochConfig,
    epoch_state: &mut EpochState,
) -> Result<()> {
    let active_factions = epoch_state.active_faction_count as usize;
    validate_active_faction_count(active_factions)?;

    // Check if any mutations happened. If mutations were disabled (rpg_progression off),
    // all scores will be 0 and no epoch rewards should be distributed.
    let total_mutation_score: u64 = epoch_state
        .faction_mutation_scores
        .iter()
        .take(active_factions)
        .sum();

    if total_mutation_score == 0 {
        // No mutations this epoch -- skip reward distribution entirely.
        // Mining pool stays at 0 so nobody can claim.
        epoch_state.epoch_mining_pool = 0;
        epoch_state.faction_reward_pools = [0u64; NUM_FACTIONS];
        epoch_state.faction_doge_reward_pools = [0u64; NUM_FACTIONS];
        epoch_state.stage = 1;
    } else {
        epoch_state.epoch_mining_pool = epoch_state.total_dogebtc_mined_in_epoch;

        // Rank factions by their accumulated mutation scores.
        let mut mutation_scores_i64 = [0i64; NUM_FACTIONS];
        for i in 0..active_factions {
            mutation_scores_i64[i] = epoch_state.faction_mutation_scores[i] as i64;
        }
        let (final_ranks, _) = compute_rankings(&mutation_scores_i64, active_factions)?;
        epoch_state.final_ranks = final_ranks;

        for faction_id in 0..active_factions {
            let (direction, rank_delta) = resolve_direction_from_ranks(
                epoch_state.start_ranks[faction_id],
                epoch_state.final_ranks[faction_id],
            );
            epoch_state.rank_deltas[faction_id] = rank_delta;
            epoch_state.resolved_directions[faction_id] = direction.as_index() as u8;
        }

        compute_faction_reward_pools(epoch_state)?;
        epoch_state.stage = 1;

        // Persist mutation-based ranks for next epoch's start_ranks.
        epoch_config.prev_epoch_mutation_ranks = final_ranks;
    }

    epoch_config.current_epoch_id = epoch_config
        .current_epoch_id
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    Ok(())
}

pub fn settle_epoch_internal(ctx: Context<SettleEpoch>) -> Result<()> {
    let epoch_config = &mut ctx.accounts.epoch_config;
    let epoch_state = &mut ctx.accounts.epoch_state;
    let mining = &ctx.accounts.mine_btc_mining;

    require!(epoch_state.stage == 0, ErrorCode::EpochNotActive);
    // Settlement is gated by the economy cycle: the LP burn must have completed.
    require!(
        mining.pol_stats.lp_operations_count >= epoch_config.epoch_settle_cycle,
        ErrorCode::EpochNotEnded
    );

    finalize_epoch_settlement(epoch_config, epoch_state)?;

    let clock = Clock::get()?;
    emit!(EpochSettled {
        epoch_id: epoch_state.epoch_id,
        total_dogebtc_mined: epoch_state.total_dogebtc_mined_in_epoch,
        epoch_mining_pool: epoch_state.epoch_mining_pool,
        start_ranks: epoch_state.start_ranks,
        final_ranks: epoch_state.final_ranks,
        rank_deltas: epoch_state.rank_deltas,
        resolved_directions: epoch_state.resolved_directions,
        faction_reward_pools: epoch_state.faction_reward_pools,
        faction_doge_reward_pools: epoch_state.faction_doge_reward_pools,
        faction_mutation_scores: epoch_state.faction_mutation_scores,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

/// Fully permissionless -- all inputs (mutation scores) are already on-chain.
/// Anyone can crank settlement once the economy cycle's LP burn has completed.
#[derive(Accounts)]
pub struct SettleEpoch<'info> {
    #[account(
        mut,
        seeds = [EPOCH_CONFIG_SEED],
        bump = epoch_config.bump,
    )]
    pub epoch_config: Account<'info, EpochConfig>,

    #[account(
        mut,
        seeds = [EPOCH_STATE_SEED, &epoch_state.epoch_id.to_le_bytes()],
        bump = epoch_state.bump,
    )]
    pub epoch_state: Account<'info, EpochState>,

    #[account(
        seeds = [MINE_BTC_MINING_SEED],
        bump = mine_btc_mining.bump,
    )]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

    /// Anyone can settle -- no authority check needed.
    pub cranker: Signer<'info>,
}

// ========================================================================================
// ============================= EPOCH CLAIM ===============================================
// ========================================================================================

pub fn claim_epoch_rewards_internal(ctx: Context<ClaimEpochRewards>, epoch_id: u64) -> Result<()> {
    let epoch_state = &ctx.accounts.epoch_state;
    let user_epoch_bets = &ctx.accounts.user_epoch_bets;
    let player_data = &mut ctx.accounts.player_data;
    let unrefined_rewards = &mut ctx.accounts.unrefined_rewards;
    let clock = Clock::get()?;
    let owner_key = user_epoch_bets.owner;

    helper::validate_reward_claim_caller(
        ctx.accounts.cranker.key(),
        owner_key,
        player_data.allow_bots_to_claim,
    )?;

    require!(epoch_state.stage == 1, ErrorCode::EpochNotSettled);
    require!(epoch_state.epoch_mining_pool > 0, ErrorCode::InvalidState);
    require_keys_eq!(player_data.owner, owner_key, ErrorCode::InvalidOwner);

    let active_factions = epoch_state.active_faction_count as usize;
    validate_active_faction_count(active_factions)?;

    // Only own-faction bets count for epoch rewards.
    let faction_id = player_data.faction_id as usize;
    let mut total_reward = 0u64;
    let mut doge_bonus_amount = 0u64;
    let mut doge_mint = Pubkey::default();

    if faction_id < active_factions {
        let resolved_direction = epoch_state.resolved_directions[faction_id] as usize;
        let user_bet = user_epoch_bets.direction_bets[faction_id][resolved_direction];
        let total_bet = epoch_state.faction_direction_totals[faction_id][resolved_direction];
        let faction_pool = epoch_state.faction_reward_pools[faction_id];
        let doge_pool = epoch_state.faction_doge_reward_pools[faction_id];
        let eligible_total =
            epoch_state.eligible_doge_direction_totals[faction_id][resolved_direction];

        if user_bet > 0 && total_bet > 0 && faction_pool > 0 {
            let reward_u128 = (faction_pool as u128)
                .checked_mul(user_bet as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(total_bet as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            total_reward = u64::try_from(reward_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;
        }

        if user_epoch_bets.doge_bonus_eligible
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
                doge_mint = user_epoch_bets.gameplay_doge;
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

    if total_reward > 0 {
        helper::add_to_total_claimable(unrefined_rewards, player_data, total_reward)?;
        player_data.total_dogebtc_won = player_data
            .total_dogebtc_won
            .checked_add(total_reward)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    player_data.pending_epoch_claims = player_data
        .pending_epoch_claims
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    emit!(EpochRewardsClaimed {
        epoch_id,
        user: user_epoch_bets.owner,
        reward_amount: total_reward,
        doge_bonus_amount,
        doge_mint,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(epoch_id: u64)]
pub struct ClaimEpochRewards<'info> {
    #[account(
        seeds = [EPOCH_STATE_SEED, &epoch_id.to_le_bytes()],
        bump = epoch_state.bump,
    )]
    pub epoch_state: Box<Account<'info, EpochState>>,

    #[account(
        mut,
        close = cranker,
        seeds = [USER_EPOCH_BETS_SEED, user_epoch_bets.owner.as_ref(), &epoch_id.to_le_bytes()],
        bump = user_epoch_bets.bump,
    )]
    pub user_epoch_bets: Box<Account<'info, UserEpochBets>>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, user_epoch_bets.owner.as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user_epoch_bets.owner @ ErrorCode::InvalidOwner,
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

    /// CHECK: Validated by constraint that player.key() == user_epoch_bets.owner
    #[account(
        mut,
        constraint = player.key() == user_epoch_bets.owner @ ErrorCode::InvalidOwner,
    )]
    pub player: AccountInfo<'info>,

    #[account(mut)]
    pub cranker: Signer<'info>,

    pub system_program: Program<'info, System>,
}
