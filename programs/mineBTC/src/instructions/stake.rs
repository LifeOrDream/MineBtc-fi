use anchor_lang::prelude::*;
use anchor_lang::system_program::System;
use anchor_spl::token::{self, Token};

// # Staking Instructions
//
// This module implements the staking system for MineBTC and LP tokens.
//
// ## Staking Mechanics
//
// Players can stake MineBTC or LP tokens in their home faction to earn passive rewards:
// - **SOL Rewards**: Distributed from staker fees collected on game bets.
// - **MineBTC Rewards**: Distributed from the mining emission pool.
//
// Longer lockup periods grant higher hashpower multipliers, increasing reward share.
//
// ## Key Functions
//
// - `stake_minebtc`: Stakes MineBTC tokens with a specified lockup duration.
// - `unstake_minebtc`: Unstakes MineBTC tokens after the lockup period expires.
// - `stake_lp_tokens`: Stakes LP tokens for dual rewards (SOL + MineBTC).
// - `unstake_lp_tokens`: Unstakes LP tokens after lockup.
// - `claim_sol_rewards`: Claims accumulated SOL rewards from staking.
// - `claim_minebtc_rewards`: Claims accumulated MineBTC rewards (with refining fee).
// - `claim_referral_rewards`: Claims referral earnings.
//
// The refining fee on MineBTC claims is redistributed to all stakers, creating a deflationary reward loop.
//

use crate::errors::ErrorCode;
use crate::events::*;
use crate::instructions::helper;
use crate::state::*;

use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface;
use anchor_spl::token_interface::{Mint as Mint2022, TokenAccount as TokenAccount2022};

pub const MAX_REFERRALS_PER_CODE: u16 = 50; // Maximum users per referral code
pub const REFERRAL_BONUS_PCT: u64 = 1; // 1% bonus to user with referral code
pub const REFERRAL_REWARD_PCT: u64 = 3; // 3% reward to referrer
                                        // --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
                                        // ---- STAKE DOGEBTC TOKENS :: User gets hashpower and SOL rewards ------
                                        // --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

fn require_tax_round_inactive(tax_config: &TaxConfig, action: &str) -> Result<()> {
    if tax_config.round_active {
        msg!(
            "⏸️ [{}] Tax round is active; hashpower-changing staking action is temporarily paused",
            action
        );
        return err!(ErrorCode::TaxRoundActive);
    }

    Ok(())
}

