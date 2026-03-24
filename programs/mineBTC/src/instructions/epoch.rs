// # Epoch Mining Instructions
//
// Implements the geopolitical prediction epoch mining system.
// Each 24-hour epoch distributes additional dogeBTC based on:
//   epoch_pool = total_dogebtc_mined_in_epoch * risk_factor / 100
// Users earn rewards proportional to: sum(their_faction_bets[i] * faction_scores[i])
//
// ## Instructions:
// - `initialize_epoch_config`: Admin setup
// - `start_epoch`: Begin a new epoch (cranker/admin)
// - `update_epoch_scores`: AI oracle posts faction score deltas (additive, with memo)
// - `update_risk_factor`: AI oracle updates global risk factor
// - `settle_epoch`: Finalize scores and compute reward denominator
// - `claim_epoch_rewards`: User claims their epoch dogeBTC rewards
// - `accumulate_epoch_bets`: Cranker records per-user per-faction bets from round data

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
// ============================= START EPOCH ==============================
// ========================================================================================

pub fn start_epoch_internal(ctx: Context<StartEpoch>) -> Result<()> {
    let epoch_config = &mut ctx.accounts.epoch_config;
    let clock = Clock::get()?;

    require!(epoch_config.is_active, ErrorCode::EpochNotActive);

    // If there's a previous epoch, it must be settled (stage 2)
    // For the very first epoch (current_epoch_id == 0), skip this check
    // The previous epoch check is done by the caller ensuring last epoch is settled

    let new_epoch_id = epoch_config.current_epoch_id + 1;
    let start_ts = clock.unix_timestamp as u64;
    let end_ts = start_ts + epoch_config.epoch_duration;

    let epoch_state = &mut ctx.accounts.epoch_state;
    epoch_state.bump = ctx.bumps.epoch_state;
    epoch_state.epoch_id = new_epoch_id;
    epoch_state.start_timestamp = start_ts;
    epoch_state.end_timestamp = end_ts;
    epoch_state.stage = 0; // active
    epoch_state.total_dogebtc_mined_in_epoch = 0;
    epoch_state.risk_factor_snapshot = 0;
    epoch_state.epoch_mining_pool = 0;
    epoch_state.faction_scores = [0u16; NUM_FACTIONS];
    epoch_state.faction_total_sol_bets = [0u64; NUM_FACTIONS];
    epoch_state.total_score_weighted_bets = 0;
    epoch_state.score_updates_count = 0;

    epoch_config.current_epoch_id = new_epoch_id;
    epoch_config.last_epoch_start = start_ts;

    msg!("🌍 [start_epoch] Epoch {} started. Window: {} -> {}",
        new_epoch_id, start_ts, end_ts);

    emit!(EpochStarted {
        epoch_id: new_epoch_id,
        start_timestamp: start_ts,
        end_timestamp: end_ts,
        risk_factor: epoch_config.risk_factor,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct StartEpoch<'info> {
    #[account(
        mut,
        seeds = [EPOCH_CONFIG_SEED],
        bump = epoch_config.bump,
    )]
    pub epoch_config: Account<'info, EpochConfig>,

    #[account(
        init,
        payer = authority,
        space = EpochState::LEN,
        seeds = [EPOCH_STATE_SEED, &(epoch_config.current_epoch_id + 1).to_le_bytes()],
        bump,
    )]
    pub epoch_state: Account<'info, EpochState>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
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

    msg!("   Scores after update: {:?}", epoch_state.faction_scores);
    msg!("   (Reason should be in transaction memo)");

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
// ============================= ACCUMULATE EPOCH BETS ==============================
// ========================================================================================

