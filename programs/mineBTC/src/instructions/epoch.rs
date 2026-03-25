// # Epoch Mining Instructions (Inline Design)
//
// Implements the geopolitical prediction epoch mining system.
// Each 24-hour epoch distributes additional dogeBTC based on:
//   epoch_pool = total_dogebtc_mined_in_epoch * risk_factor / 100
// Users earn rewards proportional to: sum(their_faction_bets[i] * faction_scores[i])
//
// ## Inline Design:
// - Epoch bet accumulation happens inline in join_round/join_round_batch/execute_autominer_bet
// - Epoch mining tracking + auto-settle + auto-start happens inline in end_round_faction_rewards
// - No separate cranker calls for start_epoch, accumulate_epoch_bets, or record_epoch_round_mining
//
// ## Instructions (kept):
// - `initialize_epoch_config`: Admin setup
// - `update_epoch_config`: Admin updates
// - `update_epoch_scores`: AI oracle posts faction score deltas (additive, with memo)
// - `update_risk_factor`: AI oracle updates global risk factor
// - `settle_epoch`: Fallback settlement (anyone can call after epoch ends + scores posted)
// - `claim_epoch_rewards`: User claims their epoch dogeBTC rewards (closes account)

use anchor_lang::prelude::*;

use crate::errors::ErrorCode;
use crate::events::*;
use crate::state::*;

// ========================================================================================
// ============================= INITIALIZE EPOCH CONFIG ==============================
// ========================================================================================