/// Stake MineBtc tokens
/// Users stake MineBtc tokens to their home faction and earn SOL and minebtc rewards
/// SOL rewards are distributed per round via join_round function
/// minebtc rewards are distributed per round via end_round function
pub fn int_stake_minebtc(
    ctx: Context<StakeMineBtc>,
    amount: u64,
    lockup_duration: u64,
    position_index: u8,
) -> Result<()> {
    msg!(
        "🔒 [stake_minebtc] Starting MineBtc staking - Amount: {}, Lockup: {} days, Position: {}",
        amount,
        lockup_duration,
        position_index
    );

    let clock = Clock::get()?;
    let current_ts = clock.unix_timestamp;

    // Store values before mutable borrow (for event emission)
    let player_data_key = ctx.accounts.player_data.key();

    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let user_position = &mut ctx.accounts.user_position;

    let hashpower_config = &ctx.accounts.hashpower_config;
    require_tax_round_inactive(&ctx.accounts.tax_config, "stake_minebtc")?;

    msg!(
        "🧭 [stake_minebtc] owner={} player={} faction_state={} faction_id={} current_position_count={}",
        ctx.accounts.authority.key(),
        player_data_key,
        faction_state.key(),
        faction_state.faction_id,
        player_data.dogebtc_position_indices.len()
    );
    msg!(
        "🧾 [stake_minebtc] player_before dogebtc_staked={} dogebtc_hashpower={} doge_multiplier={} pending_sol={} pending_minebtc={}",
        player_data.dogebtc_staked as f64 / 1e6,
        player_data.dogebtc_hashpower as f64 / 1e6,
        player_data.doge_multiplier as f64 / 1000.0,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_minebtc_rewards as f64 / 1e6
    );
    msg!(
        "🧾 [stake_minebtc] faction_before dogebtc_staked={} total_dogebtc_hashpower={} sol_index={} minebtc_index={}",
        faction_state.dogebtc_staked as f64 / 1e6,
        faction_state.total_dogebtc_hashpower as f64 / 1e6,
        faction_state.dogebtc_sol_reward_index,
        faction_state.dogebtc_dogebtc_reward_index
    );

    // Validate inputs
    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(
        lockup_duration >= hashpower_config.min_lockup_days
            && lockup_duration <= hashpower_config.max_lockup_days,
        ErrorCode::InvalidParameters
    );
    require!(
        faction_state.faction_id == player_data.faction_id,
        ErrorCode::InvalidFactionId
    );

    // Cannot add more dogeBTC to existing position
    require!(
        user_position.staked_amount == 0,
        ErrorCode::PositionAlreadyExists
    );

    // Credit the position with the exact post-fee amount implied by the live
    // Token-2022 mint config so staking stays correct if the mint fee changes.
    let transfer_fee_info = helper::get_token2022_transfer_fee_info(
        &ctx.accounts.minebtc_mint.to_account_info(),
        amount,
        clock.epoch,
    )?;
    let actual_amount = transfer_fee_info.post_fee_amount;
    require!(actual_amount > 0, ErrorCode::InvalidAmount);
    msg!(
        "🔥 mDoge transfer fee: {} bps (max {}) - Amount: {}, Fee: {}, Actual amount: {}",
        transfer_fee_info.transfer_fee_basis_points,
        transfer_fee_info.max_fee as f64 / 1e6,
        amount as f64 / 1e6,
        transfer_fee_info.fee_amount as f64 / 1e6,
        actual_amount as f64 / 1e6
    );
    msg!(
        "📊 Current faction state - Total staked: {}, Total hashpower: {}",
        faction_state.dogebtc_staked as f64 / 1e6,
        faction_state.total_dogebtc_hashpower as f64 / 1e6
    );

    // Add position index to player data
    helper::add_dogebtc_position(player_data, position_index)?;
    msg!(
        "🔍 [stake_minebtc] Position index added: {}",
        position_index
    );
    msg!(
        "🔍 [stake_minebtc] Player data - Position indices: {:?}",
        player_data.dogebtc_position_indices
    );
    msg!(
        "🔍 [stake_minebtc] Player data - Total positions: {}",
        player_data.dogebtc_position_indices.len()
    );

    // Calculate multiplier based on lockup duration
    let multiplier = helper::calculate_multiplier(
        lockup_duration,
        hashpower_config.min_lockup_days,
        hashpower_config.max_lockup_days,
        hashpower_config.base_multiplier,
        hashpower_config.max_multiplier,
    )?;
    msg!(
        "🔢 Multiplier for {} days lockup: {} ({}x)",
        lockup_duration,
        multiplier,
        multiplier as f64 / 100.0
    );

    // Calculate weighted amount for this position
    let weighted_amount = (actual_amount * multiplier as u64) / M_HUNDRED;
    msg!(
        "⚖️ Weighted amount: {} (actual amount: {} × multiplier: {}%)",
        weighted_amount as f64 / 1e6,
        actual_amount as f64 / 1e6,
        multiplier as f64 / 100.0
    );

    // -------------- ACCRUE PENDING REWARDS -------------- //

    // Process pending rewards before updating position
    let (new_sol_rewards, new_minebtc_rewards, accrued_minebtc_rewards) =
        int_update_minebtc_staking_rewards(
            player_data,
            &mut ctx.accounts.unrefined_rewards,
            faction_state,
        )?;
    msg!(
        "💹 [stake_minebtc] accrued_before_stake new_sol={} new_minebtc={} accrued_unrefined={} pending_sol={} pending_minebtc={}",
        new_sol_rewards as f64 / 1e9,
        new_minebtc_rewards as f64 / 1e6,
        accrued_minebtc_rewards as f64 / 1e6,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_minebtc_rewards as f64 / 1e6
    );

    // -------------- UPDATE POSITION -------------- //

    msg!("🆕 Creating new position {}", position_index);
    // Initialize new position
    helper::init_position(
        user_position,
        0, // position_type
        faction_state.faction_id,
        position_index,
        actual_amount,
        weighted_amount,
        lockup_duration,
        current_ts,
        multiplier,
    )?;

    // -------------- UPDATE PLAYER AND FACTION DATA -------------- //

    let doges_multiplier = player_data.doge_multiplier as u64;
    let weighted_amount_with_doges = (weighted_amount * doges_multiplier) / BASE_MULTIPLIER as u64;
    let prev_player_dogebtc_hashpower = player_data.dogebtc_hashpower;
    let prev_player_dogebtc_staked = player_data.dogebtc_staked;
    let prev_faction_dogebtc_staked = faction_state.dogebtc_staked;
    let prev_faction_dogebtc_hashpower = faction_state.total_dogebtc_hashpower;
    msg!(
        "⚙️ [stake_minebtc] position_math actual_amount={} weighted_amount={} doge_multiplier={}x hashpower_contribution={}",
        actual_amount as f64 / 1e6,
        weighted_amount as f64 / 1e6,
        doges_multiplier as f64 / 1000.0,
        weighted_amount_with_doges as f64 / 1e6
    );

    // Update player data state
    player_data.dogebtc_hashpower = player_data
        .dogebtc_hashpower
        .checked_add(weighted_amount_with_doges)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    player_data.dogebtc_staked = player_data
        .dogebtc_staked
        .checked_add(actual_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // Update faction state with actual_amount (post-tax) and weighted_amount
    faction_state.dogebtc_staked = faction_state
        .dogebtc_staked
        .checked_add(actual_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    faction_state.total_dogebtc_hashpower = faction_state
        .total_dogebtc_hashpower
        .checked_add(weighted_amount_with_doges)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   Updated faction state - Total staked: {}, Total hashpower: {}",
        faction_state.dogebtc_staked as f64 / 1e6,
        faction_state.total_dogebtc_hashpower as f64 / 1e6
    );
    msg!(
        "📈 [stake_minebtc] player_after dogebtc_staked={} -> {} dogebtc_hashpower={} -> {}",
        prev_player_dogebtc_staked as f64 / 1e6,
        player_data.dogebtc_staked as f64 / 1e6,
        prev_player_dogebtc_hashpower as f64 / 1e6,
        player_data.dogebtc_hashpower as f64 / 1e6
    );
    msg!(
        "📈 [stake_minebtc] faction_after dogebtc_staked={} -> {} total_dogebtc_hashpower={} -> {}",
        prev_faction_dogebtc_staked as f64 / 1e6,
        faction_state.dogebtc_staked as f64 / 1e6,
        prev_faction_dogebtc_hashpower as f64 / 1e6,
        faction_state.total_dogebtc_hashpower as f64 / 1e6
    );

    // -------------- TRANSFER TOKENS -------------- //

    // Transfer tokens from user to custodian
    msg!(
        "💱 Transferring {} mDoge tokens from user to custodian",
        actual_amount as f64 / 1e6
    );
    let transfer_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        token_interface::TransferChecked {
            from: ctx.accounts.user_minebtc_account.to_account_info(),
            to: ctx.accounts.minebtc_custodian.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
            mint: ctx.accounts.minebtc_mint.to_account_info(),
        },
    );
    token_interface::transfer_checked(transfer_ctx, amount, ctx.accounts.minebtc_mint.decimals)?;
    msg!("✅ [stake_minebtc] MineBtc staking successful");

    emit!(MineBtcStaked {
        owner: ctx.accounts.authority.key(),
        player_data: player_data_key,
        faction_id: faction_state.faction_id,
        position_index,
        position_key: ctx.accounts.user_position.key(),
        staked_amount: actual_amount,
        weighted_amount,
        multiplier,
        lockup_duration,
        hashpower_contribution: weighted_amount_with_doges,
        new_sol_rewards,
        new_minebtc_rewards,
        unrefined_minebtc: accrued_minebtc_rewards,
        timestamp: current_ts,
    });

    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- UNSTAKE DOGEBTC TOKENS :: User gets MINE_BTC back ------------------------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Unstake MineBtc tokens from a position
pub fn int_unstake_minebtc(ctx: Context<UnstakeMineBtc>, position_index: u8) -> Result<()> {
    // Store values before mutable borrow (for event emission)
    let position_key = ctx.accounts.user_position.key();
    let player_data_key = ctx.accounts.player_data.key();

    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let user_position = &mut ctx.accounts.user_position;
    let current_ts = Clock::get()?.unix_timestamp;
    require_tax_round_inactive(&ctx.accounts.tax_config, "unstake_minebtc")?;

    msg!(
        "🔓 [unstake_lp_tokens] Processing unstake for position {}",
        position_index
    );
    msg!(
        "🧭 [unstake_minebtc] owner={} player={} faction_state={} faction_id={} doge_multiplier={}",
        ctx.accounts.authority.key(),
        player_data_key,
        faction_state.key(),
        faction_state.faction_id,
        player_data.doge_multiplier as f64 / 1000.0
    );
    msg!(
        "🧾 [unstake_minebtc] player_before dogebtc_staked={} dogebtc_hashpower={} pending_sol={} pending_minebtc={}",
        player_data.dogebtc_staked as f64 / 1e6,
        player_data.dogebtc_hashpower as f64 / 1e6,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_minebtc_rewards as f64 / 1e6
    );
    msg!(
        "🧾 [unstake_minebtc] faction_before dogebtc_staked={} total_dogebtc_hashpower={} sol_index={} minebtc_index={}",
        faction_state.dogebtc_staked as f64 / 1e6,
        faction_state.total_dogebtc_hashpower as f64 / 1e6,
        faction_state.dogebtc_sol_reward_index,
        faction_state.dogebtc_dogebtc_reward_index
    );

    // Validate the position exists and has funds
    require!(
        faction_state.faction_id == user_position.faction_id,
        ErrorCode::InvalidFactionId
    );
    require!(
        player_data.faction_id == faction_state.faction_id,
        ErrorCode::InvalidFactionId
    );
    require!(user_position.staked_amount > 0, ErrorCode::InvalidAmount);
    require!(
        user_position.position_index == position_index,
        ErrorCode::InvalidParameters
    );
    require!(
        player_data
            .dogebtc_position_indices
            .contains(&position_index),
        ErrorCode::InvalidParameters
    );
    require!(
        position_index == user_position.position_index,
        ErrorCode::Unauthorized
    );

    msg!(
        "📊 Position details - Staked: {}, Weighted: {}, Faction: {}, Lockup ends: {}",
        user_position.staked_amount as f64 / 1e6,
        user_position.weighted_amount as f64 / 1e6,
        user_position.faction_id,
        user_position.lockup_end_timestamp
    );

    // -------------- ACCRUE PENDING REWARDS -------------- //

    // Process pending rewards before updating position
    let (new_sol_rewards, new_minebtc_rewards, accrued_minebtc_rewards) =
        int_update_minebtc_staking_rewards(
            player_data,
            &mut ctx.accounts.unrefined_rewards,
            faction_state,
        )?;
    msg!(
        "💹 [unstake_minebtc] accrued_before_unstake new_sol={} new_minebtc={} accrued_unrefined={} pending_sol={} pending_minebtc={}",
        new_sol_rewards as f64 / 1e9,
        new_minebtc_rewards as f64 / 1e6,
        accrued_minebtc_rewards as f64 / 1e6,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_minebtc_rewards as f64 / 1e6
    );

    // -------------- UPDATE FACTION AND PLAYER DATA -------------- //

    // Calculate return amount based on early withdrawal status
    let is_early_withdrawal = current_ts < user_position.lockup_end_timestamp;
    let staked_amount = user_position.staked_amount;
    let original_weighted = user_position.weighted_amount;
    let hashpower_contribution = ((original_weighted as u128 * player_data.doge_multiplier as u128)
        / BASE_MULTIPLIER as u128) as u64;
    let mut return_amount = staked_amount;
    let mut penalty_amount = 0u64;
    let prev_player_dogebtc_hashpower = player_data.dogebtc_hashpower;
    let prev_player_dogebtc_staked = player_data.dogebtc_staked;
    let prev_faction_dogebtc_staked = faction_state.dogebtc_staked;
    let prev_faction_dogebtc_hashpower = faction_state.total_dogebtc_hashpower;
    msg!(
        "⚙️ [unstake_minebtc] position_math staked_amount={} weighted_amount={} doge_multiplier={}x hashpower_contribution={} is_early={}",
        staked_amount as f64 / 1e6,
        original_weighted as f64 / 1e6,
        player_data.doge_multiplier as f64 / 1000.0,
        hashpower_contribution as f64 / 1e6,
        is_early_withdrawal
    );

    // Update faction state (decrease staked amount and hashpower)
    msg!("📊 Updating faction state");
    faction_state.dogebtc_staked -= staked_amount;
    faction_state.total_dogebtc_hashpower -= hashpower_contribution;
    msg!(
        "   New faction totals - Staked: {}, Hashpower: {}",
        faction_state.dogebtc_staked as f64 / 1e6,
        faction_state.total_dogebtc_hashpower as f64 / 1e6
    );

    // Update player data (decrease hashpower and staked amount)
    msg!("📊 Updating player data");
    player_data.dogebtc_hashpower -= hashpower_contribution;
    player_data.dogebtc_staked -= staked_amount;
    msg!(
        "   New player totals - Hashpower: {}, Staked: {}",
        player_data.dogebtc_hashpower as f64 / 1e6,
        player_data.dogebtc_staked as f64 / 1e6
    );
    msg!(
        "📈 [unstake_minebtc] player_after dogebtc_staked={} -> {} dogebtc_hashpower={} -> {}",
        prev_player_dogebtc_staked as f64 / 1e6,
        player_data.dogebtc_staked as f64 / 1e6,
        prev_player_dogebtc_hashpower as f64 / 1e6,
        player_data.dogebtc_hashpower as f64 / 1e6
    );
    msg!(
        "📈 [unstake_minebtc] faction_after dogebtc_staked={} -> {} total_dogebtc_hashpower={} -> {}",
        prev_faction_dogebtc_staked as f64 / 1e6,
        faction_state.dogebtc_staked as f64 / 1e6,
        prev_faction_dogebtc_hashpower as f64 / 1e6,
        faction_state.total_dogebtc_hashpower as f64 / 1e6
    );

    // Remove position from user's active positions
    helper::remove_dogebtc_position(player_data, position_index)?;

    // -------------- CHARGE EMERGENCY TAX -------------- //

    // Handle early withdrawal if needed
    if is_early_withdrawal {
        msg!(
            "⚠️ Early unstake detected! Current time: {}, Lockup end: {}",
            current_ts,
            user_position.lockup_end_timestamp
        );

        // Calculate remaining lockup percentage
        penalty_amount = helper::calculate_emergency_tax(
            user_position,
            current_ts,
            EMERGENCY_WITHDRAWAL_PENALTY_PCT as u64,
        );
        return_amount = staked_amount - penalty_amount;
        msg!(
            "   Total Staked: {}, Returned: {}, Penalty: {}",
            staked_amount,
            return_amount,
            penalty_amount
        );

        // Charge emergency tax if any penalty
        if penalty_amount > 0 {
            // Charge emergency tax: 50% to burn
            helper::charge_emergency_tax(
                &ctx.accounts.minebtc_custodian.to_account_info(),
                &ctx.accounts.minebtc_custodian_authority.to_account_info(),
                &ctx.accounts.minebtc_mint.to_account_info(),
                &ctx.accounts.token_program.to_account_info(),
                ctx.bumps.minebtc_custodian_authority,
                penalty_amount,
            )?;
        }
    } else {
        msg!("✅ Normal unstake - lockup period has ended");
    }

    // -------------- TRANSFER TOKENS -------------- //

    // Transfer remaining tokens back to user
    if return_amount > 0 {
        // Get and print custodian balance before transfer
        let custodian_balance = ctx.accounts.minebtc_custodian.amount;
        msg!(
            "💱 Custodian balance before transfer: {} MINE_BTC tokens",
            custodian_balance
        );
        msg!("💱 Transferring {} MINE_BTC tokens to user", return_amount);

        // Get PDA signer seeds for the minebtc_custodian authority (global, no faction_id)
        let custodian_authority_seeds = &[
            MINEBTC_CUSTODIAN_AUTHORITY_SEED.as_ref(),
            &[ctx.bumps.minebtc_custodian_authority],
        ];
        let signer = &[&custodian_authority_seeds[..]];

        // Transfer tokens back to user
        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token_interface::TransferChecked {
                from: ctx.accounts.minebtc_custodian.to_account_info(),
                to: ctx.accounts.user_minebtc_account.to_account_info(),
                authority: ctx.accounts.minebtc_custodian_authority.to_account_info(),
                mint: ctx.accounts.minebtc_mint.to_account_info(),
            },
            signer,
        );

        token_interface::transfer_checked(
            transfer_ctx,
            return_amount,
            ctx.accounts.minebtc_mint.decimals,
        )?;
    }

    // Store values for emergency withdrawal event before closing position
    let total_lockup_seconds = user_position.lockup_end_timestamp - user_position.start_timestamp;
    let remaining_seconds = user_position.lockup_end_timestamp - current_ts;
    let remaining_seconds_pct = if total_lockup_seconds > 0 && remaining_seconds > 0 {
        ((M_HUNDRED as i64 * remaining_seconds) / total_lockup_seconds) as u64
    } else {
        0u64
    };
    let calc_penalty_pct = if is_early_withdrawal && penalty_amount > 0 {
        (EMERGENCY_WITHDRAWAL_PENALTY_PCT as u64 * remaining_seconds_pct) / M_HUNDRED
    } else {
        0
    };

    // Emit events before closing account
    emit!(MineBtcUnstaked {
        owner: ctx.accounts.authority.key(),
        player_data: player_data_key,
        position_index,
        position_key,
        new_sol_rewards,
        new_minebtc_rewards,
        unrefined_minebtc: accrued_minebtc_rewards,
        original_amount: staked_amount,
        returned_amount: return_amount,
        timestamp: current_ts,
    });

    // Emit emergency withdrawal event if early withdrawal
    if is_early_withdrawal && penalty_amount > 0 {
        emit!(EmergencyWithdrawal {
            owner: ctx.accounts.authority.key(),
            player_data: player_data_key,
            position_key,
            position_index,
            original_amount: staked_amount,
            penalty_amount,
            returned_amount: return_amount,
            penalty_tax_pct: calc_penalty_pct,
            timestamp: current_ts,
        });
    }

    // Account will be automatically closed by Anchor (close = authority in account struct)
    msg!("✅ [unstake_minebtc] Unstake completed successfully. Position account will be closed.");
    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- STAKE LIQUIDITY LP TOKENS :: User gets SOL and minebtc rewards ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Stake LP tokens
/// Users stake LP tokens to their home faction and earn SOL and minebtc rewards
/// SOL rewards are distributed per round via join_round function
/// minebtc rewards are distributed per round via end_round function
pub fn int_stake_lp_tokens(
    ctx: Context<StakeLpTokens>,
    amount: u64,
    lockup_duration: u64,
    position_index: u8,
) -> Result<()> {
    msg!(
        "🔒 [stake_lp_tokens] Starting LP token staking - Amount: {}, Lockup: {} days, Position: {}",
        amount,
        lockup_duration,
        position_index,
    );

    let current_ts = Clock::get()?.unix_timestamp;

    // Store values before mutable borrow (for event emission)
    let player_data_key = ctx.accounts.player_data.key();

    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let user_position = &mut ctx.accounts.user_position;

    let hashpower_config = &ctx.accounts.hashpower_config;
    require_tax_round_inactive(&ctx.accounts.tax_config, "stake_lp_tokens")?;
    msg!(
        "🧭 [stake_lp_tokens] owner={} player={} faction_state={} faction_id={} current_position_count={}",
        ctx.accounts.authority.key(),
        player_data_key,
        faction_state.key(),
        faction_state.faction_id,
        player_data.lp_position_indices.len()
    );
    msg!(
        "🧾 [stake_lp_tokens] player_before lp_staked={} lp_hashpower={} doge_multiplier={} pending_sol={} pending_minebtc={}",
        player_data.lp_staked as f64 / 1e6,
        player_data.lp_hashpower as f64 / 1e6,
        player_data.doge_multiplier as f64 / 1000.0,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_minebtc_rewards as f64 / 1e6
    );
    msg!(
        "🧾 [stake_lp_tokens] faction_before lp_staked={} total_lp_hashpower={} sol_index={} minebtc_index={}",
        faction_state.lp_staked as f64 / 1e6,
        faction_state.total_lp_hashpower as f64 / 1e6,
        faction_state.lp_sol_reward_index,
        faction_state.lp_dogebtc_reward_index
    );

    // Validate inputs
    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(
        lockup_duration >= hashpower_config.min_lockup_days
            && lockup_duration <= hashpower_config.max_lockup_days,
        ErrorCode::InvalidParameters
    );
    require!(
        faction_state.faction_id == player_data.faction_id,
        ErrorCode::InvalidFactionId
    );

    // Cannot add more LP tokens to existing position
    require!(
        user_position.staked_amount == 0,
        ErrorCode::PositionAlreadyExists
    );

    // Calculate actual amount after burn tax
    let actual_amount = amount;
    msg!(
        " Current faction state - Total LP staked: {}, Total LP hashpower: {}",
        faction_state.total_lp_hashpower,
        faction_state.total_lp_hashpower
    );

    // Add position index to player data
    helper::add_lp_position(player_data, position_index)?;

    // Calculate multiplier based on lockup duration
    let multiplier = helper::calculate_multiplier(
        lockup_duration,
        hashpower_config.min_lockup_days,
        hashpower_config.max_lockup_days,
        hashpower_config.base_multiplier,
        hashpower_config.max_multiplier,
    )?;
    msg!(
        "🔢 Multiplier for {} days lockup: {} ({}x)",
        lockup_duration,
        multiplier,
        multiplier as f64 / 100.0
    );

    // Calculate weighted amount for this position
    let weighted_amount = actual_amount * multiplier as u64 / M_HUNDRED;
    msg!(
        "⚖️ Weighted amount: {} (amount: {} × multiplier: {}%)",
        weighted_amount as f64 / 1e6,
        actual_amount as f64 / 1e6,
        multiplier as f64 / 100.0
    );

    // -------------- ACCRUE PENDING REWARDS -------------- //

    // Process pending rewards before updating position
    let (new_sol_rewards, new_minebtc_rewards, accrued_minebtc_rewards) =
        int_update_lp_staking_rewards(
            player_data,
            &mut ctx.accounts.unrefined_rewards,
            faction_state,
        )?;
    msg!(
        "💹 [stake_lp_tokens] accrued_before_stake new_sol={} new_minebtc={} accrued_unrefined={} pending_sol={} pending_minebtc={}",
        new_sol_rewards as f64 / 1e9,
        new_minebtc_rewards as f64 / 1e6,
        accrued_minebtc_rewards as f64 / 1e6,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_minebtc_rewards as f64 / 1e6
    );

    // -------------- UPDATE POSITION -------------- //

    msg!("🆕 Creating new position {}", position_index);
    // Initialize new position
    helper::init_position(
        user_position,
        1, // position_type
        faction_state.faction_id,
        position_index,
        actual_amount,
        weighted_amount,
        lockup_duration,
        current_ts,
        multiplier,
    )?;

    // -------------- UPDATE PLAYER AND FACTION DATA -------------- //

    let doges_multiplier = player_data.doge_multiplier as u64;
    let weighted_amount_with_doges = (weighted_amount * doges_multiplier) / BASE_MULTIPLIER as u64;
    let prev_player_lp_hashpower = player_data.lp_hashpower;
    let prev_player_lp_staked = player_data.lp_staked;
    let prev_faction_lp_staked = faction_state.lp_staked;
    let prev_faction_lp_hashpower = faction_state.total_lp_hashpower;
    msg!(
        "⚙️ [stake_lp_tokens] position_math actual_amount={} weighted_amount={} doge_multiplier={}x hashpower_contribution={}",
        actual_amount as f64 / 1e6,
        weighted_amount as f64 / 1e6,
        doges_multiplier as f64 / 1000.0,
        weighted_amount_with_doges as f64 / 1e6
    );

    // Update player data state
    player_data.lp_hashpower = player_data
        .lp_hashpower
        .checked_add(weighted_amount_with_doges)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    player_data.lp_staked = player_data
        .lp_staked
        .checked_add(actual_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // Update faction state with actual_amount (post-tax) and weighted_amount
    faction_state.lp_staked = faction_state
        .lp_staked
        .checked_add(actual_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    faction_state.total_lp_hashpower = faction_state
        .total_lp_hashpower
        .checked_add(weighted_amount_with_doges)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   Updated faction state - Total staked: {}, Total hashpower: {}",
        faction_state.lp_staked as f64 / 1e6,
        faction_state.total_lp_hashpower as f64 / 1e6
    );
    msg!(
        "📈 [stake_lp_tokens] player_after lp_staked={} -> {} lp_hashpower={} -> {}",
        prev_player_lp_staked as f64 / 1e6,
        player_data.lp_staked as f64 / 1e6,
        prev_player_lp_hashpower as f64 / 1e6,
        player_data.lp_hashpower as f64 / 1e6
    );
    msg!(
        "📈 [stake_lp_tokens] faction_after lp_staked={} -> {} total_lp_hashpower={} -> {}",
        prev_faction_lp_staked as f64 / 1e6,
        faction_state.lp_staked as f64 / 1e6,
        prev_faction_lp_hashpower as f64 / 1e6,
        faction_state.total_lp_hashpower as f64 / 1e6
    );

    // -------------- TRANSFER TOKENS -------------- //

    msg!(
        "💱 Transferring {} LP tokens from user to custodian",
        actual_amount as f64 / 1e6
    );

    // Transfer tokens from user to custodian
    let transfer_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        token::Transfer {
            from: ctx.accounts.user_lp_account.to_account_info(),
            to: ctx.accounts.liquidity_custodian.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        },
    );
    token::transfer(transfer_ctx, amount)?;
    msg!("   ✓ Transferred {} LP tokens", actual_amount as f64 / 1e6);

    emit!(LiquidityStaked {
        owner: ctx.accounts.authority.key(),
        player_data: player_data_key,
        faction_id: faction_state.faction_id,
        position_index,
        position_key: ctx.accounts.user_position.key(),
        staked_amount: actual_amount,
        weighted_amount,
        multiplier,
        lockup_duration,
        hashpower_contribution: weighted_amount_with_doges,
        new_sol_rewards,
        new_minebtc_rewards,
        unrefined_minebtc: accrued_minebtc_rewards,
        timestamp: current_ts,
    });

    Ok(())
}

/// Unstake LP tokens from a position
pub fn int_unstake_lp_tokens(ctx: Context<UnstakeLpTokens>, position_index: u8) -> Result<()> {
    // Store values before mutable borrow (for event emission)
    let player_data_key = ctx.accounts.player_data.key();
    let position_key = ctx.accounts.user_position.key();

    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let user_position = &mut ctx.accounts.user_position;
    let current_ts = Clock::get()?.unix_timestamp;
    require_tax_round_inactive(&ctx.accounts.tax_config, "unstake_lp_tokens")?;

    msg!(
        "🔓 [unstake_minebtc] Processing unstake for position {}",
        position_index
    );
    msg!(
        "🧭 [unstake_lp_tokens] owner={} player={} faction_state={} faction_id={} doge_multiplier={}",
        ctx.accounts.authority.key(),
        player_data_key,
        faction_state.key(),
        faction_state.faction_id,
        player_data.doge_multiplier as f64 / 1000.0
    );
    msg!(
        "🧾 [unstake_lp_tokens] player_before lp_staked={} lp_hashpower={} pending_sol={} pending_minebtc={}",
        player_data.lp_staked as f64 / 1e6,
        player_data.lp_hashpower as f64 / 1e6,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_minebtc_rewards as f64 / 1e6
    );
    msg!(
        "🧾 [unstake_lp_tokens] faction_before lp_staked={} total_lp_hashpower={} sol_index={} minebtc_index={}",
        faction_state.lp_staked as f64 / 1e6,
        faction_state.total_lp_hashpower as f64 / 1e6,
        faction_state.lp_sol_reward_index,
        faction_state.lp_dogebtc_reward_index
    );

    // Validate the position exists and has funds
    require!(
        faction_state.faction_id == user_position.faction_id,
        ErrorCode::InvalidFactionId
    );
    require!(
        player_data.faction_id == faction_state.faction_id,
        ErrorCode::InvalidFactionId
    );
    require!(user_position.staked_amount > 0, ErrorCode::InvalidAmount);
    require!(
        user_position.position_index == position_index,
        ErrorCode::InvalidParameters
    );
    require!(
        player_data.lp_position_indices.contains(&position_index),
        ErrorCode::InvalidParameters
    );

    msg!(
        "📊 Position details - Staked: {}, Weighted: {}, Faction: {}, Lockup ends: {}",
        user_position.staked_amount as f64 / 1e6,
        user_position.weighted_amount as f64 / 1e6,
        user_position.faction_id,
        user_position.lockup_end_timestamp
    );

    // -------------- ACCRUE PENDING REWARDS -------------- //

    // Process pending rewards before updating position
    let (new_sol_rewards, new_minebtc_rewards, accrued_minebtc_rewards) =
        int_update_lp_staking_rewards(
            player_data,
            &mut ctx.accounts.unrefined_rewards,
            faction_state,
        )?;
    msg!(
        "💹 [unstake_lp_tokens] accrued_before_unstake new_sol={} new_minebtc={} accrued_unrefined={} pending_sol={} pending_minebtc={}",
        new_sol_rewards as f64 / 1e9,
        new_minebtc_rewards as f64 / 1e6,
        accrued_minebtc_rewards as f64 / 1e6,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_minebtc_rewards as f64 / 1e6
    );

    // -------------- UPDATE FACTION AND PLAYER DATA -------------- //

    // Calculate return amount based on early withdrawal status
    let is_early_withdrawal = current_ts < user_position.lockup_end_timestamp;
    let staked_amount = user_position.staked_amount;
    let original_weighted = user_position.weighted_amount;
    let hashpower_contribution = ((original_weighted as u128 * player_data.doge_multiplier as u128)
        / BASE_MULTIPLIER as u128) as u64;
    let mut return_amount = staked_amount;
    let mut penalty_amount = 0u64;
    let prev_player_lp_hashpower = player_data.lp_hashpower;
    let prev_player_lp_staked = player_data.lp_staked;
    let prev_faction_lp_staked = faction_state.lp_staked;
    let prev_faction_lp_hashpower = faction_state.total_lp_hashpower;
    msg!(
        "⚙️ [unstake_lp_tokens] position_math staked_amount={} weighted_amount={} doge_multiplier={}x hashpower_contribution={} is_early={}",
        staked_amount as f64 / 1e6,
        original_weighted as f64 / 1e6,
        player_data.doge_multiplier as f64 / 1000.0,
        hashpower_contribution as f64 / 1e6,
        is_early_withdrawal
    );

    // Update faction state (decrease staked amount and hashpower)
    msg!("📊 Updating faction state");
    faction_state.lp_staked -= staked_amount;
    faction_state.total_lp_hashpower -= hashpower_contribution;
    msg!(
        "   New faction totals - Staked: {}, Hashpower: {}",
        faction_state.lp_staked as f64 / 1e6,
        faction_state.total_lp_hashpower as f64 / 1e6
    );

    // Update player data (decrease hashpower and staked amount)
    msg!("📊 Updating player data");
    player_data.lp_hashpower -= hashpower_contribution;
    player_data.lp_staked -= staked_amount;
    msg!(
        "   New player totals - Hashpower: {}, Staked: {}",
        player_data.lp_hashpower as f64 / 1e6,
        player_data.lp_staked as f64 / 1e6
    );
    msg!(
        "📈 [unstake_lp_tokens] player_after lp_staked={} -> {} lp_hashpower={} -> {}",
        prev_player_lp_staked as f64 / 1e6,
        player_data.lp_staked as f64 / 1e6,
        prev_player_lp_hashpower as f64 / 1e6,
        player_data.lp_hashpower as f64 / 1e6
    );
    msg!(
        "📈 [unstake_lp_tokens] faction_after lp_staked={} -> {} total_lp_hashpower={} -> {}",
        prev_faction_lp_staked as f64 / 1e6,
        faction_state.lp_staked as f64 / 1e6,
        prev_faction_lp_hashpower as f64 / 1e6,
        faction_state.total_lp_hashpower as f64 / 1e6
    );

    // Remove position from user's active positions
    helper::remove_lp_position(player_data, position_index)?;

    // -------------- CHARGE EMERGENCY TAX -------------- //

    // Handle early withdrawal if needed
    if is_early_withdrawal {
        msg!(
            "⚠️ Early unstake detected! Current time: {}, Lockup end: {}",
            current_ts,
            user_position.lockup_end_timestamp
        );

        // Calculate remaining lockup percentage
        penalty_amount = helper::calculate_emergency_tax(
            user_position,
            current_ts,
            EMERGENCY_WITHDRAWAL_PENALTY_PCT as u64,
        );
        return_amount = staked_amount - penalty_amount;
        msg!(
            "   Total Staked: {}, Returned: {}, Penalty: {}",
            staked_amount,
            return_amount,
            penalty_amount
        );

        // Charge emergency tax if any penalty
        if penalty_amount > 0 {
            // Charge emergency tax: 100% to burn (no rewards to stakers)
            helper::charge_lp_emergency_tax(
                &ctx.accounts.liquidity_custodian.to_account_info(),
                &ctx.accounts.liquidity_custodian_authority.to_account_info(),
                &ctx.accounts.lp_mint.to_account_info(),
                &ctx.accounts.token_program.to_account_info(),
                ctx.bumps.liquidity_custodian_authority,
                penalty_amount,
            )?;
        }
    } else {
        msg!("✅ Normal unstake - lockup period has ended");
    }

    // -------------- TRANSFER TOKENS -------------- //

    // Transfer remaining tokens back to user
    if return_amount > 0 {
        msg!("💱 Transferring {} LP tokens to user", return_amount);

        // Get PDA signer seeds for the liquidity_custodian authority (global, no faction_id)
        let custodian_authority_seeds = &[
            LIQUIDITY_CUSTODIAN_AUTHORITY_SEED.as_ref(),
            &[ctx.bumps.liquidity_custodian_authority],
        ];
        let signer = &[&custodian_authority_seeds[..]];

        // Transfer tokens back to user
        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token_interface::TransferChecked {
                from: ctx.accounts.liquidity_custodian.to_account_info(),
                to: ctx.accounts.user_lp_account.to_account_info(),
                authority: ctx.accounts.liquidity_custodian_authority.to_account_info(),
                mint: ctx.accounts.lp_mint.to_account_info(),
            },
            signer,
        );

        token_interface::transfer_checked(
            transfer_ctx,
            return_amount,
            ctx.accounts.lp_mint.decimals,
        )?;
    }

    // Store values before emitting events and closing account
    let total_lockup_seconds = user_position.lockup_end_timestamp - user_position.start_timestamp;
    let remaining_seconds = if is_early_withdrawal {
        user_position.lockup_end_timestamp - current_ts
    } else {
        0
    };
    let remaining_seconds_pct =
        if total_lockup_seconds > 0 && is_early_withdrawal && remaining_seconds > 0 {
            ((M_HUNDRED as i64 * remaining_seconds) / total_lockup_seconds) as u64
        } else {
            0u64
        };
    let calc_penalty_pct = if is_early_withdrawal && penalty_amount > 0 {
        (EMERGENCY_WITHDRAWAL_PENALTY_PCT as u64 * remaining_seconds_pct) / M_HUNDRED
    } else {
        0
    };

    // Emit events before closing account
    emit!(LiquidityUnstaked {
        owner: ctx.accounts.authority.key(),
        player_data: player_data_key,
        position_index,
        position_key,
        new_sol_rewards,
        new_minebtc_rewards,
        unrefined_minebtc: accrued_minebtc_rewards,
        original_amount: staked_amount,
        returned_amount: return_amount,
        timestamp: current_ts,
    });

    // Emit emergency withdrawal event if early withdrawal
    if is_early_withdrawal && penalty_amount > 0 {
        emit!(EmergencyWithdrawal {
            owner: ctx.accounts.authority.key(),
            player_data: player_data_key,
            position_index,
            position_key,
            original_amount: staked_amount,
            penalty_amount,
            returned_amount: return_amount,
            penalty_tax_pct: calc_penalty_pct,
            timestamp: current_ts,
        });
    }

    // Account will be automatically closed by Anchor (close = authority in account struct)
    msg!("✅ [unstake_lp_tokens] Unstake completed successfully. Position account will be closed.");
    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- CLAIM STAKING REWARDS :: Updates indexes, transfers SOL, accumulates MineBTC to pending ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Claim staking rewards - updates all staking indexes, transfers SOL directly to owner,
/// and accumulates MineBTC to pending_minebtc_rewards (NOT transferred here)
pub fn int_claim_staking_rewards(ctx: Context<ClaimStakingRewards>) -> Result<()> {
    msg!("💰 [claim_staking_rewards] Claiming SOL rewards + accumulating MineBTC");

    // Store values before mutable borrow (for event emission)
    let player_data_key = ctx.accounts.player_data.key();
    let faction_id = ctx.accounts.faction_state.faction_id;

    let faction_state = &ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    require!(
        faction_state.faction_id == player_data.faction_id,
        ErrorCode::InvalidFactionId
    );
    msg!(
        "🧭 [claim_staking_rewards] owner={} player={} faction_state={} faction_id={}",
        ctx.accounts.authority.key(),
        player_data_key,
        faction_state.key(),
        faction_id
    );
    msg!(
        "🧾 [claim_staking_rewards] pending_before sol={} minebtc={} dogebtc_hashpower={} lp_hashpower={}",
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_minebtc_rewards as f64 / 1e6,
        player_data.dogebtc_hashpower as f64 / 1e6,
        player_data.lp_hashpower as f64 / 1e6
    );

    // Process MineBtc staking SOL rewards
    let (
        _st_minebtc_new_sol_rewards,
        _st_minebtc_new_minebtc_rewards,
        _st_minebtc_accrued_minebtc_rewards,
    ) = int_update_minebtc_staking_rewards(
        player_data,
        &mut ctx.accounts.unrefined_rewards,
        faction_state,
    )?;
    // Process LP staking SOL rewards
    let (_st_lp_new_sol_rewards, _st_lp_new_minebtc_rewards, _st_lp_accrued_minebtc_rewards) =
        int_update_lp_staking_rewards(
            player_data,
            &mut ctx.accounts.unrefined_rewards,
            faction_state,
        )?;
    msg!(
        "💹 [claim_staking_rewards] pending_after_index_sync sol={} minebtc={}",
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_minebtc_rewards as f64 / 1e6
    );

    let total_pending_sol_rewards = player_data.pending_sol_rewards;
    require!(total_pending_sol_rewards > 0, ErrorCode::InsufficientFunds);
    msg!(
        "   Total claimable SOL rewards: {} lamports",
        total_pending_sol_rewards as f64 / 1e9
    );
    msg!(
        "   Total claimable MineBtc rewards: {} minebtc",
        player_data.pending_minebtc_rewards as f64 / 1e6
    );

    // Check if user has a referrer (not system referral account)
    let player_sol = total_pending_sol_rewards;

    // Transfer SOL rewards to user (after referral fee)
    msg!(
        "   Transferring {} SOL from sol_rewards_vault to user",
        (player_sol as f64 / 1e9)
    );
    helper::transfer_from_sol_rewards_vault(
        &ctx.accounts.sol_rewards_vault.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        player_sol,
        ctx.bumps.sol_rewards_vault,
    )?;
    msg!("     ✓ SOL rewards transferred to user");

    // Reset pending rewards
    player_data.pending_sol_rewards = 0;

    emit!(SolRewardsClaimed {
        user: ctx.accounts.authority.key(),
        player_data: player_data_key,
        faction_id,
        sol_amount: player_sol,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "✅ [claim_staking_rewards] Claimed {} SOL",
        player_sol as f64 / 1e9,
    );
    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- WITHDRAW DBTC REWARDS :: User withdraws accumulated MineBTC with fees ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Withdraw accumulated MineBtc token rewards
/// Implements refining fee: 10% of claimed rewards are redistributed to other unclaimed stakers
/// NOTE: Call claim_staking_rewards first to update staking indexes and accumulate rewards
pub fn int_withdraw_dbtc_rewards(ctx: Context<WithdrawDbtcRewards>) -> Result<()> {
    msg!("💰 [withdraw_dbtc_rewards] Withdrawing MineBtc with refining fee");

    // Store values before mutable borrow (for event emission)
    let player_data_key = ctx.accounts.player_data.key();
    let faction_id = ctx.accounts.player_data.faction_id;
    let player_owner = ctx.accounts.player_data.owner;

    let player_data = &mut ctx.accounts.player_data;
    let unrefined_minebtc = &mut ctx.accounts.unrefined_rewards;
    let global_config = &ctx.accounts.global_config;
    msg!(
        "🧭 [withdraw_dbtc_rewards] owner={} player={} faction_id={} pending_minebtc={} total_claimable={} unrefining_index={}",
        player_owner,
        player_data_key,
        faction_id,
        player_data.pending_minebtc_rewards as f64 / 1e6,
        unrefined_minebtc.total_minebtc_claimable as f64 / 1e6,
        unrefined_minebtc.unrefining_index
    );

    require!(
        player_data.pending_minebtc_rewards > 0,
        ErrorCode::InsufficientFunds
    );

    // Apply refining fee (10% by default, or configured in global_config)
    let refining_fee_pct = global_config.minebtc_dist_config.refining_fee as u64;
    let refining_fee = (player_data.pending_minebtc_rewards * refining_fee_pct) / M_HUNDRED;
    let base_claimable_amount = player_data.pending_minebtc_rewards - refining_fee;
    msg!(
        "Refining fee: {} minebtc. Base claimable amount: {} minebtc",
        refining_fee as f64 / 1e6,
        base_claimable_amount as f64 / 1e6
    );

    // Apply referral logic: user gets +1% bonus, referrer gets 3%.
    // A real referral must resolve to the referrer's canonical ReferralRewards PDA.
    let has_referrer = player_data.referral_code != ctx.accounts.system_program.key();
    let (referral_bonus, referral_reward) = if has_referrer {
        helper::validate_referrer_rewards_account(
            &player_data.referral_code,
            ctx.accounts.referrer_rewards.as_ref(),
        )?;

        // User gets +1% bonus on their rewards
        let bonus = (base_claimable_amount * REFERRAL_BONUS_PCT) / 100;
        // Referrer gets 3% of user's base claimable amount
        let reward = (base_claimable_amount * REFERRAL_REWARD_PCT) / 100;
        msg!("   Referral bonus (+1%): {} minebtc", bonus as f64 / 1e6);
        msg!(
            "   Referral reward to referrer (3%): {} minebtc",
            reward as f64 / 1e6
        );

        // Add reward to referrer's pending minebtc rewards
        let referrer_rewards = ctx
            .accounts
            .referrer_rewards
            .as_mut()
            .ok_or(ErrorCode::ReferralRewardsAccountRequired)?;
        referrer_rewards.pending_minebtc_rewards = referrer_rewards
            .pending_minebtc_rewards
            .checked_add(reward)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        referrer_rewards.total_minebtc_earned = referrer_rewards
            .total_minebtc_earned
            .checked_add(reward)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "     Added {} minebtc to referrer's rewards",
            reward as f64 / 1e6
        );
        (bonus, reward)
    } else {
        (0, 0)
    };

    // User gets base amount + referral bonus
    let claimable_by_user = base_claimable_amount
        .checked_add(referral_bonus)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "Claimable by user: {} minebtc",
        claimable_by_user as f64 / 1e6
    );

    // Transfer claimable MineBtc to user
    if claimable_by_user > 0 {
        msg!(
            "💱 Transferring {} MineBtc tokens to user",
            claimable_by_user as f64 / 1e6
        );

        // Get PDA signer seeds for the minebtc vault authority
        let vault_authority_seeds = &[
            MINE_BTC_VAULT_AUTHORITY_SEED.as_ref(),
            &[ctx.bumps.minebtc_vault_authority],
        ];
        let signer = &[&vault_authority_seeds[..]];

        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token_interface::TransferChecked {
                from: ctx.accounts.minebtc_token_vault.to_account_info(),
                to: ctx.accounts.user_minebtc_account.to_account_info(),
                authority: ctx.accounts.minebtc_vault_authority.to_account_info(),
                mint: ctx.accounts.minebtc_mint.to_account_info(),
            },
            signer,
        );
        token_interface::transfer_checked(
            transfer_ctx,
            claimable_by_user,
            ctx.accounts.minebtc_mint.decimals,
        )?;
    }

    // Update total claimable minebtc amount
    // Only deduct the user's base pending rewards (what was actually tracked in total_minebtc_claimable).
    // Referral bonus + reward are paid from the emissions vault directly and were never
    // added to total_minebtc_claimable, so subtracting them would cause accounting drift
    // and inflate the refining fee index for remaining stakers.
    let base_pending = player_data.pending_minebtc_rewards;
    unrefined_minebtc.total_minebtc_claimable = unrefined_minebtc
        .total_minebtc_claimable
        .saturating_sub(base_pending);
    player_data.pending_minebtc_rewards = 0;
    msg!(
        "   Deducted {} minebtc from total claimable (referral bonus: {}, referrer reward: {} paid from emissions vault)",
        base_pending as f64 / 1e6,
        referral_bonus as f64 / 1e6,
        referral_reward as f64 / 1e6
    );

    // Update total tokens distributed
    let mine_btc_mining = &mut ctx.accounts.mine_btc_mining;
    mine_btc_mining.total_tokens_distributed = mine_btc_mining
        .total_tokens_distributed
        .checked_add(claimable_by_user)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   Updated total tokens distributed: {} (+{})",
        mine_btc_mining.total_tokens_distributed as f64 / 1e6,
        claimable_by_user as f64 / 1e6
    );

    // Store referrer before emitting event
    let referrer_pubkey = if has_referrer {
        Some(player_data.referral_code)
    } else {
        None
    };

    // Redistribute refining fee to all other stakers who haven't claimed
    // This is done by increasing the reward index, which benefits all stakers proportionally
    if refining_fee > 0 {
        msg!("   Redistributing refining fee to other stakers...");
        if unrefined_minebtc.total_minebtc_claimable > 0 {
            let increment = helper::mul_div(
                refining_fee,
                INDEX_PRECISION,
                unrefined_minebtc.total_minebtc_claimable,
            )?;
            unrefined_minebtc.unrefining_index = unrefined_minebtc
                .unrefining_index
                .checked_add(increment)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!(
                "   Updated unrefining index: {} (+{})",
                unrefined_minebtc.unrefining_index,
                increment
            );
        } else {
            msg!("   No other stakers to redistribute to. Fee remains in unrefined rewards pool.");
        }
    }

    emit!(DbtcRewardsClaimed {
        user: player_owner,
        player_data: player_data_key,
        faction_id,
        minebtc_amount: claimable_by_user,
        refining_fee,
        referral_bonus,
        referral_reward,
        referrer: referrer_pubkey,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "✅ [withdraw_dbtc_rewards] Withdrew {} MineBTC to {} (bonus: {}, referrer reward: {})",
        claimable_by_user as f64 / 1e6,
        player_owner,
        referral_bonus as f64 / 1e6,
        referral_reward as f64 / 1e6
    );

    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- CLAIM REFERRAL REWARDS :: Referrers claim their earned rewards ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Claim referral rewards (SOL and MineBtc)
pub fn int_claim_referral_rewards(ctx: Context<ClaimReferralRewards>) -> Result<()> {
    msg!("💰 [claim_referral_rewards] Claiming referral rewards");

    let referral_rewards = &mut ctx.accounts.referral_rewards;

    let pending_minebtc = referral_rewards.pending_minebtc_rewards;
    let pending_sol = referral_rewards.pending_sol_rewards;

    require!(
        pending_minebtc > 0 || pending_sol > 0,
        ErrorCode::InsufficientFunds
    );

    msg!(
        "     Pending MineBtc: {} minebtc",
        pending_minebtc as f64 / 1e6
    );
    let mut claimed_sol = 0u64;

    // Transfer MineBtc if any
    if pending_minebtc > 0 {
        let vault_authority_seeds = &[
            MINE_BTC_VAULT_AUTHORITY_SEED.as_ref(),
            &[ctx.bumps.minebtc_vault_authority],
        ];
        let signer = &[&vault_authority_seeds[..]];

        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token_interface::TransferChecked {
                from: ctx.accounts.minebtc_token_vault.to_account_info(),
                to: ctx.accounts.user_minebtc_account.to_account_info(),
                authority: ctx.accounts.minebtc_vault_authority.to_account_info(),
                mint: ctx.accounts.minebtc_mint.to_account_info(),
            },
            signer,
        );

        token_interface::transfer_checked(
            transfer_ctx,
            pending_minebtc,
            ctx.accounts.minebtc_mint.decimals,
        )?;
        msg!("   ✓ Transferred {} minebtc", pending_minebtc);
    }

    // Update total tokens distributed
    let mine_btc_mining = &mut ctx.accounts.mine_btc_mining;
    mine_btc_mining.total_tokens_distributed = mine_btc_mining
        .total_tokens_distributed
        .checked_add(pending_minebtc)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   Updated total tokens distributed: {} (+{})",
        mine_btc_mining.total_tokens_distributed as f64 / 1e6,
        pending_minebtc as f64 / 1e6
    );

    // Reset pending minebtc rewards
    referral_rewards.pending_minebtc_rewards = 0;

    // Transfer pending SOL from ReferralRewards PDA to user
    // SOL is stored as extra lamports on the PDA account
    if pending_sol > 0 {
        // Ensure we don't withdraw below rent-exempt minimum
        let rent_exempt_min = Rent::get()?.minimum_balance(ReferralRewards::LEN);
        let referral_info = referral_rewards.to_account_info();
        let current_lamports = referral_info.lamports();
        let withdrawable = current_lamports.saturating_sub(rent_exempt_min);

        // Only transfer what's actually available (capped at withdrawable)
        let transfer_amount = pending_sol.min(withdrawable);
        require!(transfer_amount > 0, ErrorCode::InsufficientFunds);

        // Transfer lamports from ReferralRewards PDA to user
        // This is safe because the PDA is owned by our program
        let user_info = ctx.accounts.authority.to_account_info();
        **referral_info.try_borrow_mut_lamports()? -= transfer_amount;
        **user_info.try_borrow_mut_lamports()? += transfer_amount;

        // Update pending to reflect any remainder (if withdrawable < pending_sol)
        referral_rewards.pending_sol_rewards = pending_sol - transfer_amount;
        claimed_sol = transfer_amount;
        msg!(
            "   ✓ Transferred {} SOL referral rewards from PDA",
            transfer_amount as f64 / 1e9
        );
        if transfer_amount < pending_sol {
            msg!(
                "   ⚠️ Partial claim: {} SOL still pending (rent-exempt reserve)",
                (pending_sol - transfer_amount) as f64 / 1e9
            );
        }
    }

    emit!(ReferralRewardsClaimed {
        referrer: ctx.accounts.authority.key(),
        referral_rewards_account: ctx.accounts.referral_rewards.key(),
        minebtc_amount: pending_minebtc,
        sol_amount: claimed_sol,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!("✅ [claim_referral_rewards] Claimed referral rewards successfully");
    Ok(())
}

// ----------------------------------------------------------------------------------------
// -------------- HELPER FUNCTIONS ---------------------------------------------------------
// ----------------------------------------------------------------------------------------

pub fn int_update_minebtc_staking_rewards(
    player_data: &mut PlayerData,
    unrefined_rewards: &mut UnrefinedRewards,
    faction_state: &FactionState,
) -> Result<(u64, u64, u64)> {
    msg!("💰 Processing pending rewards before position update");
    let mut new_minebtc_rewards = 0;
    let mut new_sol_rewards = 0;
    let mut accrued_minebtc_rewards = 0;
    msg!(
        "📚 [update_minebtc_rewards] hashpower={} sol_index={} sol_debt={} minebtc_index={} minebtc_debt={} pending_sol_before={} pending_minebtc_before={}",
        player_data.dogebtc_hashpower as f64 / 1e6,
        faction_state.dogebtc_sol_reward_index,
        player_data.dogebtc_sol_reward_debt,
        faction_state.dogebtc_dogebtc_reward_index,
        player_data.dogebtc_dogebtc_reward_debt,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_minebtc_rewards as f64 / 1e6
    );

    if player_data.dogebtc_hashpower > 0 {
        // Calculate SOL rewards using helper function (convert u128 indexes to u64 for calculation)
        new_sol_rewards = helper::calculate_staking_rewards(
            player_data.dogebtc_hashpower,
            faction_state.dogebtc_sol_reward_index,
            player_data.dogebtc_sol_reward_debt,
        )?;
        player_data.pending_sol_rewards = player_data
            .pending_sol_rewards
            .checked_add(new_sol_rewards)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "   Updated pending SOL rewards: {} (+{})",
            player_data.pending_sol_rewards as f64 / 1e9,
            new_sol_rewards as f64 / 1e9
        );

        new_minebtc_rewards = helper::calculate_staking_rewards(
            player_data.dogebtc_hashpower,
            faction_state.dogebtc_dogebtc_reward_index,
            player_data.dogebtc_dogebtc_reward_debt,
        )?;
        accrued_minebtc_rewards =
            helper::add_to_total_claimable(unrefined_rewards, player_data, new_minebtc_rewards)?;
        msg!(
            "   Updated pending MineBtc rewards: {} (+{})",
            player_data.pending_minebtc_rewards as f64 / 1e6,
            new_minebtc_rewards as f64 / 1e6
        );
    } else {
        msg!("ℹ️ [update_minebtc_rewards] no dogebtc hashpower; only syncing reward debt");
    }

    // Update reward debt to current indexes
    player_data.dogebtc_sol_reward_debt = faction_state.dogebtc_sol_reward_index;
    player_data.dogebtc_dogebtc_reward_debt = faction_state.dogebtc_dogebtc_reward_index;
    msg!(
        "📚 [update_minebtc_rewards] debt_after_sync sol_debt={} minebtc_debt={} accrued_unrefined={} total_claimable={}",
        player_data.dogebtc_sol_reward_debt,
        player_data.dogebtc_dogebtc_reward_debt,
        accrued_minebtc_rewards as f64 / 1e6,
        unrefined_rewards.total_minebtc_claimable as f64 / 1e6
    );

    Ok((
        new_sol_rewards,
        new_minebtc_rewards,
        accrued_minebtc_rewards,
    ))
}

