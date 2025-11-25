use anchor_lang::prelude::*;
use anchor_lang::system_program::System;
use anchor_spl::token::{self, Token};

// # Staking Instructions
//
// This module implements the staking system for MineBTC and LP tokens.
//
// ## Staking Mechanics
//
// Players can stake MineBTC or LP tokens to earn passive rewards:
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
use crate::state::*;
use crate::instructions::helper;

use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface;
use anchor_spl::token_interface::{Mint as Mint2022, TokenAccount as TokenAccount2022};

const REFERRAL_FEE_PCT: u64 = 5; // 5% referral fee

 

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- STAKE DOGEBTC TOKENS :: User gets electricity and SOL rewards ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------


/// Stake MineBtc tokens
/// Users stake MineBtc tokens to a faction and earn SOL and minebtc rewards
/// SOL rewards are distributed per round via join_round function
/// minebtc rewards are distributed per round via end_round function
pub fn stake_minebtc(
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

    let current_ts = Clock::get()?.unix_timestamp;
    
    // Store values before mutable borrow (for event emission)
    let player_data_key = ctx.accounts.player_data.key();
    
    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let user_position = &mut ctx.accounts.user_position;

    let hashpower_config = &ctx.accounts.hashpower_config;
    
    // Validate inputs
    require!(faction_state.faction_id == player_data.faction_id , ErrorCode::InvalidFactionId);
    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(lockup_duration >= hashpower_config.min_lockup_days && lockup_duration <= hashpower_config.max_lockup_days, ErrorCode::InvalidParameters);
    
    // Calculate actual amount after burn tax
    let burn_amount = amount * BURN_TAX_PERCENTAGE / M_HUNDRED;
    let actual_amount = amount - burn_amount;
    msg!("🔥 mDoge burn tax: {}% - Amount: {}, Burn: {}, Actual amount: {}", BURN_TAX_PERCENTAGE, amount as f64 / 1e6, burn_amount as f64 / 1e6, actual_amount as f64 / 1e6);  
    msg!("📊 Current faction state - Total staked: {}, Total hashpower: {}", faction_state.minebtc_staked as f64 / 1e6, faction_state.total_minebtc_hashpower as f64 / 1e6);

    // Add position index to player data
    helper::add_minebtc_position(player_data, position_index)?;
    msg!("🔍 [stake_minebtc] Position index added: {}", position_index);
    msg!("🔍 [stake_minebtc] Player data - Position indices: {:?}", player_data.minebtc_position_indices);
    msg!("🔍 [stake_minebtc] Player data - Total positions: {}", player_data.minebtc_position_indices.len());

    // Calculate multiplier based on lockup duration
    let multiplier = helper::calculate_multiplier(
        lockup_duration, 
        hashpower_config.min_lockup_days, 
        hashpower_config.max_lockup_days, 
        hashpower_config.base_multiplier, 
        hashpower_config.max_multiplier
    )?;
    msg!("🔢 Multiplier for {} days lockup: {} ({}x)", lockup_duration, multiplier, multiplier as f64 / 100.0);
    
    // Calculate weighted amount for this position
    let weighted_amount = (actual_amount * multiplier as u64) / M_HUNDRED;
    msg!("⚖️ Weighted amount: {} (actual amount: {} × multiplier: {}%)", weighted_amount as f64 / 1e6, actual_amount as f64 / 1e6, multiplier as f64 / 100.0);

    // -------------- ACCRUE PENDING REWARDS -------------- //
    
    // Process pending rewards before updating position
    let (new_sol_rewards, new_minebtc_rewards, accrued_minebtc_rewards) = update_minebtc_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;

    // -------------- UPDATE POSITION -------------- //

    // If position exists, validate and update
    if user_position.staked_amount > 0 {    
        msg!("🔄 Updating existing position - Current amount: {}", user_position.staked_amount);
        require!(user_position.lockup_end_timestamp > current_ts, ErrorCode::PositionNotLocked);
        require!( lockup_duration <= user_position.lockup_duration, ErrorCode::InvalidParameters);

        // Update staked amount with actual_amount (post-tax)
        user_position.staked_amount += actual_amount;
        user_position.weighted_amount += weighted_amount;        
        msg!("   Position updated - staked: {}, weighted: {} mDoge", user_position.staked_amount as f64 / 1e6, user_position.weighted_amount as f64 / 1e6);
    } else {
        msg!("🆕 Creating new position {}", position_index);
        // Initialize new position
        helper::init_position(
            user_position, 
            player_data.faction_id, 
            position_index, 
            actual_amount, 
            weighted_amount, 
            lockup_duration, 
            current_ts, 
            multiplier,
        )?;                
    }

    // -------------- UPDATE PLAYER AND FACTION DATA -------------- //

    let eggs_multiplier = player_data.egg_multiplier as u64;
    let weighted_amount_with_eggs = (weighted_amount * eggs_multiplier) / M_HUNDRED;
    
    // Update player data state
    player_data.minebtc_hashpower += weighted_amount_with_eggs;
    player_data.minebtc_staked += actual_amount;
    
    // Update faction state with actual_amount (post-tax) and weighted_amount
    faction_state.minebtc_staked += actual_amount;
    faction_state.total_minebtc_hashpower += weighted_amount_with_eggs;
    msg!("   Updated faction state - Total staked: {}, Total hashpower: {}",  faction_state.minebtc_staked as f64 / 1e6, faction_state.total_minebtc_hashpower as f64 / 1e6);

    // -------------- TRANSFER TOKENS -------------- //

    // Transfer tokens from user to custodian
    msg!("💱 Transferring {} mDoge tokens from user to custodian", actual_amount as f64 / 1e6);
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

    // Store faction_id before emitting event
    let faction_id = player_data.faction_id;

    emit!(MineBtcStaked {
        owner: ctx.accounts.authority.key(),
        player_data: player_data_key,
        faction_id,
        position_index,
        position_key: ctx.accounts.user_position.key(),
        lockup_duration,
        hashpower_contribution: weighted_amount_with_eggs,
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
pub fn unstake_minebtc(ctx: Context<UnstakeMineBtc>, position_index: u8) -> Result<()> {
    // Store values before mutable borrow (for event emission)
    let position_key = ctx.accounts.user_position.key();
    let player_data_key = ctx.accounts.player_data.key();
    
    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let user_position = &mut ctx.accounts.user_position;
    let current_ts = Clock::get()?.unix_timestamp;
    
    msg!("🔓 [unstake_minebtc] Processing unstake for position {}", position_index);
    
    // Validate the position exists and has funds
    require!(faction_state.faction_id == user_position.faction_id, ErrorCode::InvalidFactionId);
    require!(user_position.staked_amount > 0, ErrorCode::InvalidAmount);
    require!(user_position.position_index == position_index, ErrorCode::InvalidParameters);    
    require!( player_data.minebtc_position_indices.contains(&position_index), ErrorCode::InvalidParameters);
    require!( position_index == user_position.position_index, ErrorCode::Unauthorized);
    
    msg!(
        "📊 Position details - Staked: {}, Weighted: {}, Faction: {}, Lockup ends: {}",
        user_position.staked_amount as f64 / 1e6 , 
        user_position.weighted_amount as f64 / 1e6,
        user_position.faction_id,
        user_position.lockup_end_timestamp
    );
    
    // -------------- ACCRUE PENDING REWARDS -------------- //
    
    // Process pending rewards before updating position
    let (new_sol_rewards, new_minebtc_rewards, accrued_minebtc_rewards) = update_minebtc_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;
    
    // -------------- UPDATE FACTION AND PLAYER DATA -------------- //

    // Calculate return amount based on early withdrawal status
    let is_early_withdrawal = current_ts < user_position.lockup_end_timestamp;
    let staked_amount = user_position.staked_amount;
    let original_weighted = user_position.weighted_amount;
    let hashpower_contribution = (original_weighted * player_data.egg_multiplier as u64) / M_HUNDRED;
    let mut return_amount = staked_amount;
    let mut penalty_amount = 0u64;
        
    // Update faction state (decrease staked amount and hashpower)
    msg!("📊 Updating faction state");
    faction_state.minebtc_staked -= staked_amount;
    faction_state.total_minebtc_hashpower -= hashpower_contribution;
    msg!("   New faction totals - Staked: {}, Hashpower: {}", faction_state.minebtc_staked as f64 / 1e6, faction_state.total_minebtc_hashpower as f64 / 1e6);
    
    // Update player data (decrease hashpower and staked amount)
    msg!("📊 Updating player data");
    player_data.minebtc_hashpower -= hashpower_contribution;
    player_data.minebtc_staked -= staked_amount;
    msg!("   New player totals - Hashpower: {}, Staked: {}", player_data.minebtc_hashpower as f64 / 1e6, player_data.minebtc_staked as f64 / 1e6);
    
    // Remove position from user's active positions
    helper::remove_minebtc_position(player_data, position_index)?;

    // -------------- CHARGE EMERGENCY TAX -------------- //

    // Handle early withdrawal if needed
    if is_early_withdrawal {
        msg!(  "⚠️ Early unstake detected! Current time: {}, Lockup end: {}", current_ts, user_position.lockup_end_timestamp);
        
        // Calculate remaining lockup percentage        
        penalty_amount = helper::calculate_emergency_tax(user_position, current_ts, EMERGENCY_WITHDRAWAL_PENALTY_PCT as u64);
        return_amount = staked_amount - penalty_amount;
        msg!( "   Total Staked: {}, Returned: {}, Penalty: {}", staked_amount, return_amount, penalty_amount);
        
        // Charge emergency tax if any penalty
        if penalty_amount > 0 {
            // Charge emergency tax: 50% to burn, 50% to MINEBTC vault  
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
    
    // Reset position data
    user_position.staked_amount = 0;
    user_position.weighted_amount = 0;
    
    // Store values for emergency withdrawal event before resetting position
    let total_lockup_seconds = user_position.lockup_end_timestamp - user_position.start_timestamp;
    let remaining_seconds = user_position.lockup_end_timestamp - current_ts;
    let mut remaining_seconds_pct = 0u64;
    if total_lockup_seconds > 0 {
        remaining_seconds_pct = (M_HUNDRED as i64 * remaining_seconds / total_lockup_seconds) as u64;
    }
    let calc_penalty_pct = if is_early_withdrawal && penalty_amount > 0 {
        (EMERGENCY_WITHDRAWAL_PENALTY_PCT as u64 * remaining_seconds_pct) / M_HUNDRED
    } else {
        0
    };
    
    // Emit events
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
    
    msg!("✅ [unstake_minebtc] Unstake completed successfully");
    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- STAKE LIQUIDITY LP TOKENS :: User gets SOL and minebtc rewards ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Stake LP tokens
/// Users stake LP tokens to a faction and earn SOL and minebtc rewards
/// SOL rewards are distributed per round via join_round function
/// minebtc rewards are distributed per round via end_round function
pub fn stake_lp_tokens(
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

    // Validate inputs
    require!(faction_state.faction_id == player_data.faction_id, ErrorCode::InvalidFactionId);
    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(lockup_duration >= hashpower_config.min_lockup_days && lockup_duration <= hashpower_config.max_lockup_days, ErrorCode::InvalidParameters);
    
    // Calculate actual amount after burn tax
    let actual_amount = amount;
    msg!(" Current faction state - Total LP staked: {}, Total LP hashpower: {}", faction_state.total_lp_hashpower, faction_state.total_lp_hashpower);
    
    // Add position index to player data
    helper::add_lp_position(player_data, position_index)?;

    // Calculate multiplier based on lockup duration
    let multiplier = helper::calculate_multiplier(
        lockup_duration, 
        hashpower_config.min_lockup_days, 
        hashpower_config.max_lockup_days, 
        hashpower_config.base_multiplier, 
        hashpower_config.max_multiplier
    )?;
    msg!("🔢 Multiplier for {} days lockup: {} ({}x)", lockup_duration, multiplier, multiplier as f64 / 100.0);
    
    // Calculate weighted amount for this position
    let weighted_amount = actual_amount * multiplier as u64 / M_HUNDRED;
    msg!("⚖️ Weighted amount: {} (amount: {} × multiplier: {}%)",  weighted_amount as f64 / 1e6, actual_amount as f64 / 1e6, multiplier as f64 / 100.0);
    
    // -------------- ACCRUE PENDING REWARDS -------------- //
    
    // Process pending rewards before updating position
    let (new_sol_rewards, new_minebtc_rewards, accrued_minebtc_rewards) = update_lp_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;

    // -------------- UPDATE POSITION -------------- //

    // If position exists, validate and update
    if user_position.staked_amount > 0 {    
        msg!("🔄 Updating existing position - Current amount: {}", user_position.staked_amount as f64 / 1e6);
        require!(user_position.lockup_end_timestamp > current_ts, ErrorCode::PositionNotLocked);
        require!( lockup_duration <= user_position.lockup_duration, ErrorCode::InvalidParameters);

        // Update staked amount with actual_amount (post-tax)
        user_position.staked_amount += actual_amount;
        user_position.weighted_amount += weighted_amount;        
        msg!("   Position updated - staked: {}, weighted: {} mDoge", user_position.staked_amount as f64 / 1e6, user_position.weighted_amount as f64 / 1e6);
    } else {
        msg!("🆕 Creating new position {}", position_index);
        // Initialize new position
        helper::init_position(
            user_position, 
            player_data.faction_id, 
            position_index, 
            actual_amount, 
            weighted_amount, 
            lockup_duration, 
            current_ts, 
            multiplier,
        )?;                
    }

    // -------------- UPDATE PLAYER AND FACTION DATA -------------- //
    
    let eggs_multiplier = player_data.egg_multiplier as u64;
    let weighted_amount_with_eggs = (weighted_amount * eggs_multiplier) / M_HUNDRED;

    // Update player data state
    player_data.lp_hashpower += weighted_amount_with_eggs;
    player_data.lp_staked += actual_amount;

    // Update faction state with actual_amount (post-tax) and weighted_amount
    faction_state.lp_staked += actual_amount;
    faction_state.total_lp_hashpower += weighted_amount_with_eggs;
    msg!("   Updated faction state - Total staked: {}, Total hashpower: {}",  faction_state.lp_staked as f64 / 1e6, faction_state.total_lp_hashpower as f64 / 1e6);

    // -------------- TRANSFER TOKENS -------------- //

    msg!("💱 Transferring {} LP tokens from user to custodian", actual_amount as f64 / 1e6);

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
    
    // Store faction_id before emitting event
    let faction_id = player_data.faction_id;
    
    emit!(LiquidityStaked {
        owner: ctx.accounts.authority.key(),
        player_data: player_data_key,
        position_index,
        position_key: ctx.accounts.user_position.key(),
        faction_id,
        lockup_duration,
        hashpower_contribution: weighted_amount_with_eggs,
        new_sol_rewards,
        new_minebtc_rewards,
        unrefined_minebtc: accrued_minebtc_rewards,
        timestamp: current_ts,
    });
    
    Ok(())
}

/// Unstake LP tokens from a position
pub fn unstake_lp_tokens(ctx: Context<UnstakeLpTokens>, position_index: u8) -> Result<()> {
    // Store values before mutable borrow (for event emission)
    let player_data_key = ctx.accounts.player_data.key();
    let position_key = ctx.accounts.user_position.key();
    
    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let user_position = &mut ctx.accounts.user_position;
    let current_ts = Clock::get()?.unix_timestamp;
    
    msg!("🔓 [unstake_minebtc] Processing unstake for position {}", position_index);
    
    // Validate the position exists and has funds
    require!(faction_state.faction_id == user_position.faction_id, ErrorCode::InvalidFactionId);
    require!(user_position.staked_amount > 0, ErrorCode::InvalidAmount);
    require!(user_position.position_index == position_index, ErrorCode::InvalidParameters);    
    require!( player_data.lp_position_indices.contains(&position_index), ErrorCode::InvalidParameters);
    
    msg!(
        "📊 Position details - Staked: {}, Weighted: {}, Faction: {}, Lockup ends: {}",
        user_position.staked_amount as f64 / 1e6 , 
        user_position.weighted_amount as f64 / 1e6,
        user_position.faction_id,
        user_position.lockup_end_timestamp
    );
    
    // -------------- ACCRUE PENDING REWARDS -------------- //
    
    // Process pending rewards before updating position
    let (new_sol_rewards, new_minebtc_rewards, accrued_minebtc_rewards) = update_lp_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;

    // -------------- UPDATE FACTION AND PLAYER DATA -------------- //

    // Calculate return amount based on early withdrawal status
    let is_early_withdrawal = current_ts < user_position.lockup_end_timestamp;
    let staked_amount = user_position.staked_amount;
    let original_weighted = user_position.weighted_amount;
    let hashpower_contribution = (original_weighted * player_data.egg_multiplier as u64) / M_HUNDRED;
    let mut return_amount = staked_amount;
    let mut penalty_amount = 0u64;
        
    // Update faction state (decrease staked amount and hashpower)
    msg!("📊 Updating faction state");
    faction_state.lp_staked -= staked_amount;
    faction_state.total_lp_hashpower -= hashpower_contribution;
    msg!("   New faction totals - Staked: {}, Hashpower: {}", faction_state.lp_staked as f64 / 1e6, faction_state.total_lp_hashpower as f64 / 1e6);
    
    // Update player data (decrease hashpower and staked amount)
    msg!("📊 Updating player data");
    player_data.lp_hashpower -= hashpower_contribution;
    player_data.lp_staked -= staked_amount;
    msg!("   New player totals - Hashpower: {}, Staked: {}", player_data.lp_hashpower as f64 / 1e6, player_data.lp_staked as f64 / 1e6);
    
    // Remove position from user's active positions
    helper::remove_lp_position(player_data, position_index)?;
        

    // -------------- CHARGE EMERGENCY TAX -------------- //

    // Handle early withdrawal if needed
    if is_early_withdrawal {
        msg!(  "⚠️ Early unstake detected! Current time: {}, Lockup end: {}", current_ts, user_position.lockup_end_timestamp);
        
        // Calculate remaining lockup percentage        
        penalty_amount = helper::calculate_emergency_tax(user_position, current_ts, EMERGENCY_WITHDRAWAL_PENALTY_PCT as u64);
        return_amount = staked_amount - penalty_amount;
        msg!( "   Total Staked: {}, Returned: {}, Penalty: {}", staked_amount, return_amount, penalty_amount);
        
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
    
    // Reset position data
    user_position.staked_amount = 0;
    user_position.weighted_amount = 0;
    
    // Store values before emitting events (to avoid borrow conflicts)
    let total_lockup_seconds = user_position.lockup_end_timestamp - user_position.start_timestamp;
    let remaining_seconds = if is_early_withdrawal {
        user_position.lockup_end_timestamp - current_ts
    } else {
        0
    };
    let mut remaining_seconds_pct = 0u64;
    if total_lockup_seconds > 0 && is_early_withdrawal {
        remaining_seconds_pct = (M_HUNDRED as i64 * remaining_seconds / total_lockup_seconds) as u64;
    }
    let calc_penalty_pct = if is_early_withdrawal && penalty_amount > 0 {
        (EMERGENCY_WITHDRAWAL_PENALTY_PCT as u64 * remaining_seconds_pct) / M_HUNDRED
    } else {
        0
    };
    
    // Emit events
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
    
    msg!("✅ [unstake_minebtc] Unstake completed successfully");
    Ok(())
}


// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- CLAIM SOL REWARDS :: User earns SOL rewards from staking ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Claim SOL rewards from staking MineBtc and LP tokens
pub fn claim_sol_rewards(ctx: Context<ClaimSolRewards>) -> Result<()> {
    msg!("💰 [claim_sol_rewards] Claiming SOL rewards from staking");
    
    require!(ctx.accounts.faction_state.faction_id == ctx.accounts.player_data.faction_id, ErrorCode::InvalidFactionId);
    
    // Store values before mutable borrow (for event emission)
    let player_data_key = ctx.accounts.player_data.key();
    let faction_id = ctx.accounts.faction_state.faction_id;
    
    let faction_state = &ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    
    // Process MineBtc staking SOL rewards
    let (_st_minebtc_new_sol_rewards, _st_minebtc_new_minebtc_rewards, _st_minebtc_accrued_minebtc_rewards) = update_minebtc_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;
    // Process LP staking SOL rewards
    let (_st_lp_new_sol_rewards, _st_lp_new_minebtc_rewards, _st_lp_accrued_minebtc_rewards) = update_lp_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;
    
    let total_pending_sol_rewards = player_data.pending_sol_rewards;
    require!(total_pending_sol_rewards > 0, ErrorCode::InsufficientFunds);    
    msg!("   Total claimable SOL rewards: {} lamports", total_pending_sol_rewards as f64 / 1e9);
    msg!("   Total claimable MineBtc rewards: {} minebtc", player_data.pending_minebtc_rewards as f64 / 1e6);
    
    // Check if user has a referrer (not system referral account)
    let has_referrer = player_data.referral_code != ctx.accounts.system_program.key();

    let (referral_fee, player_sol) = if has_referrer {
        let referral_rewards = total_pending_sol_rewards * REFERRAL_FEE_PCT / 100; // 5% referral fee
        msg!("     Referral fee (5%): {} lamports", referral_rewards as f64 / 1e9);
        
        // Add fee to referrer's pending SOL rewards
        if let Some(referrer_rewards) = &mut ctx.accounts.referrer_rewards {
            referrer_rewards.pending_sol_rewards += referral_rewards;
            referrer_rewards.total_sol_earned += referral_rewards;
            msg!("     Added {} SOL to referrer's rewards", referral_rewards as f64 / 1e9);
        }
        (referral_rewards, total_pending_sol_rewards - referral_rewards)
    } else {
        (0, total_pending_sol_rewards)
    };
    
    // Transfer SOL rewards to user (after referral fee)
    msg!("   Transferring {} SOL from sol_rewards_vault to user", (player_sol as f64 / 1e9));
    helper::transfer_from_sol_rewards_vault(
        &ctx.accounts.sol_rewards_vault.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        player_sol,
        ctx.bumps.sol_rewards_vault,
    )?;
    msg!("     ✓ SOL rewards transferred to user");
        
    // Store referrer before resetting and emitting event
    let referrer_pubkey = if has_referrer { Some(player_data.referral_code) } else { None };
    
    // Reset pending rewards
    player_data.pending_sol_rewards = 0;
    
    emit!(SolRewardsClaimed {
        user: ctx.accounts.authority.key(),
        player_data: player_data_key,
        faction_id,
        sol_amount: player_sol,
        referral_fee,
        referrer: referrer_pubkey,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    msg!("✅ [claim_sol_rewards] Claimed {} SOL (fee: {} to referrer)", player_sol as f64 / 1e9, referral_fee as f64 / 1e9);
    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- CLAIM MINEBTC REWARDS :: User earns minebtc rewards from staking ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Claim MineBtc token rewards from staking MineBtc and LP tokens
/// Implements refining fee: 10% of claimed rewards are redistributed to other unclaimed stakers
/// Also increases power of all staked eggs proportionally to claimed amount
pub fn claim_minebtc_rewards(ctx: Context<ClaimDbtcRewards>) -> Result<()> {
    msg!("💰 [claim_minebtc_rewards] Claiming MineBtc token rewards with refining fee");
    
    require!(ctx.accounts.faction_state.faction_id == ctx.accounts.player_data.faction_id, ErrorCode::InvalidFactionId);
    
    // Store values before mutable borrow (for event emission)
    let player_data_key = ctx.accounts.player_data.key();
    let faction_id = ctx.accounts.faction_state.faction_id;
    
    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let unrefined_minebtc = &mut ctx.accounts.unrefined_rewards;
    let global_config = &ctx.accounts.global_config;

    // Process MineBtc staking SOL rewards
    let (_st_minebtc_new_sol_rewards, _st_minebtc_new_minebtc_rewards, _st_minebtc_accrued_minebtc_rewards) = update_minebtc_staking_rewards(player_data, unrefined_minebtc, faction_state)?;
    // Process LP staking SOL rewards
    let (_st_lp_new_sol_rewards, _st_lp_new_minebtc_rewards, _st_lp_accrued_minebtc_rewards) = update_lp_staking_rewards(player_data, unrefined_minebtc, faction_state)?;
    
    require!(player_data.pending_minebtc_rewards > 0, ErrorCode::InsufficientFunds);
    
    // Apply refining fee (10% by default, or configured in global_config)
    let refining_fee_pct = global_config.minebtc_dist_config.refining_fee as u64;
    let refining_fee = (player_data.pending_minebtc_rewards * refining_fee_pct) / M_HUNDRED;
    let claimable_amount = player_data.pending_minebtc_rewards - refining_fee;
    msg!("Refining fee: {} minebtc. Claimable amount: {} minebtc", refining_fee as f64 / 1e6, claimable_amount as f64 / 1e6);
    
    // Apply referral fee first (5% of total)
    let has_referrer = player_data.referral_code != ctx.accounts.system_program.key();
    let referral_fee = if has_referrer {
        let fee = claimable_amount * REFERRAL_FEE_PCT / 100; // 5% referral fee
        msg!("   Referral fee (5%): {} minebtc", fee);
        
        // Add fee to referrer's pending minebtc rewards
        if let Some(referrer_rewards) = &mut ctx.accounts.referrer_rewards {
            referrer_rewards.pending_minebtc_rewards += fee;
            referrer_rewards.total_minebtc_earned += fee;
            msg!("     Added {} minebtc to referrer's rewards", fee);
        }
        fee
    } else {
        0
    };
    
    let claimable_by_user = claimable_amount - referral_fee;
    msg!("Claimable by user: {} minebtc", claimable_by_user as f64 / 1e6);
    
    // Accumulate power points (1 power per 1000 minebtc claimed, minimum 1 power)

    let power_points = calculate_power_points(claimable_by_user);
    player_data.claimable_power += power_points;
    msg!("   Accumulated {} power points (total claimable: {})", power_points, player_data.claimable_power);
    
    // Transfer claimable MineBtc to user
    if claimable_by_user > 0 {
        msg!("💱 Transferring {} MineBtc tokens to user", claimable_amount as f64 / 1e6);
        
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

    // update total claimable minebtc amount 
    unrefined_minebtc.total_minebtc_claimable = unrefined_minebtc.total_minebtc_claimable - player_data.pending_minebtc_rewards;
    player_data.pending_minebtc_rewards = 0;

    // Update total tokens distributed
    let mine_btc_mining = &mut ctx.accounts.mine_btc_mining;
    mine_btc_mining.total_tokens_distributed += claimable_by_user;
    msg!("   Updated total tokens distributed: {} (+{})", mine_btc_mining.total_tokens_distributed as f64 / 1e6, claimable_by_user as f64 / 1e6);
    
    // Store referrer before emitting event
    let referrer_pubkey = if has_referrer { Some(player_data.referral_code) } else { None };
    
    // Redistribute refining fee to all other stakers who haven't claimed
    // This is done by increasing the reward index, which benefits all stakers proportionally
    if refining_fee > 0 {
        msg!("   Redistributing refining fee to other stakers...");
        if unrefined_minebtc.total_minebtc_claimable > 0 {
            let increment = helper::mul_div(refining_fee, INDEX_PRECISION, unrefined_minebtc.total_minebtc_claimable)?;
            unrefined_minebtc.unrefining_index += increment;
            msg!("   Updated unrefining index: {} (+{})", unrefined_minebtc.unrefining_index, increment);        
        } else {
            msg!("   No other stakers to redistribute to. Fee remains in unrefined rewards pool.");
        }
    }
    
    emit!(DbtcRewardsClaimed {
        user: ctx.accounts.authority.key(),
        player_data: player_data_key,
        faction_id,
        minebtc_amount: claimable_by_user,
        refining_fee,
        referral_fee,
        power_points_earned: power_points,
        referrer: referrer_pubkey,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- CLAIM REFERRAL REWARDS :: Referrers claim their earned rewards ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Claim referral rewards (SOL and MineBtc)
pub fn claim_referral_rewards(ctx: Context<ClaimReferralRewards>) -> Result<()> {
    msg!("💰 [claim_referral_rewards] Claiming referral rewards");
    
    let referral_rewards = &mut ctx.accounts.referral_rewards;
    
    let pending_sol = referral_rewards.pending_sol_rewards;
    let pending_minebtc = referral_rewards.pending_minebtc_rewards;
    
    require!(pending_sol > 0 || pending_minebtc > 0, ErrorCode::InsufficientFunds);
    
    msg!("   Pending SOL: {} lamports. Pending MineBtc: {} minebtc", pending_sol as f64 / 1e9, pending_minebtc as f64 / 1e6);
    
    // Transfer SOL if any
    if pending_sol > 0 {
        msg!("   Transferring {} SOL from sol_rewards_vault to referrer", (pending_sol as f64 / 1e9));
        helper::transfer_from_sol_rewards_vault(
            &ctx.accounts.sol_rewards_vault.to_account_info(),
            &ctx.accounts.authority.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            pending_sol,
            ctx.bumps.sol_rewards_vault,
        )?;
        msg!("     ✓ Transferred {} SOL", pending_sol as f64 / 1e9);
    }
    
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
    mine_btc_mining.total_tokens_distributed += pending_minebtc;
    msg!("   Updated total tokens distributed: {} (+{})", mine_btc_mining.total_tokens_distributed as f64 / 1e6, pending_minebtc as f64 / 1e6);
    
    // Reset pending rewards
    referral_rewards.pending_sol_rewards = 0;
    referral_rewards.pending_minebtc_rewards = 0;
    
    emit!(ReferralRewardsClaimed {
        referrer: ctx.accounts.authority.key(),
        referral_rewards_account: ctx.accounts.referral_rewards.key(),
        sol_amount: pending_sol,
        minebtc_amount: pending_minebtc,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    msg!("✅ [claim_referral_rewards] Claimed referral rewards successfully");
    Ok(())
}

// ----------------------------------------------------------------------------------------
// -------------- HELPER FUNCTIONS ---------------------------------------------------------
// ----------------------------------------------------------------------------------------


pub fn update_minebtc_staking_rewards( player_data: &mut PlayerData, unrefined_rewards: &mut UnrefinedRewards, faction_state: &FactionState) -> Result<(u64, u64, u64)> {
    msg!("💰 Processing pending rewards before position update");
    let mut new_minebtc_rewards = 0;
    let mut new_sol_rewards = 0;
    let mut accrued_minebtc_rewards = 0;
        
    if player_data.minebtc_hashpower > 0 {
                
        // Calculate SOL rewards using helper function (convert u128 indexes to u64 for calculation)
        new_sol_rewards = helper::calculate_staking_rewards( player_data.minebtc_hashpower,faction_state.minebtc_sol_reward_index, player_data.minebtc_sol_reward_debt)?;
        player_data.pending_sol_rewards += new_sol_rewards;
        msg!("   Updated pending SOL rewards: {} (+{})", player_data.pending_sol_rewards as f64 / 1e9, new_sol_rewards as f64 / 1e9);

        new_minebtc_rewards = helper::calculate_staking_rewards( player_data.minebtc_hashpower,faction_state.minebtc_minebtc_reward_index, player_data.minebtc_minebtc_reward_debt)?;
        accrued_minebtc_rewards = helper::add_to_total_claimable(unrefined_rewards, player_data, new_minebtc_rewards);
        msg!("   Updated pending MineBtc rewards: {} (+{})", player_data.pending_minebtc_rewards as f64 / 1e6, new_minebtc_rewards as f64 / 1e6);
    }

    // Update reward debt to current indexes
    player_data.minebtc_sol_reward_debt = faction_state.minebtc_sol_reward_index;
    player_data.minebtc_minebtc_reward_debt = faction_state.minebtc_minebtc_reward_index;
    
    Ok((new_sol_rewards, new_minebtc_rewards, accrued_minebtc_rewards))
}



pub fn update_lp_staking_rewards( player_data: &mut PlayerData, unrefined_rewards: &mut UnrefinedRewards, faction_state: &FactionState) -> Result<(u64, u64, u64)> {
    msg!("💰 Processing pending rewards before position update");
    let mut new_minebtc_rewards = 0;
    let mut new_sol_rewards = 0;
    let mut accrued_minebtc_rewards = 0;

    if player_data.lp_hashpower > 0 {                    
        // Calculate SOL rewards using helper function (convert u128 indexes to u64 for calculation)
        new_sol_rewards = helper::calculate_staking_rewards( player_data.lp_hashpower,faction_state.lp_sol_reward_index, player_data.lp_sol_reward_debt)?;
        player_data.pending_sol_rewards += new_sol_rewards;
        msg!("   Updated pending SOL rewards: {} (+{})", player_data.pending_sol_rewards as f64 / 1e9, new_sol_rewards as f64 / 1e9);

        new_minebtc_rewards = helper::calculate_staking_rewards( player_data.lp_hashpower,faction_state.lp_minebtc_reward_index, player_data.lp_minebtc_reward_debt)?;
        accrued_minebtc_rewards = helper::add_to_total_claimable(unrefined_rewards, player_data, new_minebtc_rewards);
        msg!("   Updated pending MineBtc rewards: {} (+{})", player_data.pending_minebtc_rewards as f64 / 1e6, new_minebtc_rewards as f64 / 1e6);

        // Update reward debt to current indexes
        player_data.lp_sol_reward_debt = faction_state.lp_sol_reward_index;
        player_data.lp_minebtc_reward_debt = faction_state.lp_minebtc_reward_index;
    }

    Ok((new_sol_rewards, new_minebtc_rewards, accrued_minebtc_rewards))
}

fn calculate_power_points(claimable_by_user: u64) -> u64 {
    let power_points = if claimable_by_user > 0 {
        let power = claimable_by_user / 10_000; // 100 power per 1 minebtc (with 6 decimals)
        if power == 0 && claimable_by_user > 0 {
            1 // Minimum 1 power if any minebtc claimed
        } else {
            power
        }
    } else {
        0
    };
    return power_points;
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
    // Global config
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    // Hashpower config (contains lockup and multiplier settings)
    #[account(
        seeds = [HASHPOWER_CONFIG_SEED.as_ref()],
        bump
    )]
    pub hashpower_config: Account<'info, HashpowerConfig>,
    
    // Faction state
    #[account(
        mut,
    )]
    pub faction_state: Account<'info, FactionState>,
    
    // Player data
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
    pub global_config: Account<'info, GlobalConfig>,
    
    // Faction state
    #[account(
        mut,
    )]
    pub faction_state: Box<Account<'info, FactionState>>,
    
    // Player data
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Box<Account<'info, PlayerData>>,
    
    // Staked position
    #[account(
        mut,
    )]
    pub user_position: Box<Account<'info, StakedPosition>>,
    
    /// CHECK: MINE_BTC Mint - must be mut for burn instruction during emergency withdrawal
    #[account(mut)]
    pub minebtc_mint: InterfaceAccount<'info, Mint2022>,

    // Token accounts
    #[account(
        mut,
        constraint = user_minebtc_account.owner == authority.key() @ ErrorCode::InvalidOwner,
        constraint = user_minebtc_account.mint == minebtc_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// User's MineBtc token account to receive the unstaked tokens
    pub user_minebtc_account: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        mut,
        seeds = [MINEBTC_CUSTODIAN_SEED.as_ref()],
        bump,
        constraint = minebtc_custodian.mint == minebtc_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// Token-2022 account that holds staked MINE_BTC (global for all factions)
    pub minebtc_custodian: InterfaceAccount<'info, TokenAccount2022>,
    
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
    pub unrefined_rewards: Account<'info, UnrefinedRewards>,
                
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
    #[account(
        mut,
    )]
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
    pub global_config: Account<'info, GlobalConfig>,
    
    // Faction state
    #[account(
        mut,
    )]
    pub faction_state: Box<Account<'info, FactionState>>,
    
    // Player data
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Box<Account<'info, PlayerData>>,
    
    // Staked position
    #[account(
        mut,
    )]
    pub user_position: Box<Account<'info, StakedPosition>>,
    
    #[account(
        mut,
        seeds = [UNREFINED_REWARDS_SEED.as_ref()],
        bump
    )]
    pub unrefined_rewards: Account<'info, UnrefinedRewards>,

    /// CHECK: LP Mint - must be mut for burn instruction during emergency withdrawal
    #[account(mut)]
    pub lp_mint: Account<'info, token::Mint>,
    
    // Token accounts
    #[account(
        mut,
        constraint = user_lp_account.owner == authority.key() @ ErrorCode::InvalidOwner,
        constraint = user_lp_account.mint == lp_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// User's LP token account to receive the unstaked tokens
    pub user_lp_account: Account<'info, token::TokenAccount>,
    
    #[account(
        mut,
        seeds = [LIQUIDITY_CUSTODIAN_SEED.as_ref()],
        bump,
        constraint = liquidity_custodian.mint == lp_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// Token account that holds staked LP tokens for this faction
    pub liquidity_custodian: Account<'info, token::TokenAccount>,
    
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
// --------- CLAIM SOL REWARDS ---------
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

#[derive(Accounts)]
pub struct ClaimSolRewards<'info> {
    // Faction state
    #[account()]
    pub faction_state: Account<'info, FactionState>,
    
    // Player data
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,
    
    /// Optional referrer rewards account (if player has a referrer)
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
    
    /// CHECK: SOL rewards vault (System Account)
    #[account(
        mut,
        seeds = [STAKER_SOL_REWARD_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_rewards_vault: UncheckedAccount<'info>,
    
    /// User claiming rewards
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --------- CLAIM MINEBTC REWARDS ---------
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

#[derive(Accounts)]
pub struct ClaimDbtcRewards<'info> {
    // Global config
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    // Faction state
    #[account(
        mut,
    )]
    pub faction_state: Account<'info, FactionState>,
    
    // Player data
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,
    
    /// Optional referrer rewards account (if player has a referrer)
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
    
    /// CHECK: SOL rewards vault (System Account)
    #[account(
        mut,
        seeds = [STAKER_SOL_REWARD_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_rewards_vault: UncheckedAccount<'info>,
    
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