/// Cranker records a user's per-faction bets for the current epoch from a completed round.
/// Reads the UserGameBet for a given round and maps block bets → faction bets using
/// the GameSession's block_assignments. Also accumulates into EpochState totals.
pub fn accumulate_epoch_bets_internal(ctx: Context<AccumulateEpochBets>) -> Result<()> {
    let game_session = &ctx.accounts.game_session;
    let user_game_bet = &mut ctx.accounts.user_game_bet;
    let epoch_state = &mut ctx.accounts.epoch_state;
    let user_epoch_bets = &mut ctx.accounts.user_epoch_bets;

    require!(epoch_state.stage == 0, ErrorCode::EpochAlreadySettled);
    require!(!user_game_bet.epoch_accumulated, ErrorCode::EpochAlreadySettled);

    // Verify this round falls within the epoch window
    require!(
        game_session.round_start_timestamp >= epoch_state.start_timestamp as i64
            && game_session.round_start_timestamp < epoch_state.end_timestamp as i64,
        ErrorCode::InvalidRound
    );

    // Initialize UserEpochBets if first time
    if user_epoch_bets.owner == Pubkey::default() {
        user_epoch_bets.owner = user_game_bet.owner;
        user_epoch_bets.epoch_id = epoch_state.epoch_id;
        user_epoch_bets.faction_bets = [0u64; NUM_FACTIONS];
        user_epoch_bets.claimed = false;
        user_epoch_bets.bump = ctx.bumps.user_epoch_bets;
    }

    // Map each block bet to its faction
    for (idx, &block_id) in user_game_bet.block_ids.iter().enumerate() {
        let faction_id = game_session.block_assignments[block_id as usize] as usize;
        if faction_id < NUM_FACTIONS {
            let sol_bet = user_game_bet.sol_bets[idx];
            // Apply active multiplier (wgtd_points gives us the multiplier-weighted amount)
            let wgtd_bet = user_game_bet.wgtd_points_bets[idx];

            user_epoch_bets.faction_bets[faction_id] += wgtd_bet;
            epoch_state.faction_total_sol_bets[faction_id] += wgtd_bet;

            msg!("   Block {} -> Faction {}: {} wgtd_points (sol: {})",
                block_id, faction_id, wgtd_bet, sol_bet);
        }
    }

    msg!("🌍 [accumulate_epoch_bets] User {} round {} -> epoch {}",
        user_game_bet.owner, game_session.round_id, epoch_state.epoch_id);

    user_game_bet.epoch_accumulated = true;

    Ok(())
}

#[derive(Accounts)]
#[instruction()]
pub struct AccumulateEpochBets<'info> {
    #[account(
        mut,
        seeds = [EPOCH_STATE_SEED, &epoch_state.epoch_id.to_le_bytes()],
        bump = epoch_state.bump,
    )]
    pub epoch_state: Account<'info, EpochState>,

    /// The completed game session (any stage, just need block_assignments)
    pub game_session: Account<'info, GameSession>,

    /// The user's bet for that round
    #[account(mut)]
    pub user_game_bet: Account<'info, UserGameBet>,

    #[account(
        init_if_needed,
        payer = payer,
        space = UserEpochBets::LEN,
        seeds = [USER_EPOCH_BETS_SEED, user_game_bet.owner.as_ref(), &epoch_state.epoch_id.to_le_bytes()],
        bump,
    )]
    pub user_epoch_bets: Account<'info, UserEpochBets>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [EPOCH_CONFIG_SEED],
        bump = epoch_config.bump,
        constraint = epoch_config.oracle_authority == payer.key() @ ErrorCode::InvalidOracleAuthority,
    )]
    pub epoch_config: Account<'info, EpochConfig>,

    pub system_program: Program<'info, System>,
}

// ========================================================================================
// ============================= RECORD EPOCH ROUND MINING ==============================
// ========================================================================================

/// Called by cranker after end_round to record the dogeBTC mined in that round
/// towards the current epoch's total. This is how the epoch pool grows.
pub fn record_epoch_round_mining_internal(
    ctx: Context<RecordEpochRoundMining>,
    dogebtc_mined: u64,
) -> Result<()> {
    let epoch_state = &mut ctx.accounts.epoch_state;

    require!(epoch_state.stage == 0, ErrorCode::EpochAlreadySettled);

    epoch_state.total_dogebtc_mined_in_epoch += dogebtc_mined;

    msg!("🌍 [record_epoch_round_mining] Epoch {}: +{} dogeBTC mined (total: {})",
        epoch_state.epoch_id, dogebtc_mined, epoch_state.total_dogebtc_mined_in_epoch);

    Ok(())
}

#[derive(Accounts)]
pub struct RecordEpochRoundMining<'info> {
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

    /// Cranker or oracle authority
    pub authority: Signer<'info>,
}

// ========================================================================================
// ============================= SETTLE EPOCH ==============================
// ========================================================================================