pub fn initialize_epoch_config_internal(
    ctx: Context<InitializeEpochConfig>,
    oracle_authority: Pubkey,
    epoch_duration: u64,
    risk_factor: u16,
) -> Result<()> {
    msg!("🌍 [initialize_epoch_config] Setting up epoch mining system");

    require!(epoch_duration > 0, ErrorCode::InvalidParameters);
    require!(risk_factor <= 1000, ErrorCode::InvalidParameters); // max 10.00x

    let epoch_config = &mut ctx.accounts.epoch_config;
    epoch_config.bump = ctx.bumps.epoch_config;
    epoch_config.oracle_authority = oracle_authority;
    epoch_config.epoch_duration = epoch_duration;
    epoch_config.current_epoch_id = 0;
    epoch_config.last_epoch_start = 0;
    epoch_config.risk_factor = risk_factor;
    epoch_config.is_active = true;

    msg!("   ✅ Epoch config initialized: oracle={}, duration={}s, risk_factor={}",
        oracle_authority, epoch_duration, risk_factor);

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

// ========================================================================================
// ============================= UPDATE EPOCH CONFIG ==============================
// ========================================================================================

pub fn update_epoch_config_internal(
    ctx: Context<UpdateEpochConfig>,
    oracle_authority: Option<Pubkey>,
    epoch_duration: Option<u64>,
    is_active: Option<bool>,
) -> Result<()> {
    msg!("🌍 [update_epoch_config] Updating epoch config");

    let epoch_config = &mut ctx.accounts.epoch_config;

    if let Some(auth) = oracle_authority {
        epoch_config.oracle_authority = auth;
        msg!("   Updated oracle_authority: {}", auth);
    }
    if let Some(dur) = epoch_duration {
        require!(dur > 0, ErrorCode::InvalidParameters);
        epoch_config.epoch_duration = dur;
        msg!("   Updated epoch_duration: {}s", dur);
    }
    if let Some(active) = is_active {
        epoch_config.is_active = active;
        msg!("   Updated is_active: {}", active);
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
// ============================= UPDATE EPOCH SCORES (AI ORACLE) ==============================
// ========================================================================================

/// AI Oracle posts additive score deltas for factions.
/// Called every 15-30 min with score increments and a reason in the tx memo.
/// Scores accumulate over the epoch. Each faction's score is a u16 (0-10000 bps).
pub fn update_epoch_scores_internal(
    ctx: Context<UpdateEpochScores>,
    score_deltas: [u16; NUM_FACTIONS],
) -> Result<()> {
    let epoch_state = &mut ctx.accounts.epoch_state;
    let clock = Clock::get()?;

    require!(epoch_state.stage == 0, ErrorCode::EpochAlreadySettled);

    msg!("🌍 [update_epoch_scores] Epoch {} score update #{}",
        epoch_state.epoch_id, epoch_state.score_updates_count + 1);

    // Add deltas to current scores (saturating to prevent overflow)
    for i in 0..NUM_FACTIONS {
        epoch_state.faction_scores[i] = epoch_state.faction_scores[i].saturating_add(score_deltas[i]);
    }

    epoch_state.score_updates_count += 1;

    // After scores are posted, set stage to 1 (scores finalized)
    // This enables auto-settlement in end_round_faction_rewards
    epoch_state.stage = 1;

    msg!("   Scores after update: {:?}", epoch_state.faction_scores);
    msg!("   Stage set to 1 (scores posted)");

    emit!(EpochScoresUpdated {
        epoch_id: epoch_state.epoch_id,
        score_deltas,
        cumulative_scores: epoch_state.faction_scores,
        update_number: epoch_state.score_updates_count,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateEpochScores<'info> {
    #[account(
        mut,
        seeds = [EPOCH_STATE_SEED, &epoch_state.epoch_id.to_le_bytes()],
        bump = epoch_state.bump,
    )]
    pub epoch_state: Account<'info, EpochState>,

    #[account(
        seeds = [EPOCH_CONFIG_SEED],
        bump = epoch_config.bump,
        constraint = epoch_config.oracle_authority == authority.key() @ ErrorCode::InvalidOracleAuthority,
    )]
    pub epoch_config: Account<'info, EpochConfig>,

    pub authority: Signer<'info>,
}

// ========================================================================================
// ============================= UPDATE RISK FACTOR ==============================
// ========================================================================================

/// AI Oracle updates the global risk factor based on world volatility.
/// risk_factor: 0-1000 (representing 0.00x to 10.00x multiplier on epoch mining pool).
pub fn update_risk_factor_internal(
    ctx: Context<UpdateRiskFactor>,
    risk_factor: u16,
) -> Result<()> {
    require!(risk_factor <= 1000, ErrorCode::InvalidParameters);

    let old_risk_factor = ctx.accounts.epoch_config.risk_factor;
    ctx.accounts.epoch_config.risk_factor = risk_factor;

    let clock = Clock::get()?;
    msg!("🌍 [update_risk_factor] {} -> {} (reason in memo)",
        old_risk_factor, risk_factor);

    emit!(RiskFactorUpdated {
        old_risk_factor,
        new_risk_factor: risk_factor,
        timestamp: clock.unix_timestamp,
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

// ========================================================================================
// ============================= SETTLE EPOCH (FALLBACK) ==============================
// ========================================================================================

/// Fallback settlement: anyone can call after epoch end_timestamp has passed and scores are posted.
/// Normally auto-settled in end_round_faction_rewards, but this provides a manual fallback.
pub fn settle_epoch_internal(ctx: Context<SettleEpoch>) -> Result<()> {
    let epoch_config = &mut ctx.accounts.epoch_config;
    let epoch_state = &mut ctx.accounts.epoch_state;
    let clock = Clock::get()?;

    require!(epoch_state.stage == 1, ErrorCode::EpochNotActive); // scores must be posted (stage 1)
    require!(
        clock.unix_timestamp as u64 >= epoch_state.end_timestamp,
        ErrorCode::EpochNotEnded
    );

    // Snapshot risk factor
    epoch_state.risk_factor_snapshot = epoch_config.risk_factor;

    // Calculate epoch mining pool: total_dogebtc_mined * risk_factor / 100
    epoch_state.epoch_mining_pool = (epoch_state.total_dogebtc_mined_in_epoch as u128)
        .checked_mul(epoch_state.risk_factor_snapshot as u128)
        .unwrap_or(0)
        .checked_div(100)
        .unwrap_or(0) as u64;

    // Calculate denominator: sum(faction_total_sol_bets[i] * faction_scores[i])
    let mut total_weighted: u128 = 0;
    for i in 0..NUM_FACTIONS {
        total_weighted += (epoch_state.faction_total_sol_bets[i] as u128)
            * (epoch_state.faction_scores[i] as u128);
    }
    epoch_state.total_score_weighted_bets = total_weighted;

    epoch_state.stage = 2; // settled, claims open

    // Auto-start next epoch by incrementing current_epoch_id
    epoch_config.current_epoch_id += 1;
    epoch_config.last_epoch_start = clock.unix_timestamp as u64;

    msg!("🌍 [settle_epoch] Epoch {} settled:", epoch_state.epoch_id);
    msg!("   Total dogeBTC mined in epoch: {}", epoch_state.total_dogebtc_mined_in_epoch);
    msg!("   Risk factor: {}", epoch_state.risk_factor_snapshot);
    msg!("   Epoch mining pool: {}", epoch_state.epoch_mining_pool);
    msg!("   Total score-weighted bets: {}", total_weighted);
    msg!("   Next epoch_id: {}", epoch_config.current_epoch_id);

    emit!(EpochSettled {
        epoch_id: epoch_state.epoch_id,
        total_dogebtc_mined: epoch_state.total_dogebtc_mined_in_epoch,
        risk_factor: epoch_state.risk_factor_snapshot,
        epoch_mining_pool: epoch_state.epoch_mining_pool,
        faction_scores: epoch_state.faction_scores,
        total_score_weighted_bets: total_weighted,
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

    pub authority: Signer<'info>,
}

// ========================================================================================
// ============================= CLAIM EPOCH REWARDS ==============================
// ========================================================================================

/// User claims their epoch mining rewards.
/// Reward = (sum(user_faction_bets[i] * faction_scores[i]) / total_score_weighted_bets) * epoch_mining_pool
/// dogeBTC is added to player_data.pending_minebtc_rewards (same as round rewards).
/// The user_epoch_bets account is CLOSED (rent returned to user). Account existence = unclaimed.
pub fn claim_epoch_rewards_internal(ctx: Context<ClaimEpochRewards>, epoch_id: u64) -> Result<()> {
    let epoch_state = &ctx.accounts.epoch_state;
    let user_epoch_bets = &ctx.accounts.user_epoch_bets;
    let player_data = &mut ctx.accounts.player_data;
    let clock = Clock::get()?;

    require!(epoch_state.stage == 2, ErrorCode::EpochNotSettled);
    require!(
        epoch_state.total_score_weighted_bets > 0,
        ErrorCode::InvalidState
    );
    require!(epoch_state.epoch_mining_pool > 0, ErrorCode::InvalidState);

    // Calculate user's score-weighted bets
    let mut user_weighted: u128 = 0;
    for i in 0..NUM_FACTIONS {
        user_weighted += (user_epoch_bets.faction_bets[i] as u128)
            * (epoch_state.faction_scores[i] as u128);
    }

    // reward = user_weighted * epoch_mining_pool / total_score_weighted_bets
    let reward = if user_weighted > 0 {
        (user_weighted)
            .checked_mul(epoch_state.epoch_mining_pool as u128)
            .unwrap_or(0)
            .checked_div(epoch_state.total_score_weighted_bets)
            .unwrap_or(0) as u64
    } else {
        0
    };

    if reward > 0 {
        // Add to pending rewards (same path as round rewards, claimed via withdraw_dbtc_rewards)
        player_data.pending_minebtc_rewards += reward;

        msg!("🌍 [claim_epoch_rewards] User {} earned {} dogeBTC from epoch {}",
            user_epoch_bets.owner, reward, epoch_id);
    } else {
        msg!("🌍 [claim_epoch_rewards] User {} has no rewards for epoch {}",
            user_epoch_bets.owner, epoch_id);
    }

    // Account will be closed by Anchor `close = authority` attribute -> rent returned

    emit!(EpochRewardsClaimed {
        epoch_id,
        user: user_epoch_bets.owner,
        reward_amount: reward,
        user_weighted_score: user_weighted,
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
        seeds = [USER_EPOCH_BETS_SEED, authority.key().as_ref(), &epoch_id.to_le_bytes()],
        bump = user_epoch_bets.bump,
        constraint = user_epoch_bets.owner == authority.key() @ ErrorCode::InvalidOwner,
        close = authority, // CLOSE the account, rent returned to user
    )]
    pub user_epoch_bets: Account<'info, UserEpochBets>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::InvalidOwner,
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