pub fn int_update_lp_staking_rewards(
    player_data: &mut PlayerData,
    unrefined_rewards: &mut UnrefinedRewards,
    faction_state: &FactionState,
) -> Result<(u64, u64, u64)> {
    msg!("💰 Processing pending rewards before position update");
    let mut new_minebtc_rewards = 0;
    let mut new_sol_rewards = 0;
    let mut accrued_minebtc_rewards = 0;
    msg!(
        "📚 [update_lp_rewards] hashpower={} sol_index={} sol_debt={} minebtc_index={} minebtc_debt={} pending_sol_before={} pending_minebtc_before={}",
        player_data.lp_hashpower as f64 / 1e6,
        faction_state.lp_sol_reward_index,
        player_data.lp_sol_reward_debt,
        faction_state.lp_dogebtc_reward_index,
        player_data.lp_dogebtc_reward_debt,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_minebtc_rewards as f64 / 1e6
    );

    if player_data.lp_hashpower > 0 {
        // Calculate SOL rewards using helper function (convert u128 indexes to u64 for calculation)
        new_sol_rewards = helper::calculate_staking_rewards(
            player_data.lp_hashpower,
            faction_state.lp_sol_reward_index,
            player_data.lp_sol_reward_debt,
        )?;
        player_data.pending_sol_rewards = player_data
            .pending_sol_rewards
            .checked_add(new_sol_rewards)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "   Updated pending SOL rewards: {} (+{})",
            player_data.pending_sol_rewards as f64 / 1e9,
            new_sol_rewards as f64 / 1e9
        );

        new_minebtc_rewards = helper::calculate_staking_rewards(
            player_data.lp_hashpower,
            faction_state.lp_dogebtc_reward_index,
            player_data.lp_dogebtc_reward_debt,
        )?;
        accrued_minebtc_rewards =
            helper::add_to_total_claimable(unrefined_rewards, player_data, new_minebtc_rewards)?;
        msg!(
            "   Updated pending MineBtc rewards: {} (+{})",
            player_data.pending_minebtc_rewards as f64 / 1e6,
            new_minebtc_rewards as f64 / 1e6
        );
    } else {
        msg!("ℹ️ [update_lp_rewards] no lp hashpower; only syncing reward debt");
    }

    // Update reward debt to current indexes (MUST be outside if block to prevent
    // phantom rewards when user unstakes all LP and re-stakes later)
    player_data.lp_sol_reward_debt = faction_state.lp_sol_reward_index;
    player_data.lp_dogebtc_reward_debt = faction_state.lp_dogebtc_reward_index;
    msg!(
        "📚 [update_lp_rewards] debt_after_sync sol_debt={} minebtc_debt={} accrued_unrefined={} total_claimable={}",
        player_data.lp_sol_reward_debt,
        player_data.lp_dogebtc_reward_debt,
        accrued_minebtc_rewards as f64 / 1e6,
        unrefined_rewards.total_minebtc_claimable as f64 / 1e6
    );

    Ok((
        new_sol_rewards,
        new_minebtc_rewards,
        accrued_minebtc_rewards,
    ))
}

