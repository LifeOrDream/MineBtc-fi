// # Epoch Mining Instructions (Inline Design — Parimutuel + Top 3)
//
// Implements the geopolitical prediction epoch mining system (Nation Pulse Index).
// Each 24-hour epoch distributes additional dogeBTC based on:
//   epoch_pool = total_dogebtc_mined_in_epoch * risk_factor / 100
//
// ## Reward Model (Model 5 + Top 3 Bonus):
//   1. model5_pct% of pool distributed to ALL factions proportionally by score
//      faction_model5[i] = model5_pool * score[i] / total_scores
//   2. Top 3 bonus: top1_pct/top2_pct/top3_pct% of pool given to #1/#2/#3 ranked factions
//   3. faction_reward_pools[i] = model5[i] + top3_bonus[i]
//   4. User reward = sum over factions { faction_reward_pools[i] * user_bets[i] / total_bets[i] }
//
// This creates parimutuel dynamics: fewer bettors on a faction = higher reward per SOL.
// Contrarian bets on underdog factions that score well = massive payouts.
//
// ## Inline Design:
// - Epoch bet accumulation happens inline in join_round/join_round_batch/execute_autominer_bet
// - Epoch mining tracking + auto-settle + auto-start happens inline in end_round_faction_rewards
//
// ## Instructions:
// - `initialize_epoch_config`: Admin setup (with reward split percentages)
// - `update_epoch_config`: Admin updates
// - `update_epoch_scores`: AI oracle posts faction score deltas
// - `update_risk_factor`: AI oracle updates global risk factor
// - `settle_epoch`: Fallback settlement (anyone can call)
// - `claim_epoch_rewards`: User claims their epoch dogeBTC rewards (closes account)

use anchor_lang::prelude::*;

use crate::errors::ErrorCode;
use crate::events::*;
use crate::instructions::helper;
use crate::state::*;

// ========================================================================================
// ============================= SETTLEMENT HELPER ==============================
// ========================================================================================

/// Computes faction_reward_pools using Model 5 (parimutuel by score) + Top 3 bonus.
/// This is shared by settle_epoch (fallback) and auto-settle (in end_round_faction_rewards).
pub fn compute_faction_reward_pools(epoch_state: &mut EpochState, epoch_config: &EpochConfig) {
    let pool = epoch_state.epoch_mining_pool;
    if pool == 0 {
        epoch_state.faction_reward_pools = [0u64; NUM_FACTIONS];
        return;
    }

    // --- Step 1: Model 5 — distribute model5_pct% of pool by score ratio ---
    let model5_pool = pool as u128 * epoch_config.model5_pct as u128 / 100;
    let total_scores: u128 = epoch_state.faction_scores.iter().map(|&s| s as u128).sum();

    let mut reward_pools = [0u64; NUM_FACTIONS];
    if total_scores > 0 {
        for i in 0..NUM_FACTIONS {
            reward_pools[i] =
                (model5_pool * epoch_state.faction_scores[i] as u128 / total_scores) as u64;
        }
    }

    // --- Step 2: Find top 3 factions by score ---
    // Build (score, faction_index) pairs and find top 3
    let mut ranked: [(u16, usize); NUM_FACTIONS] = [(0, 0); NUM_FACTIONS];
    for i in 0..NUM_FACTIONS {
        ranked[i] = (epoch_state.faction_scores[i], i);
    }
    // Sort descending by score (simple selection sort — only 12 elements)
    for i in 0..NUM_FACTIONS {
        let mut max_idx = i;
        for j in (i + 1)..NUM_FACTIONS {
            if ranked[j].0 > ranked[max_idx].0 {
                max_idx = j;
            }
        }
        ranked.swap(i, max_idx);
    }

    // --- Step 3: Add top 3 bonus pools ---
    let top1_bonus = pool as u128 * epoch_config.top1_pct as u128 / 100;
    let top2_bonus = pool as u128 * epoch_config.top2_pct as u128 / 100;
    let top3_bonus = pool as u128 * epoch_config.top3_pct as u128 / 100;

    if ranked[0].0 > 0 {
        reward_pools[ranked[0].1] += top1_bonus as u64;
    }
    if NUM_FACTIONS > 1 && ranked[1].0 > 0 {
        reward_pools[ranked[1].1] += top2_bonus as u64;
    }
    if NUM_FACTIONS > 2 && ranked[2].0 > 0 {
        reward_pools[ranked[2].1] += top3_bonus as u64;
    }

    epoch_state.faction_reward_pools = reward_pools;

    msg!("   🌍 Faction reward pools computed. Top3: #1={} (faction {}), #2={} (faction {}), #3={} (faction {})",
        ranked[0].0, ranked[0].1, ranked[1].0, ranked[1].1, ranked[2].0, ranked[2].1);
}

// ========================================================================================
// ============================= INITIALIZE EPOCH CONFIG ==============================
// ========================================================================================

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
    msg!("🌍 [initialize_epoch_config] Setting up epoch mining system");

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
    epoch_config.current_epoch_id = 0;
    epoch_config.last_epoch_start = 0;
    epoch_config.risk_factor = risk_factor;
    epoch_config.is_active = true;
    epoch_config.model5_pct = model5_pct;
    epoch_config.top1_pct = top1_pct;
    epoch_config.top2_pct = top2_pct;
    epoch_config.top3_pct = top3_pct;

    msg!("   ✅ Epoch config initialized: oracle={}, duration={}s, risk_factor={}, split={}/{}+{}+{}",
        oracle_authority, epoch_duration, risk_factor, model5_pct, top1_pct, top2_pct, top3_pct);

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
    model5_pct: Option<u8>,
    top1_pct: Option<u8>,
    top2_pct: Option<u8>,
    top3_pct: Option<u8>,
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

    // Validate sum
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

    msg!(
        "🌍 [update_epoch_scores] Epoch {} score update #{}",
        epoch_state.epoch_id,
        epoch_state.score_updates_count + 1
    );

    for i in 0..NUM_FACTIONS {
        epoch_state.faction_scores[i] =
            epoch_state.faction_scores[i].saturating_add(score_deltas[i]);
    }

    epoch_state.score_updates_count += 1;
    epoch_state.stage = 1; // scores posted, enables auto-settlement

    msg!("   Scores after update: {:?}", epoch_state.faction_scores);

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

