use anchor_lang::prelude::*;
use anchor_lang::system_program::System;
use anchor_spl::token::{self, Token};

// # Staking Instructions
//
// Passive staking has three layers:
// - A user opens MineBTC or LP lockup positions in their home faction.
// - Lockup duration can add up to 3x weighted hashpower.
// - The player's staked HashBeasts can add another 3x passive hashpower multiplier.
// - The maxed-out staking setup is therefore capped at 9x total.
//
// Reward sources:
// - **SOL staking rewards** come from the round staker-fee lane and are paid out directly.
// - **MineBTC staking rewards** come from round mining distribution indexes and are
//   claimed directly with SOL staking rewards, outside the HODL-tax path.
// - **HODL tax redistribution** happens when a player withdraws gameplay-earned MineBTC
//   rewards: a configurable fee is taken from the eligible gameplay portion and re-indexed
//   across the remaining unclaimed gameplay rewards. Passive staking MineBTC is excluded.
//
// This file is intentionally verbose in its logs because the staking flows are one of the
// highest-value accounting surfaces in the program.
//

use crate::errors::ErrorCode;
use crate::events::*;
use crate::instructions::helper;
use crate::state::*;

use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface;
use anchor_spl::token_interface::{Mint as Mint2022, TokenAccount as TokenAccount2022};

pub const REFERRAL_BONUS_PCT: u64 = 1; // 1% flat bonus to referred user (paid in degenBTC at withdraw).
                                       // Referrer commissions accrue in SOL from referees' protocol fees on bets/mints,
                                       // not from degenBTC emission. Percentages live on `GlobalConfig.sol_fee_config`
                                       // (admin-tunable, capped at `MAX_REFERRAL_FEE_PCT = 10`); lifetime accrual is
                                       // capped at `MAX_REFERRER_SOL_LIFETIME`. Sybil farming is structurally
                                       // unprofitable: the sybil pays 100% of the bet but extracts only a fraction
                                       // of the protocol fee back.
                                       // --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
                                       // ---- STAKE DEGENBTC TOKENS :: User gets hashpower and SOL rewards ------
                                       // --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