// ----------------------------------------------------------------------------------------
// ------------ ACCOUNT STRUCTS ----------------------------------------------------------
// ----------------------------------------------------------------------------------------

// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --------- STAKE DOGEBTC ---------
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

#[derive(Accounts)]
#[instruction(amount: u64, lockup_duration: u64, position_index: u8)]
pub struct StakeMineBtc<'info> {
    #[account(seeds = [GLOBAL_CONFIG_SEED.as_ref()], bump = global_config.bump)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(seeds = [HASHPOWER_CONFIG_SEED.as_ref()], bump)]
    pub hashpower_config: Box<Account<'info, HashpowerConfig>>,

    #[account(mut)]
    pub faction_state: Box<Account<'info, FactionState>>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    // Staked position
    #[account(
        init_if_needed,
        payer = authority,
        space = StakedPosition::LEN,
        seeds = [
            "0".as_ref(), // position_type
            STAKED_POSITION_SEED.as_ref(),
            authority.key().as_ref(),
            &[position_index]
        ],
        bump
    )]
    pub user_position: Account<'info, StakedPosition>,

    /// CHECK: MINE_BTC Mint (validated manually)
    pub minebtc_mint: InterfaceAccount<'info, Mint2022>,

    // Token accounts
    #[account(
        mut,
        constraint = user_minebtc_account.mint == minebtc_mint.key() @ ErrorCode::InvalidParameters,
        constraint = user_minebtc_account.owner == authority.key() @ ErrorCode::InvalidOwner,
        constraint = user_minebtc_account.amount >= amount @ ErrorCode::InsufficientFunds,
    )]
    /// User's MineBtc token account
    pub user_minebtc_account: InterfaceAccount<'info, TokenAccount2022>,

    #[account(
        mut,
        seeds = [MINEBTC_CUSTODIAN_SEED.as_ref()],
        bump,
        constraint = minebtc_custodian.mint == minebtc_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// Token-2022 account that holds staked MINE_BTC for this faction
    pub minebtc_custodian: InterfaceAccount<'info, TokenAccount2022>,

    #[account(
        mut,
        seeds = [UNREFINED_REWARDS_SEED.as_ref()],
        bump
    )]
    pub unrefined_rewards: Account<'info, UnrefinedRewards>,

    #[account(seeds = [TAX_CONFIG_SEED.as_ref()], bump = tax_config.bump)]
    pub tax_config: Account<'info, TaxConfig>,

    /// User who is staking tokens
    #[account(mut)]
    pub authority: Signer<'info>,

    /// System program for creating accounts
    pub system_program: Program<'info, System>,

    /// Token-2022 program for SPL-22 token operations
    pub token_program: Program<'info, Token2022>,
}

// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --------- UNSTAKE DOGEBTC ---------
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

#[derive(Accounts)]
#[instruction(position_index: u8)]
pub struct UnstakeMineBtc<'info> {
    // Global config
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    // Faction state
    #[account(mut)]
    pub faction_state: Box<Account<'info, FactionState>>,

    // Player data
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    // Staked position - will be closed and rent returned to authority
    #[account(
        mut,
        close = authority,
        seeds = [
            "0".as_ref(),
            STAKED_POSITION_SEED.as_ref(),
            authority.key().as_ref(),
            &[position_index]
        ],
        bump = user_position.bump,
        constraint = user_position.faction_id == player_data.faction_id @ ErrorCode::InvalidFactionId,
        constraint = user_position.position_type == 0 @ ErrorCode::InvalidParameters
    )]
    pub user_position: Box<Account<'info, StakedPosition>>,

    /// CHECK: MINE_BTC Mint - must be mut for burn instruction during emergency withdrawal
    #[account(mut)]
    pub minebtc_mint: Box<InterfaceAccount<'info, Mint2022>>,

    // Token accounts
    #[account(
        mut,
        constraint = user_minebtc_account.owner == authority.key() @ ErrorCode::InvalidOwner,
        constraint = user_minebtc_account.mint == minebtc_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// User's MineBtc token account to receive the unstaked tokens
    pub user_minebtc_account: Box<InterfaceAccount<'info, TokenAccount2022>>,

    #[account(
        mut,
        seeds = [MINEBTC_CUSTODIAN_SEED.as_ref()],
        bump,
        constraint = minebtc_custodian.mint == minebtc_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// Token-2022 account that holds staked MINE_BTC (global for all factions)
    pub minebtc_custodian: Box<InterfaceAccount<'info, TokenAccount2022>>,

    #[account(
        seeds = [MINEBTC_CUSTODIAN_AUTHORITY_SEED.as_ref()],
        bump,
    )]
    /// CHECK: Authority of the custodian (PDA that signs for token transfers, global for all factions)
    pub minebtc_custodian_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [UNREFINED_REWARDS_SEED.as_ref()],
        bump
    )]
    pub unrefined_rewards: Box<Account<'info, UnrefinedRewards>>,

    #[account(seeds = [TAX_CONFIG_SEED.as_ref()], bump = tax_config.bump)]
    pub tax_config: Box<Account<'info, TaxConfig>>,

    /// User who is unstaking tokens
    #[account(mut)]
    pub authority: Signer<'info>,

    /// System program for creating accounts
    pub system_program: Program<'info, System>,

    /// Token-2022 program for SPL-22 token operations
    pub token_program: Program<'info, Token2022>,
}

