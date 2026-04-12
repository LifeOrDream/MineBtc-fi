use anchor_lang::prelude::*;

use crate::errors::ErrorCode;
use crate::events::*;
use crate::instructions::helper;
use crate::state::*;

pub fn compute_rankings(scores: &[i64; NUM_FACTIONS]) -> ([u8; NUM_FACTIONS], [u8; NUM_FACTIONS]) {
    let mut ordered = [0u8; NUM_FACTIONS];
    for (idx, slot) in ordered.iter_mut().enumerate() {
        *slot = idx as u8;
    }

    ordered.sort_by(|a, b| {
        scores[*b as usize]
            .cmp(&scores[*a as usize])
            .then_with(|| a.cmp(b))
    });

    let mut ranks = [0u8; NUM_FACTIONS];
    for (rank, faction_id) in ordered.iter().enumerate() {
        ranks[*faction_id as usize] = rank as u8;
    }

    (ranks, ordered)
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

pub fn compute_faction_reward_pools(
    epoch_state: &mut EpochState,
    epoch_config: &EpochConfig,
) -> Result<()> {
    let pool = epoch_state.epoch_mining_pool;
    if pool == 0 {
        epoch_state.faction_reward_pools = [0u64; NUM_FACTIONS];
        return Ok(());
    }

    let pct_of_pool = |pct: u8| -> Result<u64> {
        let value = (pool as u128)
            .checked_mul(pct as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(100)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        u64::try_from(value).map_err(|_| error!(ErrorCode::ArithmeticOverflow))
    };

    let base_pool = pct_of_pool(epoch_config.model5_pct)? as u128;
    let total_rank_points: u128 = epoch_state
        .final_ranks
        .iter()
        .map(|&rank| (NUM_FACTIONS - rank as usize) as u128)
        .sum();

    let mut reward_pools = [0u64; NUM_FACTIONS];
    if total_rank_points > 0 {
        for faction_id in 0..NUM_FACTIONS {
            let rank_points = (NUM_FACTIONS - epoch_state.final_ranks[faction_id] as usize) as u128;
            let share = base_pool
                .checked_mul(rank_points)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(total_rank_points)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            reward_pools[faction_id] =
                u64::try_from(share).map_err(|_| ErrorCode::ArithmeticOverflow)?;
        }
    }

    let (_, ordered) = compute_rankings(&epoch_state.final_scores);
    let top1_bonus = pct_of_pool(epoch_config.top1_pct)?;
    let top2_bonus = pct_of_pool(epoch_config.top2_pct)?;
    let top3_bonus = pct_of_pool(epoch_config.top3_pct)?;

    reward_pools[ordered[0] as usize] = reward_pools[ordered[0] as usize]
        .checked_add(top1_bonus)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    if NUM_FACTIONS > 1 {
        reward_pools[ordered[1] as usize] = reward_pools[ordered[1] as usize]
            .checked_add(top2_bonus)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    if NUM_FACTIONS > 2 {
        reward_pools[ordered[2] as usize] = reward_pools[ordered[2] as usize]
            .checked_add(top3_bonus)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    epoch_state.faction_reward_pools = reward_pools;
    Ok(())
}

pub fn initialize_epoch_config_internal(
    ctx: Context<InitializeEpochConfig>,
    oracle_authority: Pubkey,
    epoch_duration: u64,
    risk_factor: u16,
    model5_pct: u8,
    top1_pct: u8,
    top2_pct: u8,
    top3_pct: u8,
) -> Result<()> {
    require!(epoch_duration > 0, ErrorCode::InvalidParameters);
    require!(risk_factor <= 1000, ErrorCode::InvalidParameters);
    require!(
        model5_pct as u16 + top1_pct as u16 + top2_pct as u16 + top3_pct as u16 <= 100,
        ErrorCode::InvalidParameters
    );

    let epoch_config = &mut ctx.accounts.epoch_config;
    epoch_config.bump = ctx.bumps.epoch_config;
    epoch_config.oracle_authority = oracle_authority;
    epoch_config.epoch_duration = epoch_duration;
    epoch_config.current_epoch_id = 1;
    epoch_config.last_epoch_start = 0;
    epoch_config.risk_factor = risk_factor;
    epoch_config.is_active = true;
    epoch_config.active_index_id = 0;
    epoch_config.active_question_hash = [0u8; 32];
    epoch_config.next_index_id = 0;
    epoch_config.next_question_hash = [0u8; 32];
    epoch_config.model5_pct = model5_pct;
    epoch_config.top1_pct = top1_pct;
    epoch_config.top2_pct = top2_pct;
    epoch_config.top3_pct = top3_pct;

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
    oracle_authority: Option<Pubkey>,
    epoch_duration: Option<u64>,
    is_active: Option<bool>,
    model5_pct: Option<u8>,
    top1_pct: Option<u8>,
    top2_pct: Option<u8>,
    top3_pct: Option<u8>,
) -> Result<()> {
    let epoch_config = &mut ctx.accounts.epoch_config;

    if let Some(auth) = oracle_authority {
        epoch_config.oracle_authority = auth;
    }
    if let Some(duration) = epoch_duration {
        require!(duration > 0, ErrorCode::InvalidParameters);
        epoch_config.epoch_duration = duration;
    }
    if let Some(active) = is_active {
        epoch_config.is_active = active;
    }
    if let Some(pct) = model5_pct {
        epoch_config.model5_pct = pct;
    }
    if let Some(pct) = top1_pct {
        epoch_config.top1_pct = pct;
    }
    if let Some(pct) = top2_pct {
        epoch_config.top2_pct = pct;
    }
    if let Some(pct) = top3_pct {
        epoch_config.top3_pct = pct;
    }

    let total_pct = epoch_config.model5_pct as u16
        + epoch_config.top1_pct as u16
        + epoch_config.top2_pct as u16
        + epoch_config.top3_pct as u16;
    require!(total_pct <= 100, ErrorCode::InvalidParameters);

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

pub fn initialize_index_state_internal(
    ctx: Context<InitializeIndexState>,
    index_id: u8,
    name: String,
    initial_scores: [i64; NUM_FACTIONS],
) -> Result<()> {
    require!(
        !name.is_empty() && name.len() <= MAX_INDEX_NAME_LENGTH,
        ErrorCode::InvalidParameters
    );

    let (ranks, _) = compute_rankings(&initial_scores);
    let index_state = &mut ctx.accounts.index_state;
    index_state.bump = ctx.bumps.index_state;
    index_state.index_id = index_id;
    index_state.name = name.clone();
    index_state.is_active = true;
    index_state.latest_scores = initial_scores;
    index_state.latest_ranks = ranks;
    index_state.score_updates_count = 0;
    index_state.last_update_ts = Clock::get()?.unix_timestamp;

    emit!(IndexInitialized {
        index_id,
        name,
        initial_scores,
        initial_ranks: ranks,
        timestamp: index_state.last_update_ts,
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(index_id: u8)]
pub struct InitializeIndexState<'info> {
    #[account(
        init,
        payer = authority,
        space = IndexState::LEN,
        seeds = [INDEX_STATE_SEED, &[index_id]],
        bump,
    )]
    pub index_state: Account<'info, IndexState>,

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

pub fn schedule_next_epoch_market_internal(
    ctx: Context<ScheduleNextEpochMarket>,
    next_index_id: u8,
    question_hash: [u8; 32],
) -> Result<()> {
    require!(
        ctx.accounts.index_state.index_id == next_index_id && ctx.accounts.index_state.is_active,
        ErrorCode::InvalidIndexState
    );

    let epoch_config = &mut ctx.accounts.epoch_config;
    let bootstrap_active_market =
        epoch_config.last_epoch_start == 0 && epoch_config.active_question_hash == [0u8; 32];

    if bootstrap_active_market {
        epoch_config.active_index_id = next_index_id;
        epoch_config.active_question_hash = question_hash;
    }
    epoch_config.next_index_id = next_index_id;
    epoch_config.next_question_hash = question_hash;

    emit!(EpochMarketScheduled {
        active_index_id: epoch_config.active_index_id,
        next_index_id,
        next_question_hash: question_hash,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct ScheduleNextEpochMarket<'info> {
    #[account(
        mut,
        seeds = [EPOCH_CONFIG_SEED],
        bump = epoch_config.bump,
        constraint = epoch_config.oracle_authority == authority.key() @ ErrorCode::InvalidOracleAuthority,
    )]
    pub epoch_config: Account<'info, EpochConfig>,

    #[account(
        seeds = [INDEX_STATE_SEED, &[index_state.index_id]],
        bump = index_state.bump,
    )]
    pub index_state: Account<'info, IndexState>,

    pub authority: Signer<'info>,
}

pub fn update_epoch_scores_internal(
    ctx: Context<UpdateEpochScores>,
    score_deltas: [i64; NUM_FACTIONS],
) -> Result<()> {
    let index_state = &mut ctx.accounts.index_state;
    let clock = Clock::get()?;

    for (idx, delta) in score_deltas.iter().enumerate() {
        index_state.latest_scores[idx] = index_state.latest_scores[idx]
            .checked_add(*delta)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    let (ranks, _) = compute_rankings(&index_state.latest_scores);
    index_state.latest_ranks = ranks;
    index_state.score_updates_count = index_state
        .score_updates_count
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    index_state.last_update_ts = clock.unix_timestamp;

    emit!(EpochScoresUpdated {
        index_id: index_state.index_id,
        score_deltas,
        cumulative_scores: index_state.latest_scores,
        ranks,
        update_number: index_state.score_updates_count,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateEpochScores<'info> {
    #[account(
        mut,
        seeds = [INDEX_STATE_SEED, &[index_state.index_id]],
        bump = index_state.bump,
    )]
    pub index_state: Account<'info, IndexState>,

    #[account(
        seeds = [EPOCH_CONFIG_SEED],
        bump = epoch_config.bump,
        constraint = epoch_config.oracle_authority == authority.key() @ ErrorCode::InvalidOracleAuthority,
    )]
    pub epoch_config: Account<'info, EpochConfig>,

    pub authority: Signer<'info>,
}

pub fn update_risk_factor_internal(ctx: Context<UpdateRiskFactor>, risk_factor: u16) -> Result<()> {
    require!(risk_factor <= 1000, ErrorCode::InvalidParameters);

    let old_risk_factor = ctx.accounts.epoch_config.risk_factor;
    ctx.accounts.epoch_config.risk_factor = risk_factor;

    emit!(RiskFactorUpdated {
        old_risk_factor,
        new_risk_factor: risk_factor,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateRiskFactor<'info> {
    #[account(
        mut,
        seeds = [EPOCH_CONFIG_SEED],
        bump = epoch_config.bump,
        constraint = epoch_config.oracle_authority == authority.key() @ ErrorCode::InvalidOracleAuthority,
    )]
    pub epoch_config: Account<'info, EpochConfig>,

    pub authority: Signer<'info>,
}

pub fn settle_epoch_internal(ctx: Context<SettleEpoch>) -> Result<()> {
    let epoch_config = &mut ctx.accounts.epoch_config;
    let epoch_state = &mut ctx.accounts.epoch_state;
    let index_state = &ctx.accounts.index_state;
    let clock = Clock::get()?;

    require!(epoch_state.stage == 0, ErrorCode::EpochNotActive);
    require!(
        clock.unix_timestamp >= epoch_state.end_timestamp as i64,
        ErrorCode::EpochNotEnded
    );
    require!(
        index_state.index_id == epoch_state.index_id,
        ErrorCode::InvalidIndexState
    );

    epoch_state.risk_factor_snapshot = epoch_config.risk_factor;
    let pool_u128 = (epoch_state.total_dogebtc_mined_in_epoch as u128)
        .checked_mul(epoch_state.risk_factor_snapshot as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    epoch_state.epoch_mining_pool =
        u64::try_from(pool_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;

    epoch_state.final_scores = index_state.latest_scores;
    epoch_state.final_ranks = index_state.latest_ranks;

    for faction_id in 0..NUM_FACTIONS {
        let (direction, rank_delta) = resolve_direction_from_ranks(
            epoch_state.start_ranks[faction_id],
            epoch_state.final_ranks[faction_id],
        );
        epoch_state.rank_deltas[faction_id] = rank_delta;
        epoch_state.resolved_directions[faction_id] = direction.as_index() as u8;
    }

    compute_faction_reward_pools(epoch_state, epoch_config)?;
    epoch_state.stage = 1;

    epoch_config.current_epoch_id = epoch_config
        .current_epoch_id
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    epoch_config.last_epoch_start = clock.unix_timestamp.max(0) as u64;
    epoch_config.active_index_id = epoch_config.next_index_id;
    epoch_config.active_question_hash = epoch_config.next_question_hash;
    epoch_config.next_index_id = epoch_config.active_index_id;
    epoch_config.next_question_hash = epoch_config.active_question_hash;

    emit!(EpochSettled {
        epoch_id: epoch_state.epoch_id,
        index_id: epoch_state.index_id,
        question_hash: epoch_state.question_hash,
        total_dogebtc_mined: epoch_state.total_dogebtc_mined_in_epoch,
        risk_factor: epoch_state.risk_factor_snapshot,
        epoch_mining_pool: epoch_state.epoch_mining_pool,
        start_scores: epoch_state.start_scores,
        final_scores: epoch_state.final_scores,
        start_ranks: epoch_state.start_ranks,
        final_ranks: epoch_state.final_ranks,
        rank_deltas: epoch_state.rank_deltas,
        resolved_directions: epoch_state.resolved_directions,
        faction_reward_pools: epoch_state.faction_reward_pools,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

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
        seeds = [INDEX_STATE_SEED, &[index_state.index_id]],
        bump = index_state.bump,
        constraint = index_state.index_id == epoch_state.index_id @ ErrorCode::InvalidIndexState,
    )]
    pub index_state: Account<'info, IndexState>,

    pub authority: Signer<'info>,
}

pub fn claim_epoch_rewards_internal(ctx: Context<ClaimEpochRewards>, epoch_id: u64) -> Result<()> {
    let epoch_state = &ctx.accounts.epoch_state;
    let user_epoch_bets = &ctx.accounts.user_epoch_bets;
    let player_data = &mut ctx.accounts.player_data;
    let unrefined_rewards = &mut ctx.accounts.unrefined_rewards;
    let clock = Clock::get()?;

    require!(epoch_state.stage == 1, ErrorCode::EpochNotSettled);
    require!(epoch_state.epoch_mining_pool > 0, ErrorCode::InvalidState);

    let mut total_reward = 0u64;
    for faction_id in 0..NUM_FACTIONS {
        let resolved_direction = epoch_state.resolved_directions[faction_id] as usize;
        let user_bet = user_epoch_bets.direction_bets[faction_id][resolved_direction];
        let total_bet = epoch_state.faction_direction_totals[faction_id][resolved_direction];
        let faction_pool = epoch_state.faction_reward_pools[faction_id];

        if user_bet > 0 && total_bet > 0 && faction_pool > 0 {
            let reward_u128 = (faction_pool as u128)
                .checked_mul(user_bet as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(total_bet as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            let reward = u64::try_from(reward_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;
            total_reward = total_reward
                .checked_add(reward)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
    }

    if total_reward > 0 {
        helper::add_to_total_claimable(unrefined_rewards, player_data, total_reward)?;
        player_data.total_dogebtc_won = player_data
            .total_dogebtc_won
            .checked_add(total_reward)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    emit!(EpochRewardsClaimed {
        epoch_id,
        index_id: epoch_state.index_id,
        user: user_epoch_bets.owner,
        reward_amount: total_reward,
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
    pub epoch_state: Account<'info, EpochState>,

    #[account(
        mut,
        seeds = [USER_EPOCH_BETS_SEED, user_epoch_bets.owner.as_ref(), &epoch_id.to_le_bytes()],
        bump = user_epoch_bets.bump,
        close = player,
    )]
    pub user_epoch_bets: Account<'info, UserEpochBets>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, user_epoch_bets.owner.as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user_epoch_bets.owner @ ErrorCode::InvalidOwner,
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(
        mut,
        seeds = [UNREFINED_REWARDS_SEED],
        bump,
    )]
    pub unrefined_rewards: Account<'info, UnrefinedRewards>,

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