fn calc_weighted_amount(staked_amount: u64, lockup_multiplier: u16) -> Result<u64> {
    let weighted_u128 = (staked_amount as u128)
        .checked_mul(lockup_multiplier as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(M_HUNDRED as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    u64::try_from(weighted_u128).map_err(|_| ErrorCode::ArithmeticOverflow.into())
}

fn calc_hashpower_contribution(weighted_amount: u64, hashbeast_multiplier: u16) -> Result<u64> {
    let hashpower_u128 = (weighted_amount as u128)
        .checked_mul(hashbeast_multiplier as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(BASE_MULTIPLIER as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    u64::try_from(hashpower_u128).map_err(|_| ErrorCode::ArithmeticOverflow.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_lockup_and_passive_hashbeasts_cap_staking_at_9x() {
        let deposit = 1_000_000u64;
        let weighted = calc_weighted_amount(deposit, 300).unwrap();
        let hashpower =
            calc_hashpower_contribution(weighted, PASSIVE_HASHBEAST_STAKING_MAX_MULTIPLIER)
                .unwrap();

        assert_eq!(weighted, deposit * 3);
        assert_eq!(hashpower, deposit * 9);
    }
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
    crate::log_fn!("stake", "int_stake_minebtc");
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
    let user_position_bump = ctx.bumps.user_position;

    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let user_position = &mut ctx.accounts.user_position;

    let hashpower_config = &ctx.accounts.hashpower_config;

    msg!(
        "🧭 [stake_minebtc] owner={} player={} faction_state={} faction_id={} current_position_count={}",
        ctx.accounts.authority.key(),
        player_data_key,
        faction_state.key(),
        faction_state.faction_id,
        player_data.degenbtc_position_indices.len()
    );
    msg!(
        "🧾 [stake_minebtc] player_before degenbtc_staked={} degenbtc_hashpower={} hashbeast_multiplier={} pending_sol={} pending_minebtc={}",
        player_data.degenbtc_staked as f64 / 1e6,
        player_data.degenbtc_hashpower as f64 / 1e6,
        player_data.hashbeast_multiplier as f64 / 1000.0,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_minebtc_rewards as f64 / 1e6
    );
    msg!(
        "🧾 [stake_minebtc] faction_before degenbtc_staked={} total_degenbtc_hashpower={} sol_index={} minebtc_index={}",
        faction_state.degenbtc_staked as f64 / 1e6,
        faction_state.total_degenbtc_hashpower as f64 / 1e6,
        faction_state.degenbtc_sol_reward_index,
        faction_state.degenbtc_degenbtc_reward_index
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

    // Cannot add more degenBTC to existing position
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
        "🔥 [stake_minebtc] transfer_fee bps={} max_fee={} requested={} fee={} credited={}",
        transfer_fee_info.transfer_fee_basis_points,
        transfer_fee_info.max_fee as f64 / 1e6,
        amount as f64 / 1e6,
        transfer_fee_info.fee_amount as f64 / 1e6,
        actual_amount as f64 / 1e6
    );
    msg!(
        "📊 Current faction state - Total staked: {}, Total hashpower: {}",
        faction_state.degenbtc_staked as f64 / 1e6,
        faction_state.total_degenbtc_hashpower as f64 / 1e6
    );

    // Add position index to player data
    helper::add_degenbtc_position(player_data, position_index)?;
    msg!(
        "🔍 [stake_minebtc] Position index added: {}",
        position_index
    );
    msg!(
        "🔍 [stake_minebtc] Player data - Position indices: {:?}",
        player_data.degenbtc_position_indices
    );
    msg!(
        "🔍 [stake_minebtc] Player data - Total positions: {}",
        player_data.degenbtc_position_indices.len()
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
    let weighted_amount = calc_weighted_amount(actual_amount, multiplier)?;
    msg!(
        "⚖️ Weighted amount: {} (actual amount: {} × multiplier: {}%)",
        weighted_amount as f64 / 1e6,
        actual_amount as f64 / 1e6,
        multiplier as f64 / 100.0
    );

    // -------------- ACCRUE PENDING REWARDS -------------- //

    // Process pending rewards before updating position
    let (new_sol_rewards, new_minebtc_rewards) =
        int_update_minebtc_staking_rewards(player_data, faction_state)?;
    msg!(
        "💹 [stake_minebtc] accrued_before_stake new_sol={} new_minebtc={} pending_sol={} pending_staking_minebtc={}",
        new_sol_rewards as f64 / 1e9,
        new_minebtc_rewards as f64 / 1e6,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_staking_minebtc_rewards as f64 / 1e6
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
        user_position_bump,
    )?;

    // -------------- UPDATE PLAYER AND FACTION DATA -------------- //

    let hashbeasts_multiplier = player_data.hashbeast_multiplier;
    let weighted_amount_with_hashbeasts =
        calc_hashpower_contribution(weighted_amount, hashbeasts_multiplier)?;
    let prev_player_degenbtc_hashpower = player_data.degenbtc_hashpower;
    let prev_player_degenbtc_staked = player_data.degenbtc_staked;
    let prev_faction_degenbtc_staked = faction_state.degenbtc_staked;
    let prev_faction_degenbtc_hashpower = faction_state.total_degenbtc_hashpower;
    msg!(
        "⚙️ [stake_minebtc] position_math actual_amount={} weighted_amount={} hashbeast_multiplier={}x hashpower_contribution={}",
        actual_amount as f64 / 1e6,
        weighted_amount as f64 / 1e6,
        hashbeasts_multiplier as f64 / 1000.0,
        weighted_amount_with_hashbeasts as f64 / 1e6
    );

    // Update player data state
    player_data.degenbtc_hashpower = player_data
        .degenbtc_hashpower
        .checked_add(weighted_amount_with_hashbeasts)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    player_data.degenbtc_staked = player_data
        .degenbtc_staked
        .checked_add(actual_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // Update faction state with actual_amount (post-tax) and weighted_amount
    faction_state.degenbtc_staked = faction_state
        .degenbtc_staked
        .checked_add(actual_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    faction_state.total_degenbtc_hashpower = faction_state
        .total_degenbtc_hashpower
        .checked_add(weighted_amount_with_hashbeasts)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   Updated faction state - Total staked: {}, Total hashpower: {}",
        faction_state.degenbtc_staked as f64 / 1e6,
        faction_state.total_degenbtc_hashpower as f64 / 1e6
    );
    msg!(
        "📈 [stake_minebtc] player_after degenbtc_staked={} -> {} degenbtc_hashpower={} -> {}",
        prev_player_degenbtc_staked as f64 / 1e6,
        player_data.degenbtc_staked as f64 / 1e6,
        prev_player_degenbtc_hashpower as f64 / 1e6,
        player_data.degenbtc_hashpower as f64 / 1e6
    );
    msg!(
        "📈 [stake_minebtc] faction_after degenbtc_staked={} -> {} total_degenbtc_hashpower={} -> {}",
        prev_faction_degenbtc_staked as f64 / 1e6,
        faction_state.degenbtc_staked as f64 / 1e6,
        prev_faction_degenbtc_hashpower as f64 / 1e6,
        faction_state.total_degenbtc_hashpower as f64 / 1e6
    );

    // -------------- TRANSFER TOKENS -------------- //

    // Transfer tokens from user to custodian
    msg!(
        "💱 Transferring {} dBTC tokens from user to custodian",
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
        hashpower_contribution: weighted_amount_with_hashbeasts,
        new_sol_rewards,
        new_minebtc_rewards,
        unrefined_minebtc: 0,
        timestamp: current_ts,
    });

    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- UNSTAKE DEGENBTC TOKENS :: User gets MINE_BTC back ------------------------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Unstake MineBtc tokens from a position
pub fn int_unstake_minebtc(ctx: Context<UnstakeMineBtc>, position_index: u8) -> Result<()> {
    crate::log_fn!("stake", "int_unstake_minebtc");
    // Store values before mutable borrow (for event emission)
    let position_key = ctx.accounts.user_position.key();
    let player_data_key = ctx.accounts.player_data.key();

    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let user_position = &mut ctx.accounts.user_position;
    let current_ts = Clock::get()?.unix_timestamp;

    msg!(
        "🔓 [unstake_minebtc] Processing unstake for position {}",
        position_index
    );
    msg!(
        "🧭 [unstake_minebtc] owner={} player={} faction_state={} faction_id={} hashbeast_multiplier={}",
        ctx.accounts.authority.key(),
        player_data_key,
        faction_state.key(),
        faction_state.faction_id,
        player_data.hashbeast_multiplier as f64 / 1000.0
    );
    msg!(
        "🧾 [unstake_minebtc] player_before degenbtc_staked={} degenbtc_hashpower={} pending_sol={} pending_minebtc={}",
        player_data.degenbtc_staked as f64 / 1e6,
        player_data.degenbtc_hashpower as f64 / 1e6,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_minebtc_rewards as f64 / 1e6
    );
    msg!(
        "🧾 [unstake_minebtc] faction_before degenbtc_staked={} total_degenbtc_hashpower={} sol_index={} minebtc_index={}",
        faction_state.degenbtc_staked as f64 / 1e6,
        faction_state.total_degenbtc_hashpower as f64 / 1e6,
        faction_state.degenbtc_sol_reward_index,
        faction_state.degenbtc_degenbtc_reward_index
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
            .degenbtc_position_indices
            .contains(&position_index),
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
    let (new_sol_rewards, new_minebtc_rewards) =
        int_update_minebtc_staking_rewards(player_data, faction_state)?;
    msg!(
        "💹 [unstake_minebtc] accrued_before_unstake new_sol={} new_minebtc={} pending_sol={} pending_staking_minebtc={}",
        new_sol_rewards as f64 / 1e9,
        new_minebtc_rewards as f64 / 1e6,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_staking_minebtc_rewards as f64 / 1e6
    );

    // -------------- UPDATE FACTION AND PLAYER DATA -------------- //

    // Calculate return amount based on early withdrawal status
    let is_early_withdrawal = current_ts < user_position.lockup_end_timestamp;
    let staked_amount = user_position.staked_amount;
    let original_weighted = user_position.weighted_amount;
    let hashpower_contribution =
        calc_hashpower_contribution(original_weighted, player_data.hashbeast_multiplier)?;
    let mut return_amount = staked_amount;
    let mut penalty_amount = 0u64;
    let prev_player_degenbtc_hashpower = player_data.degenbtc_hashpower;
    let prev_player_degenbtc_staked = player_data.degenbtc_staked;
    let prev_faction_degenbtc_staked = faction_state.degenbtc_staked;
    let prev_faction_degenbtc_hashpower = faction_state.total_degenbtc_hashpower;
    msg!(
        "⚙️ [unstake_minebtc] position_math staked_amount={} weighted_amount={} hashbeast_multiplier={}x hashpower_contribution={} is_early={}",
        staked_amount as f64 / 1e6,
        original_weighted as f64 / 1e6,
        player_data.hashbeast_multiplier as f64 / 1000.0,
        hashpower_contribution as f64 / 1e6,
        is_early_withdrawal
    );

    // Update faction state (decrease staked amount and hashpower)
    msg!("📊 Updating faction state");
    faction_state.degenbtc_staked = faction_state
        .degenbtc_staked
        .checked_sub(staked_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    faction_state.total_degenbtc_hashpower = faction_state
        .total_degenbtc_hashpower
        .checked_sub(hashpower_contribution)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   New faction totals - Staked: {}, Hashpower: {}",
        faction_state.degenbtc_staked as f64 / 1e6,
        faction_state.total_degenbtc_hashpower as f64 / 1e6
    );

    // Update player data (decrease hashpower and staked amount)
    msg!("📊 Updating player data");
    player_data.degenbtc_hashpower = player_data
        .degenbtc_hashpower
        .checked_sub(hashpower_contribution)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    player_data.degenbtc_staked = player_data
        .degenbtc_staked
        .checked_sub(staked_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   New player totals - Hashpower: {}, Staked: {}",
        player_data.degenbtc_hashpower as f64 / 1e6,
        player_data.degenbtc_staked as f64 / 1e6
    );
    msg!(
        "📈 [unstake_minebtc] player_after degenbtc_staked={} -> {} degenbtc_hashpower={} -> {}",
        prev_player_degenbtc_staked as f64 / 1e6,
        player_data.degenbtc_staked as f64 / 1e6,
        prev_player_degenbtc_hashpower as f64 / 1e6,
        player_data.degenbtc_hashpower as f64 / 1e6
    );
    msg!(
        "📈 [unstake_minebtc] faction_after degenbtc_staked={} -> {} total_degenbtc_hashpower={} -> {}",
        prev_faction_degenbtc_staked as f64 / 1e6,
        faction_state.degenbtc_staked as f64 / 1e6,
        prev_faction_degenbtc_hashpower as f64 / 1e6,
        faction_state.total_degenbtc_hashpower as f64 / 1e6
    );

    // Remove position from user's active positions
    helper::remove_degenbtc_position(player_data, position_index)?;

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
        )?;
        return_amount = staked_amount - penalty_amount;
        msg!(
            "   Total Staked: {}, Returned: {}, Penalty: {}",
            staked_amount,
            return_amount,
            penalty_amount
        );

        // Charge emergency tax if any penalty
        if penalty_amount > 0 {
            // Charge the full early-withdrawal penalty by burning it from custody.
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
    let total_lockup_seconds = user_position
        .lockup_end_timestamp
        .checked_sub(user_position.start_timestamp)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let remaining_seconds = user_position
        .lockup_end_timestamp
        .checked_sub(current_ts)
        .unwrap_or(0);
    let remaining_seconds_pct = if total_lockup_seconds > 0 && remaining_seconds > 0 {
        u64::try_from(helper::mul_div(
            M_HUNDRED,
            u64::try_from(remaining_seconds).map_err(|_| ErrorCode::ArithmeticOverflow)?,
            u64::try_from(total_lockup_seconds).map_err(|_| ErrorCode::ArithmeticOverflow)?,
        )?)
        .map_err(|_| ErrorCode::ArithmeticOverflow)?
    } else {
        0u64
    };
    let calc_penalty_pct = if is_early_withdrawal && penalty_amount > 0 {
        u64::try_from(helper::mul_div(
            EMERGENCY_WITHDRAWAL_PENALTY_PCT as u64,
            remaining_seconds_pct,
            M_HUNDRED,
        )?)
        .map_err(|_| ErrorCode::ArithmeticOverflow)?
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
        unrefined_minebtc: 0,
        original_amount: staked_amount,
        returned_amount: return_amount,
        timestamp: current_ts,
    });

    // Emit emergency withdrawal event if early withdrawal
    if is_early_withdrawal && penalty_amount > 0 {
        let days_remaining = if current_ts < user_position.lockup_end_timestamp {
            ((user_position.lockup_end_timestamp - current_ts) as u64)
                .checked_add(86400 - 1)
                .unwrap_or(0)
                / 86400
        } else {
            0
        };

        emit!(PaperHandBurned {
            owner: ctx.accounts.authority.key(),
            player_data: player_data_key,
            position_key,
            position_index,
            staked_token_type: 0, // MineBTC
            original_amount: staked_amount,
            penalty_amount,
            returned_amount: return_amount,
            penalty_tax_pct: calc_penalty_pct,
            days_remaining,
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
    crate::log_fn!("stake", "int_stake_lp_tokens");
    msg!(
        "🔒 [stake_lp_tokens] Starting LP token staking - Amount: {}, Lockup: {} days, Position: {}",
        amount,
        lockup_duration,
        position_index,
    );

    let current_ts = Clock::get()?.unix_timestamp;

    // Store values before mutable borrow (for event emission)
    let player_data_key = ctx.accounts.player_data.key();
    let user_position_bump = ctx.bumps.user_position;

    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let user_position = &mut ctx.accounts.user_position;

    let hashpower_config = &ctx.accounts.hashpower_config;

    msg!(
        "🧭 [stake_lp_tokens] owner={} player={} faction_state={} faction_id={} current_position_count={}",
        ctx.accounts.authority.key(),
        player_data_key,
        faction_state.key(),
        faction_state.faction_id,
        player_data.lp_position_indices.len()
    );
    msg!(
        "🧾 [stake_lp_tokens] player_before lp_staked={} lp_hashpower={} hashbeast_multiplier={} pending_sol={} pending_minebtc={}",
        player_data.lp_staked as f64 / 1e6,
        player_data.lp_hashpower as f64 / 1e6,
        player_data.hashbeast_multiplier as f64 / 1000.0,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_minebtc_rewards as f64 / 1e6
    );
    msg!(
        "🧾 [stake_lp_tokens] faction_before lp_staked={} total_lp_hashpower={} sol_index={} minebtc_index={}",
        faction_state.lp_staked as f64 / 1e6,
        faction_state.total_lp_hashpower as f64 / 1e6,
        faction_state.lp_sol_reward_index,
        faction_state.lp_degenbtc_reward_index
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

    // LP positions currently credit the full deposited amount. There is no Token-2022 fee
    // normalization step here because LP tokens are standard SPL tokens.
    let actual_amount = amount;
    msg!(
        "📊 Current faction state - Total LP staked: {}, Total LP hashpower: {}",
        faction_state.lp_staked,
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
    let weighted_amount = calc_weighted_amount(actual_amount, multiplier)?;
    msg!(
        "⚖️ Weighted amount: {} (amount: {} × multiplier: {}%)",
        weighted_amount as f64 / 1e6,
        actual_amount as f64 / 1e6,
        multiplier as f64 / 100.0
    );

    // -------------- ACCRUE PENDING REWARDS -------------- //

    // Process pending rewards before updating position
    let (new_sol_rewards, new_minebtc_rewards) =
        int_update_lp_staking_rewards(player_data, faction_state)?;
    msg!(
        "💹 [stake_lp_tokens] accrued_before_stake new_sol={} new_minebtc={} pending_sol={} pending_staking_minebtc={}",
        new_sol_rewards as f64 / 1e9,
        new_minebtc_rewards as f64 / 1e6,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_staking_minebtc_rewards as f64 / 1e6
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
        user_position_bump,
    )?;

    // -------------- UPDATE PLAYER AND FACTION DATA -------------- //

    let hashbeasts_multiplier = player_data.hashbeast_multiplier;
    let weighted_amount_with_hashbeasts =
        calc_hashpower_contribution(weighted_amount, hashbeasts_multiplier)?;
    let prev_player_lp_hashpower = player_data.lp_hashpower;
    let prev_player_lp_staked = player_data.lp_staked;
    let prev_faction_lp_staked = faction_state.lp_staked;
    let prev_faction_lp_hashpower = faction_state.total_lp_hashpower;
    msg!(
        "⚙️ [stake_lp_tokens] position_math actual_amount={} weighted_amount={} hashbeast_multiplier={}x hashpower_contribution={}",
        actual_amount as f64 / 1e6,
        weighted_amount as f64 / 1e6,
        hashbeasts_multiplier as f64 / 1000.0,
        weighted_amount_with_hashbeasts as f64 / 1e6
    );

    // Update player data state
    player_data.lp_hashpower = player_data
        .lp_hashpower
        .checked_add(weighted_amount_with_hashbeasts)
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
        .checked_add(weighted_amount_with_hashbeasts)
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
        hashpower_contribution: weighted_amount_with_hashbeasts,
        new_sol_rewards,
        new_minebtc_rewards,
        unrefined_minebtc: 0,
        timestamp: current_ts,
    });

    Ok(())
}

/// Unstake LP tokens from a position
pub fn int_unstake_lp_tokens(ctx: Context<UnstakeLpTokens>, position_index: u8) -> Result<()> {
    crate::log_fn!("stake", "int_unstake_lp_tokens");
    // Store values before mutable borrow (for event emission)
    let player_data_key = ctx.accounts.player_data.key();
    let position_key = ctx.accounts.user_position.key();

    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let user_position = &mut ctx.accounts.user_position;
    let current_ts = Clock::get()?.unix_timestamp;

    msg!(
        "🔓 [unstake_lp_tokens] Processing unstake for position {}",
        position_index
    );
    msg!(
        "🧭 [unstake_lp_tokens] owner={} player={} faction_state={} faction_id={} hashbeast_multiplier={}",
        ctx.accounts.authority.key(),
        player_data_key,
        faction_state.key(),
        faction_state.faction_id,
        player_data.hashbeast_multiplier as f64 / 1000.0
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
        faction_state.lp_degenbtc_reward_index
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
    let (new_sol_rewards, new_minebtc_rewards) =
        int_update_lp_staking_rewards(player_data, faction_state)?;
    msg!(
        "💹 [unstake_lp_tokens] accrued_before_unstake new_sol={} new_minebtc={} pending_sol={} pending_staking_minebtc={}",
        new_sol_rewards as f64 / 1e9,
        new_minebtc_rewards as f64 / 1e6,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_staking_minebtc_rewards as f64 / 1e6
    );

    // -------------- UPDATE FACTION AND PLAYER DATA -------------- //

    // Calculate return amount based on early withdrawal status
    let is_early_withdrawal = current_ts < user_position.lockup_end_timestamp;
    let staked_amount = user_position.staked_amount;
    let original_weighted = user_position.weighted_amount;
    let hashpower_contribution =
        calc_hashpower_contribution(original_weighted, player_data.hashbeast_multiplier)?;
    let mut return_amount = staked_amount;
    let mut penalty_amount = 0u64;
    let prev_player_lp_hashpower = player_data.lp_hashpower;
    let prev_player_lp_staked = player_data.lp_staked;
    let prev_faction_lp_staked = faction_state.lp_staked;
    let prev_faction_lp_hashpower = faction_state.total_lp_hashpower;
    msg!(
        "⚙️ [unstake_lp_tokens] position_math staked_amount={} weighted_amount={} hashbeast_multiplier={}x hashpower_contribution={} is_early={}",
        staked_amount as f64 / 1e6,
        original_weighted as f64 / 1e6,
        player_data.hashbeast_multiplier as f64 / 1000.0,
        hashpower_contribution as f64 / 1e6,
        is_early_withdrawal
    );

    // Update faction state (decrease staked amount and hashpower)
    msg!("📊 Updating faction state");
    faction_state.lp_staked = faction_state
        .lp_staked
        .checked_sub(staked_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    faction_state.total_lp_hashpower = faction_state
        .total_lp_hashpower
        .checked_sub(hashpower_contribution)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   New faction totals - Staked: {}, Hashpower: {}",
        faction_state.lp_staked as f64 / 1e6,
        faction_state.total_lp_hashpower as f64 / 1e6
    );

    // Update player data (decrease hashpower and staked amount)
    msg!("📊 Updating player data");
    player_data.lp_hashpower = player_data
        .lp_hashpower
        .checked_sub(hashpower_contribution)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    player_data.lp_staked = player_data
        .lp_staked
        .checked_sub(staked_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
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
        )?;
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
    let total_lockup_seconds = user_position
        .lockup_end_timestamp
        .checked_sub(user_position.start_timestamp)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let remaining_seconds = if is_early_withdrawal {
        user_position
            .lockup_end_timestamp
            .checked_sub(current_ts)
            .unwrap_or(0)
    } else {
        0
    };
    let remaining_seconds_pct =
        if total_lockup_seconds > 0 && is_early_withdrawal && remaining_seconds > 0 {
            u64::try_from(helper::mul_div(
                M_HUNDRED,
                u64::try_from(remaining_seconds).map_err(|_| ErrorCode::ArithmeticOverflow)?,
                u64::try_from(total_lockup_seconds).map_err(|_| ErrorCode::ArithmeticOverflow)?,
            )?)
            .map_err(|_| ErrorCode::ArithmeticOverflow)?
        } else {
            0u64
        };
    let calc_penalty_pct = if is_early_withdrawal && penalty_amount > 0 {
        u64::try_from(helper::mul_div(
            EMERGENCY_WITHDRAWAL_PENALTY_PCT as u64,
            remaining_seconds_pct,
            M_HUNDRED,
        )?)
        .map_err(|_| ErrorCode::ArithmeticOverflow)?
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
        unrefined_minebtc: 0,
        original_amount: staked_amount,
        returned_amount: return_amount,
        timestamp: current_ts,
    });

    // Emit emergency withdrawal event if early withdrawal
    if is_early_withdrawal && penalty_amount > 0 {
        let days_remaining = if current_ts < user_position.lockup_end_timestamp {
            ((user_position.lockup_end_timestamp - current_ts) as u64)
                .checked_add(86400 - 1)
                .unwrap_or(0)
                / 86400
        } else {
            0
        };

        emit!(PaperHandBurned {
            owner: ctx.accounts.authority.key(),
            player_data: player_data_key,
            position_index,
            position_key,
            staked_token_type: 1, // LP
            original_amount: staked_amount,
            penalty_amount,
            returned_amount: return_amount,
            penalty_tax_pct: calc_penalty_pct,
            days_remaining,
            timestamp: current_ts,
        });
    }

    // Account will be automatically closed by Anchor (close = authority in account struct)
    msg!("✅ [unstake_lp_tokens] Unstake completed successfully. Position account will be closed.");
    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- CLAIM STAKING REWARDS :: Updates indexes and transfers passive staking rewards ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Claim staking rewards - updates all staking indexes and transfers SOL + staking MineBTC
/// directly to owner. Staking MineBTC never enters the gameplay HODL-tax pool.
pub fn int_claim_staking_rewards(ctx: Context<ClaimStakingRewards>) -> Result<()> {
    crate::log_fn!("stake", "int_claim_staking_rewards");
    msg!("💰 [claim_staking_rewards] Claiming SOL and staking MineBTC rewards");

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
        "🧾 [claim_staking_rewards] pending_before sol={} staking_minebtc={} gameplay_minebtc={} degenbtc_hashpower={} lp_hashpower={}",
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_staking_minebtc_rewards as f64 / 1e6,
        player_data.pending_minebtc_rewards as f64 / 1e6,
        player_data.degenbtc_hashpower as f64 / 1e6,
        player_data.lp_hashpower as f64 / 1e6
    );

    // Process MineBtc and LP staking rewards before paying out.
    let (_st_minebtc_new_sol_rewards, _st_minebtc_new_minebtc_rewards) =
        int_update_minebtc_staking_rewards(player_data, faction_state)?;
    let (_st_lp_new_sol_rewards, _st_lp_new_minebtc_rewards) =
        int_update_lp_staking_rewards(player_data, faction_state)?;
    msg!(
        "💹 [claim_staking_rewards] pending_after_index_sync sol={} staking_minebtc={} gameplay_minebtc={}",
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_staking_minebtc_rewards as f64 / 1e6,
        player_data.pending_minebtc_rewards as f64 / 1e6
    );

    let total_pending_sol_rewards = player_data.pending_sol_rewards;
    let total_pending_minebtc_rewards = player_data.pending_staking_minebtc_rewards;
    require!(
        total_pending_sol_rewards > 0 || total_pending_minebtc_rewards > 0,
        ErrorCode::InsufficientFunds
    );
    msg!(
        "   Total claimable SOL rewards: {} lamports",
        total_pending_sol_rewards as f64 / 1e9
    );
    msg!(
        "   Total claimable staking MineBtc rewards: {} minebtc",
        total_pending_minebtc_rewards as f64 / 1e6
    );

    let player_sol = total_pending_sol_rewards;

    if player_sol > 0 {
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
    }

    if total_pending_minebtc_rewards > 0 {
        msg!(
            "   Transferring {} staking MineBtc from vault to user",
            total_pending_minebtc_rewards as f64 / 1e6
        );
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
            total_pending_minebtc_rewards,
            ctx.accounts.minebtc_mint.decimals,
        )?;
        ctx.accounts.mine_btc_mining.total_tokens_distributed = ctx
            .accounts
            .mine_btc_mining
            .total_tokens_distributed
            .checked_add(total_pending_minebtc_rewards)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "     ✓ Updated total tokens distributed: {} (+{})",
            ctx.accounts.mine_btc_mining.total_tokens_distributed as f64 / 1e6,
            total_pending_minebtc_rewards as f64 / 1e6
        );
        msg!("     ✓ Staking MineBtc rewards transferred to user");
    }

    // Reset pending rewards
    player_data.pending_sol_rewards = 0;
    player_data.pending_staking_minebtc_rewards = 0;

    emit!(SolRewardsClaimed {
        user: ctx.accounts.authority.key(),
        player_data: player_data_key,
        faction_id,
        sol_amount: player_sol,
        minebtc_amount: total_pending_minebtc_rewards,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "✅ [claim_staking_rewards] Claimed {} SOL and {} staking MineBtc",
        player_sol as f64 / 1e9,
        total_pending_minebtc_rewards as f64 / 1e6,
    );
    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- WITHDRAW DBTC REWARDS :: User withdraws accumulated MineBTC with fees ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Withdraw accumulated MineBtc token rewards
/// Implements HODL tax on gameplay-earned rewards only.
/// NOTE: Call claim_staking_rewards first to update staking indexes and accumulate rewards
pub fn int_withdraw_dbtc_rewards(ctx: Context<WithdrawDbtcRewards>) -> Result<()> {
    crate::log_fn!("stake", "int_withdraw_dbtc_rewards");
    msg!("💰 [withdraw_dbtc_rewards] Withdrawing MineBtc with HODL tax");

    // Store values before mutable borrow (for event emission)
    let player_data_key = ctx.accounts.player_data.key();
    let faction_id = ctx.accounts.player_data.faction_id;
    let player_owner = ctx.accounts.player_data.owner;

    let player_data = &mut ctx.accounts.player_data;
    let unrefined_minebtc = &mut ctx.accounts.hodl_pool;
    let global_config = &ctx.accounts.global_config;

    // Realize any deferred hodl-tax-index rewards before applying a new HODL tax.
    // Without this sync, users with no fresh staking updates can miss previously accrued
    // HODL-tax distributions when they go straight to withdraw.
    let synced_unrefined_bonus = helper::add_to_total_claimable(
        unrefined_minebtc,
        player_data,
        0,
        player_owner,
        player_data_key,
        CLAIMABLE_MINEBTC_SOURCE_REFINING_SYNC,
        0,
    )?;

    msg!(
        "🧭 [withdraw_dbtc_rewards] owner={} player={} faction_id={} pending_minebtc={} total_claimable={} hodl_tax_index={} synced_unrefined_bonus={}",
        player_owner,
        player_data_key,
        faction_id,
        player_data.pending_minebtc_rewards as f64 / 1e6,
        unrefined_minebtc.total_minebtc_claimable as f64 / 1e6,
        unrefined_minebtc.hodl_tax_index,
        synced_unrefined_bonus as f64 / 1e6
    );

    require!(
        player_data.pending_minebtc_rewards > 0,
        ErrorCode::InsufficientFunds
    );

    let base_pending = player_data.pending_minebtc_rewards;
    let remaining_claimable_after_this_user = unrefined_minebtc
        .total_minebtc_claimable
        .checked_sub(base_pending)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // pending_minebtc_rewards is gameplay-only. Passive staking MineBTC is paid by
    // claim_staking_rewards and never enters this HODL-tax pool.
    let hodl_tax_pct = global_config.minebtc_dist_config.hodl_tax_pct as u64;
    let hodl_tax = if remaining_claimable_after_this_user > 0 {
        u64::try_from(helper::mul_div(base_pending, hodl_tax_pct, M_HUNDRED)?)
            .map_err(|_| ErrorCode::ArithmeticOverflow)?
    } else {
        0
    };
    let base_claimable_amount = base_pending
        .checked_sub(hodl_tax)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "🧮 [withdraw_dbtc_rewards] base_pending={} remaining_after_user={} hodl_tax_pct={} hodl_tax={} base_claimable={}",
        base_pending as f64 / 1e6,
        remaining_claimable_after_this_user as f64 / 1e6,
        hodl_tax_pct,
        hodl_tax as f64 / 1e6,
        base_claimable_amount as f64 / 1e6
    );

    // Apply referral bonus to the referee only. Referrer commissions accrue in SOL
    // from referees' protocol fees (see internal_process_bets / NFT mint flows),
    // not from degenBTC emission. This keeps the 21M cap unaffected by referrals
    // and makes sybil farming structurally unprofitable.
    let has_referrer = player_data.referral_code != ctx.accounts.system_program.key();
    let referral_bonus = if has_referrer {
        helper::validate_referrer_rewards_account(
            &player_data.referral_code,
            ctx.accounts.referrer_rewards.as_ref(),
        )?;
        let bonus = u64::try_from(helper::mul_div(
            base_claimable_amount,
            REFERRAL_BONUS_PCT,
            M_HUNDRED,
        )?)
        .map_err(|_| ErrorCode::ArithmeticOverflow)?;
        msg!("   Referral bonus (+1%): {} minebtc", bonus as f64 / 1e6);
        bonus
    } else {
        0
    };
    let referral_reward = 0u64; // Referrer SOL commissions are paid out-of-band on bets/mints, not here.

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
    // and inflate the HODL tax index for remaining gameplay reward claimants.
    unrefined_minebtc.total_minebtc_claimable = unrefined_minebtc
        .total_minebtc_claimable
        .checked_sub(base_pending)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    player_data.pending_minebtc_rewards = 0;
    player_data.unrefined_minebtc_rewards = 0;
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

    // Redistribute HODL tax to other gameplay reward claimants. Passive staking
    // rewards are not in this denominator and do not earn this yield.
    if hodl_tax > 0 {
        msg!("   Redistributing HODL tax to gameplay reward claimants...");
        let increment = helper::mul_div(
            hodl_tax,
            INDEX_PRECISION,
            unrefined_minebtc.total_minebtc_claimable,
        )?;
        unrefined_minebtc.hodl_tax_index = unrefined_minebtc
            .hodl_tax_index
            .checked_add(increment)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "📈 [withdraw_dbtc_rewards] hodl_tax_index_after={} increment={} remaining_gameplay_claimable={}",
            unrefined_minebtc.hodl_tax_index,
            increment,
            unrefined_minebtc.total_minebtc_claimable as f64 / 1e6
        );
        emit!(HodlTaxRedistributed {
            paper_hand: player_owner,
            player_data: player_data_key,
            tax_amount: hodl_tax,
            redistributed_amount: hodl_tax,
            redistributed_index_increment: increment as u128,
            remaining_total_claimable: unrefined_minebtc.total_minebtc_claimable,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    emit!(DbtcRewardsClaimed {
        user: player_owner,
        player_data: player_data_key,
        faction_id,
        minebtc_amount: claimable_by_user,
        hodl_tax,
        referral_bonus,
        referral_reward,
        referrer: referrer_pubkey,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "✅ [withdraw_dbtc_rewards] withdrew={} owner={} bonus={} referrer_reward={} pending_after={} total_claimable_after={}",
        claimable_by_user as f64 / 1e6,
        player_owner,
        referral_bonus as f64 / 1e6,
        referral_reward as f64 / 1e6,
        player_data.pending_minebtc_rewards as f64 / 1e6,
        unrefined_minebtc.total_minebtc_claimable as f64 / 1e6
    );

    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- CLAIM REFERRAL REWARDS :: Referrers claim their earned rewards ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Claim referral rewards (SOL and MineBtc)
pub fn int_claim_referral_rewards(ctx: Context<ClaimReferralRewards>) -> Result<()> {
    crate::log_fn!("stake", "int_claim_referral_rewards");
    msg!("💰 [claim_referral_rewards] Claiming referral rewards");

    let referral_rewards = &mut ctx.accounts.referral_rewards;
    let pending_sol = referral_rewards.pending_sol_rewards;

    require!(pending_sol > 0, ErrorCode::InsufficientFunds);

    let mut claimed_sol = 0u64;

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
        minebtc_amount: 0,
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
    faction_state: &FactionState,
) -> Result<(u64, u64)> {
    crate::log_fn!("stake", "int_update_minebtc_staking_rewards");
    msg!("💰 Processing pending rewards before position update");
    let mut new_minebtc_rewards = 0;
    let mut new_sol_rewards = 0;
    msg!(
        "📚 [update_minebtc_rewards] hashpower={} sol_index={} sol_debt={} minebtc_index={} minebtc_debt={} pending_sol_before={} pending_staking_minebtc_before={}",
        player_data.degenbtc_hashpower as f64 / 1e6,
        faction_state.degenbtc_sol_reward_index,
        player_data.degenbtc_sol_reward_debt,
        faction_state.degenbtc_degenbtc_reward_index,
        player_data.degenbtc_degenbtc_reward_debt,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_staking_minebtc_rewards as f64 / 1e6
    );

    if player_data.degenbtc_hashpower > 0 {
        // Calculate SOL rewards using helper function (convert u128 indexes to u64 for calculation)
        new_sol_rewards = helper::calculate_staking_rewards(
            player_data.degenbtc_hashpower,
            faction_state.degenbtc_sol_reward_index,
            player_data.degenbtc_sol_reward_debt,
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
            player_data.degenbtc_hashpower,
            faction_state.degenbtc_degenbtc_reward_index,
            player_data.degenbtc_degenbtc_reward_debt,
        )?;
        player_data.pending_staking_minebtc_rewards = player_data
            .pending_staking_minebtc_rewards
            .checked_add(new_minebtc_rewards)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "   Updated pending staking MineBtc rewards: {} (+{})",
            player_data.pending_staking_minebtc_rewards as f64 / 1e6,
            new_minebtc_rewards as f64 / 1e6
        );
    } else {
        msg!("ℹ️ [update_minebtc_rewards] no degenbtc hashpower; only syncing reward debt");
    }

    // Update reward debt to current indexes
    player_data.degenbtc_sol_reward_debt = faction_state.degenbtc_sol_reward_index;
    player_data.degenbtc_degenbtc_reward_debt = faction_state.degenbtc_degenbtc_reward_index;
    msg!(
        "📚 [update_minebtc_rewards] debt_after_sync sol_debt={} minebtc_debt={} pending_staking_minebtc={}",
        player_data.degenbtc_sol_reward_debt,
        player_data.degenbtc_degenbtc_reward_debt,
        player_data.pending_staking_minebtc_rewards as f64 / 1e6
    );

    Ok((new_sol_rewards, new_minebtc_rewards))
}

pub fn int_update_lp_staking_rewards(
    player_data: &mut PlayerData,
    faction_state: &FactionState,
) -> Result<(u64, u64)> {
    crate::log_fn!("stake", "int_update_lp_staking_rewards");
    msg!("💰 Processing pending rewards before position update");
    let mut new_minebtc_rewards = 0;
    let mut new_sol_rewards = 0;
    msg!(
        "📚 [update_lp_rewards] hashpower={} sol_index={} sol_debt={} minebtc_index={} minebtc_debt={} pending_sol_before={} pending_staking_minebtc_before={}",
        player_data.lp_hashpower as f64 / 1e6,
        faction_state.lp_sol_reward_index,
        player_data.lp_sol_reward_debt,
        faction_state.lp_degenbtc_reward_index,
        player_data.lp_degenbtc_reward_debt,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_staking_minebtc_rewards as f64 / 1e6
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
            faction_state.lp_degenbtc_reward_index,
            player_data.lp_degenbtc_reward_debt,
        )?;
        player_data.pending_staking_minebtc_rewards = player_data
            .pending_staking_minebtc_rewards
            .checked_add(new_minebtc_rewards)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "   Updated pending staking MineBtc rewards: {} (+{})",
            player_data.pending_staking_minebtc_rewards as f64 / 1e6,
            new_minebtc_rewards as f64 / 1e6
        );
    } else {
        msg!("ℹ️ [update_lp_rewards] no lp hashpower; only syncing reward debt");
    }

    // Update reward debt to current indexes (MUST be outside if block to prevent
    // phantom rewards when user unstakes all LP and re-stakes later)
    player_data.lp_sol_reward_debt = faction_state.lp_sol_reward_index;
    player_data.lp_degenbtc_reward_debt = faction_state.lp_degenbtc_reward_index;
    msg!(
        "📚 [update_lp_rewards] debt_after_sync sol_debt={} minebtc_debt={} pending_staking_minebtc={}",
        player_data.lp_sol_reward_debt,
        player_data.lp_degenbtc_reward_debt,
        player_data.pending_staking_minebtc_rewards as f64 / 1e6
    );

    Ok((new_sol_rewards, new_minebtc_rewards))
}

// ----------------------------------------------------------------------------------------
// ------------ ACCOUNT STRUCTS ----------------------------------------------------------
// ----------------------------------------------------------------------------------------

// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --------- STAKE DEGENBTC ---------
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

#[derive(Accounts)]
#[instruction(amount: u64, lockup_duration: u64, position_index: u8)]
pub struct StakeMineBtc<'info> {
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
    pub player_data: Box<Account<'info, PlayerData>>,

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
    pub user_position: Box<Account<'info, StakedPosition>>,

    /// CHECK: MINE_BTC Mint (validated manually)
    pub minebtc_mint: Box<InterfaceAccount<'info, Mint2022>>,

    // Token accounts
    #[account(
        mut,
        constraint = user_minebtc_account.mint == minebtc_mint.key() @ ErrorCode::InvalidParameters,
        constraint = user_minebtc_account.owner == authority.key() @ ErrorCode::InvalidOwner,
        constraint = user_minebtc_account.amount >= amount @ ErrorCode::InsufficientFunds,
    )]
    /// User's MineBtc token account
    pub user_minebtc_account: Box<InterfaceAccount<'info, TokenAccount2022>>,

    #[account(
        mut,
        seeds = [MINEBTC_CUSTODIAN_SEED.as_ref()],
        bump,
        constraint = minebtc_custodian.mint == minebtc_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// Token-2022 account that holds staked MINE_BTC for this faction
    pub minebtc_custodian: Box<InterfaceAccount<'info, TokenAccount2022>>,

    /// User who is staking tokens
    #[account(mut)]
    pub authority: Signer<'info>,

    /// System program for creating accounts
    pub system_program: Program<'info, System>,

    /// Token-2022 program for SPL-22 token operations
    pub token_program: Program<'info, Token2022>,
}

// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --------- UNSTAKE DEGENBTC ---------
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

#[derive(Accounts)]
#[instruction(position_index: u8)]
pub struct UnstakeMineBtc<'info> {
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
        bump,
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

    /// User who is unstaking tokens
    #[account(mut)]
    pub authority: Signer<'info>,

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
        bump,
        constraint = user_position.faction_id == player_data.faction_id @ ErrorCode::InvalidFactionId,
        constraint = user_position.position_type == 1 @ ErrorCode::InvalidParameters
    )]
    pub user_position: Box<Account<'info, StakedPosition>>,

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

    /// Token program for SPL token operations
    pub token_program: Program<'info, Token>,
}

// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --------- CLAIM STAKING REWARDS (SOL + staking MineBTC transfer) ---------
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

    /// CHECK: SOL rewards vault (System Account)
    #[account(
        mut,
        seeds = [STAKER_SOL_REWARD_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_rewards_vault: UncheckedAccount<'info>,

    /// CHECK: MINE_BTC Mint (validated by token-account constraints)
    pub minebtc_mint: InterfaceAccount<'info, Mint2022>,

    #[account(
        mut,
        constraint = user_minebtc_account.mint == minebtc_mint.key() @ ErrorCode::InvalidParameters,
        constraint = user_minebtc_account.owner == authority.key() @ ErrorCode::InvalidOwner,
    )]
    /// User's MineBtc token account to receive staking rewards
    pub user_minebtc_account: InterfaceAccount<'info, TokenAccount2022>,

    #[account(
        mut,
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump
    )]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

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

    /// User claiming rewards (must be player_data.owner)
    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,

    /// Token-2022 program for SPL-22 token operations
    pub token_program: Program<'info, Token2022>,
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
        seeds = [HODL_POOL_SEED.as_ref()],
        bump
    )]
    pub hodl_pool: Account<'info, HodlPool>,

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

    /// Referrer claiming rewards
    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