#[derive(Accounts)]
#[instruction(amount: u64, lockup_duration: u64, position_index: u8)]
pub struct StakeLpTokens<'info> {
    // Hashpower config (contains lockup and multiplier settings)
    #[account(
        seeds = [HASHPOWER_CONFIG_SEED.as_ref()],
        bump
    )]
    pub hashpower_config: Account<'info, HashpowerConfig>,

    // Faction state
    #[account(mut)]
    pub faction_state: Account<'info, FactionState>,

    // Player data
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    // Staked position (LP uses same StakedPosition struct)
    #[account(
        init_if_needed,
        payer = authority,
        space = StakedPosition::LEN,
        seeds = [
            "1".as_ref(), // position_type
            LP_STAKED_POSITION_SEED.as_ref(),
            authority.key().as_ref(),
            &[position_index]
        ],
        bump
    )]
    pub user_position: Account<'info, StakedPosition>,

    #[account(
        mut,
        seeds = [UNREFINED_REWARDS_SEED.as_ref()],
        bump
    )]
    pub unrefined_rewards: Account<'info, UnrefinedRewards>,

    #[account(seeds = [TAX_CONFIG_SEED.as_ref()], bump = tax_config.bump)]
    pub tax_config: Account<'info, TaxConfig>,

    /// CHECK: LP Mint (validated manually)
    pub lp_mint: Account<'info, token::Mint>,

    // Token accounts
    #[account(
        mut,
        constraint = user_lp_account.mint == lp_mint.key() @ ErrorCode::InvalidParameters,
        constraint = user_lp_account.owner == authority.key() @ ErrorCode::InvalidOwner,
        constraint = user_lp_account.amount >= amount @ ErrorCode::InsufficientFunds,
    )]
    /// User's LP token account
    pub user_lp_account: Account<'info, token::TokenAccount>,

    #[account(
        mut,
        seeds = [LIQUIDITY_CUSTODIAN_SEED.as_ref()],
        bump,
        constraint = liquidity_custodian.mint == lp_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// Token account that holds staked LP tokens for this faction
    pub liquidity_custodian: Account<'info, token::TokenAccount>,

    /// User who is staking tokens
    #[account(mut)]
    pub authority: Signer<'info>,

    /// System program for creating accounts
    pub system_program: Program<'info, System>,

    /// Token program for SPL token operations
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(position_index: u8)]
pub struct UnstakeLpTokens<'info> {
    // Global config
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    // Faction state
    #[account(mut)]
    pub faction_state: Box<Account<'info, FactionState>>,

    // Player data
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    // Staked position - will be closed and rent returned to authority
    #[account(
        mut,
        close = authority,
        seeds = [
            "1".as_ref(),
            LP_STAKED_POSITION_SEED.as_ref(),
            authority.key().as_ref(),
            &[position_index]
        ],
        bump = user_position.bump,
        constraint = user_position.faction_id == player_data.faction_id @ ErrorCode::InvalidFactionId,
        constraint = user_position.position_type == 1 @ ErrorCode::InvalidParameters
    )]
    pub user_position: Box<Account<'info, StakedPosition>>,

    #[account(
        mut,
        seeds = [UNREFINED_REWARDS_SEED.as_ref()],
        bump
    )]
    pub unrefined_rewards: Box<Account<'info, UnrefinedRewards>>,

    #[account(seeds = [TAX_CONFIG_SEED.as_ref()], bump = tax_config.bump)]
    pub tax_config: Box<Account<'info, TaxConfig>>,

    /// CHECK: LP Mint - must be mut for burn instruction during emergency withdrawal
    #[account(mut)]
    pub lp_mint: Box<Account<'info, token::Mint>>,

    // Token accounts
    #[account(
        mut,
        constraint = user_lp_account.owner == authority.key() @ ErrorCode::InvalidOwner,
        constraint = user_lp_account.mint == lp_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// User's LP token account to receive the unstaked tokens
    pub user_lp_account: Box<Account<'info, token::TokenAccount>>,

    #[account(
        mut,
        seeds = [LIQUIDITY_CUSTODIAN_SEED.as_ref()],
        bump,
        constraint = liquidity_custodian.mint == lp_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// Token account that holds staked LP tokens for this faction
    pub liquidity_custodian: Box<Account<'info, token::TokenAccount>>,

    #[account(
        seeds = [LIQUIDITY_CUSTODIAN_AUTHORITY_SEED.as_ref()],
        bump,
    )]
    /// CHECK: Authority of the custodian (PDA that signs for token transfers, global for all factions)
    pub liquidity_custodian_authority: UncheckedAccount<'info>,

    /// User who is unstaking tokens
    #[account(mut)]
    pub authority: Signer<'info>,

    /// System program for creating accounts
    pub system_program: Program<'info, System>,

    /// Token program for SPL token operations
    pub token_program: Program<'info, Token>,
}

// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --------- CLAIM STAKING REWARDS (SOL transfer + MineBTC accumulate) ---------
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

#[derive(Accounts)]
pub struct ClaimStakingRewards<'info> {
    // Faction state
    #[account()]
    pub faction_state: Account<'info, FactionState>,

    // Player data - must be owned by authority
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(
        mut,
        seeds = [UNREFINED_REWARDS_SEED.as_ref()],
        bump
    )]
    pub unrefined_rewards: Account<'info, UnrefinedRewards>,

    /// CHECK: SOL rewards vault (System Account)
    #[account(
        mut,
        seeds = [STAKER_SOL_REWARD_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_rewards_vault: UncheckedAccount<'info>,

    /// User claiming rewards (must be player_data.owner)
    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --------- WITHDRAW DBTC REWARDS (MineBTC transfer with fees) ---------
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

#[derive(Accounts)]
pub struct WithdrawDbtcRewards<'info> {
    #[account(seeds = [GLOBAL_CONFIG_SEED.as_ref()], bump = global_config.bump)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    // Player data - must be owned by authority
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    /// Optional only when the player has no referrer.
    /// Referred players must provide the canonical referrer's ReferralRewards PDA.
    #[account(
        mut,
        seeds = [REFERRAL_REWARDS_SEED.as_ref(), player_data.referral_code.as_ref()],
        bump
    )]
    pub referrer_rewards: Option<Account<'info, ReferralRewards>>,

    #[account(
        mut,
        seeds = [UNREFINED_REWARDS_SEED.as_ref()],
        bump
    )]
    pub unrefined_rewards: Account<'info, UnrefinedRewards>,

    /// CHECK: MINE_BTC Mint (validated manually)
    pub minebtc_mint: InterfaceAccount<'info, Mint2022>,

    // Token accounts
    #[account(
        mut,
        constraint = user_minebtc_account.mint == minebtc_mint.key() @ ErrorCode::InvalidParameters,
        constraint = user_minebtc_account.owner == authority.key() @ ErrorCode::InvalidOwner,
    )]
    /// User's MineBtc token account to receive rewards
    pub user_minebtc_account: InterfaceAccount<'info, TokenAccount2022>,

    /// CHECK: MineBtc mining state (needed for vault PDA derivation)
    #[account(
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump
    )]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

    /// CHECK: MineBtc token vault (main vault where tokens are deposited)
    #[account(
        mut,
        seeds = [MINE_BTC_VAULT_SEED.as_ref(), mine_btc_mining.key().as_ref()],
        bump,
        constraint = minebtc_token_vault.mint == minebtc_mint.key() @ ErrorCode::InvalidMint,
    )]
    pub minebtc_token_vault: InterfaceAccount<'info, TokenAccount2022>,

    #[account(
        seeds = [MINE_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump
    )]
    /// CHECK: Authority of the token vault (PDA that signs for token transfers)
    pub minebtc_vault_authority: UncheckedAccount<'info>,

    /// User claiming rewards
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,

    /// Token-2022 program for SPL-22 token operations
    pub token_program: Program<'info, Token2022>,
}

// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --------- CLAIM REFERRAL REWARDS ---------
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

#[derive(Accounts)]
pub struct ClaimReferralRewards<'info> {
    #[account(
        mut,
        seeds = [REFERRAL_REWARDS_SEED.as_ref(), authority.key().as_ref()],
        bump = referral_rewards.bump,
        constraint = referral_rewards.owner == authority.key() @ ErrorCode::InvalidOwner
    )]
    pub referral_rewards: Account<'info, ReferralRewards>,

    /// CHECK: MINE_BTC Mint (validated manually)
    pub minebtc_mint: InterfaceAccount<'info, Mint2022>,

    // Token accounts
    #[account(
        mut,
        constraint = user_minebtc_account.mint == minebtc_mint.key() @ ErrorCode::InvalidParameters,
        constraint = user_minebtc_account.owner == authority.key() @ ErrorCode::InvalidOwner,
    )]
    /// Referrer's MineBtc token account to receive rewards
    pub user_minebtc_account: InterfaceAccount<'info, TokenAccount2022>,

    /// CHECK: MineBtc mining state (needed for vault PDA derivation)
    #[account(
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump
    )]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

    /// CHECK: MineBtc token vault (main vault where tokens are deposited)
    #[account(
        mut,
        seeds = [MINE_BTC_VAULT_SEED.as_ref(), mine_btc_mining.key().as_ref()],
        bump,
        constraint = minebtc_token_vault.mint == minebtc_mint.key() @ ErrorCode::InvalidMint,
    )]
    pub minebtc_token_vault: InterfaceAccount<'info, TokenAccount2022>,

    #[account(
        seeds = [MINE_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump
    )]
    /// CHECK: Authority of the token vault (PDA that signs for token transfers)
    pub minebtc_vault_authority: UncheckedAccount<'info>,

    /// Referrer claiming rewards
    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,

    /// Token-2022 program for SPL-22 token operations
    pub token_program: Program<'info, Token2022>,
}