/// Finalizes an epoch: snapshots risk_factor, computes epoch_mining_pool,
/// calculates total_score_weighted_bets denominator.
/// Can only be called after epoch end_timestamp has passed.
pub fn settle_epoch_internal(ctx: Context<SettleEpoch>) -> Result<()> {
    let epoch_config = &ctx.accounts.epoch_config;
    let epoch_state = &mut ctx.accounts.epoch_state;
    let clock = Clock::get()?;

    require!(epoch_state.stage == 0, ErrorCode::EpochAlreadySettled);
    require!(
        clock.unix_timestamp as u64 >= epoch_state.end_timestamp,
        ErrorCode::EpochNotEnded
    );

    // Snapshot risk factor
    epoch_state.risk_factor_snapshot = epoch_config.risk_factor;

    // Calculate epoch mining pool: total_dogebtc_mined * risk_factor / 100
    // risk_factor is 0-1000 representing 0.00-10.00x
    // So pool = total_mined * risk_factor / 100
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

    msg!("🌍 [settle_epoch] Epoch {} settled:", epoch_state.epoch_id);
    msg!("   Total dogeBTC mined in epoch: {}", epoch_state.total_dogebtc_mined_in_epoch);
    msg!("   Risk factor: {}", epoch_state.risk_factor_snapshot);
    msg!("   Epoch mining pool: {}", epoch_state.epoch_mining_pool);
    msg!("   Total score-weighted bets: {}", total_weighted);
    msg!("   Faction scores: {:?}", epoch_state.faction_scores);

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
        seeds = [EPOCH_CONFIG_SEED],
        bump = epoch_config.bump,
        constraint = epoch_config.oracle_authority == authority.key() @ ErrorCode::InvalidOracleAuthority,
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
pub fn claim_epoch_rewards_internal(ctx: Context<ClaimEpochRewards>) -> Result<()> {
    let epoch_state = &ctx.accounts.epoch_state;
    let user_epoch_bets = &mut ctx.accounts.user_epoch_bets;
    let player_data = &mut ctx.accounts.player_data;
    let _mine_btc_mining = &ctx.accounts.mine_btc_mining;
    let clock = Clock::get()?;

    require!(epoch_state.stage == 2, ErrorCode::EpochNotSettled);
    require!(!user_epoch_bets.claimed, ErrorCode::EpochRewardsAlreadyClaimed);
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

    if user_weighted == 0 {
        msg!("   User has no score-weighted bets, no rewards");
        user_epoch_bets.claimed = true;
        return Ok(());
    }

    // reward = user_weighted * epoch_mining_pool / total_score_weighted_bets
    let reward = (user_weighted)
        .checked_mul(epoch_state.epoch_mining_pool as u128)
        .unwrap_or(0)
        .checked_div(epoch_state.total_score_weighted_bets)
        .unwrap_or(0) as u64;

    if reward > 0 {
        // Add to pending rewards (same path as round rewards, claimed via withdraw_dbtc_rewards)
        player_data.pending_minebtc_rewards += reward;

        msg!("🌍 [claim_epoch_rewards] User {} earned {} dogeBTC from epoch {}",
            user_epoch_bets.owner, reward, epoch_state.epoch_id);
    }

    user_epoch_bets.claimed = true;

    emit!(EpochRewardsClaimed {
        epoch_id: epoch_state.epoch_id,
        user: user_epoch_bets.owner,
        reward_amount: reward,
        user_weighted_score: user_weighted,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimEpochRewards<'info> {
    #[account(
        seeds = [EPOCH_STATE_SEED, &epoch_state.epoch_id.to_le_bytes()],
        bump = epoch_state.bump,
    )]
    pub epoch_state: Account<'info, EpochState>,

    #[account(
        mut,
        seeds = [USER_EPOCH_BETS_SEED, authority.key().as_ref(), &epoch_state.epoch_id.to_le_bytes()],
        bump = user_epoch_bets.bump,
        constraint = user_epoch_bets.owner == authority.key() @ ErrorCode::InvalidOwner,
    )]
    pub user_epoch_bets: Account<'info, UserEpochBets>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::InvalidOwner,
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(
        seeds = [MINE_BTC_MINING_SEED],
        bump = mine_btc_mining.bump,
    )]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

    pub authority: Signer<'info>,
}