pub fn update_risk_factor_internal(ctx: Context<UpdateRiskFactor>, risk_factor: u16) -> Result<()> {
    require!(risk_factor <= 1000, ErrorCode::InvalidParameters);

    let old_risk_factor = ctx.accounts.epoch_config.risk_factor;
    ctx.accounts.epoch_config.risk_factor = risk_factor;

    let clock = Clock::get()?;
    msg!(
        "🌍 [update_risk_factor] {} -> {}",
        old_risk_factor,
        risk_factor
    );

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

/// Fallback settlement: anyone can call after epoch end_timestamp has passed and scores posted.
/// Normally auto-settled in end_round_faction_rewards.
pub fn settle_epoch_internal(ctx: Context<SettleEpoch>) -> Result<()> {
    let epoch_config = &mut ctx.accounts.epoch_config;
    let epoch_state = &mut ctx.accounts.epoch_state;
    let clock = Clock::get()?;

    require!(epoch_state.stage == 1, ErrorCode::EpochNotActive);
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

    // Compute faction reward pools (Model 5 + Top 3)
    compute_faction_reward_pools(epoch_state, epoch_config);

    epoch_state.stage = 2; // settled, claims open

    // Auto-start next epoch
    epoch_config.current_epoch_id += 1;
    epoch_config.last_epoch_start = clock.unix_timestamp as u64;

    msg!("🌍 [settle_epoch] Epoch {} settled:", epoch_state.epoch_id);
    msg!(
        "   Pool: {}, Faction pools: {:?}",
        epoch_state.epoch_mining_pool,
        epoch_state.faction_reward_pools
    );
    msg!("   Next epoch_id: {}", epoch_config.current_epoch_id);

    emit!(EpochSettled {
        epoch_id: epoch_state.epoch_id,
        total_dogebtc_mined: epoch_state.total_dogebtc_mined_in_epoch,
        risk_factor: epoch_state.risk_factor_snapshot,
        epoch_mining_pool: epoch_state.epoch_mining_pool,
        faction_scores: epoch_state.faction_scores,
        total_score_weighted_bets: 0, // deprecated, kept for event compat
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

/// User claims their epoch mining rewards using parimutuel formula:
/// reward = sum over factions { faction_reward_pools[i] * user_bets[i] / total_bets[i] }
/// Rewards go through the refining system (add_to_total_claimable) so the 10% refining
/// fee applies when user later withdraws, and other holders accrue from it.
/// The user_epoch_bets account is CLOSED (rent returned to user).
pub fn claim_epoch_rewards_internal(ctx: Context<ClaimEpochRewards>, epoch_id: u64) -> Result<()> {
    let epoch_state = &ctx.accounts.epoch_state;
    let user_epoch_bets = &ctx.accounts.user_epoch_bets;
    let player_data = &mut ctx.accounts.player_data;
    let unrefined_rewards = &mut ctx.accounts.unrefined_rewards;
    let clock = Clock::get()?;

    require!(epoch_state.stage == 2, ErrorCode::EpochNotSettled);
    require!(epoch_state.epoch_mining_pool > 0, ErrorCode::InvalidState);

    // Calculate user reward: sum across all factions
    let mut total_reward: u64 = 0;
    for i in 0..NUM_FACTIONS {
        let user_bet = user_epoch_bets.faction_bets[i];
        let total_bet = epoch_state.faction_total_sol_bets[i];
        let faction_pool = epoch_state.faction_reward_pools[i];

        if user_bet > 0 && total_bet > 0 && faction_pool > 0 {
            // reward_from_faction = faction_pool * user_bet / total_bet
            let reward = (faction_pool as u128)
                .checked_mul(user_bet as u128)
                .unwrap_or(0)
                .checked_div(total_bet as u128)
                .unwrap_or(0) as u64;
            total_reward += reward;
        }
    }

    if total_reward > 0 {
        // Use refining system: accrues unrefined rewards, updates total_minebtc_claimable,
        // syncs player unrefining_index. This ensures the 10% refining fee applies on withdrawal.
        let accrued = helper::add_to_total_claimable(unrefined_rewards, player_data, total_reward);

        // Track in player stats
        player_data.total_dogebtc_won += total_reward;

        msg!("🌍 [claim_epoch_rewards] User {} earned {} dogeBTC from epoch {} (accrued refining bonus: {})",
            user_epoch_bets.owner, total_reward, epoch_id, accrued);
    } else {
        msg!(
            "🌍 [claim_epoch_rewards] User {} has no rewards for epoch {}",
            user_epoch_bets.owner,
            epoch_id
        );
    }

    emit!(EpochRewardsClaimed {
        epoch_id,
        user: user_epoch_bets.owner,
        reward_amount: total_reward,
        user_weighted_score: 0, // deprecated field, kept for event compat
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
        close = authority,
    )]
    pub user_epoch_bets: Account<'info, UserEpochBets>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::InvalidOwner,
    )]
    pub player_data: Account<'info, PlayerData>,

    /// Unrefined rewards tracker (for refining index system)
    #[account(
        mut,
        seeds = [UNREFINED_REWARDS_SEED],
        bump,
    )]
    pub unrefined_rewards: Account<'info, UnrefinedRewards>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
