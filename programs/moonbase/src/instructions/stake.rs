use anchor_lang::prelude::*;
use anchor_lang::system_program::System;
use anchor_spl::token::{self, Token};

use crate::errors::ErrorCode;
use crate::events::*;
use crate::state::*;
use crate::instructions::helper;

use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface;
use anchor_spl::token_interface::{Mint as Mint2022, TokenAccount as TokenAccount2022};

const REFERRAL_FEE_PCT: u64 = 5; // 5% referral fee

 

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- STAKE MOONDOGE TOKENS :: User gets electricity and SOL rewards ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------


/// Stake DogeBtc tokens
/// Users stake DogeBtc tokens to a faction and earn SOL and dbtc rewards
/// SOL rewards are distributed per round via join_round function
/// dbtc rewards are distributed per round via end_round function
pub fn stake_moondoge(
    ctx: Context<StakeDogeBtc>,
    amount: u64,
    lockup_duration: u64,
    position_index: u8,
) -> Result<()> {
    msg!(
        "🔒 [stake_moondoge] Starting DogeBtc staking - Amount: {}, Lockup: {} days, Position: {}",
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
    msg!("📊 Current faction state - Total staked: {}, Total hashpower: {}", faction_state.dbtc_staked as f64 / 1e6, faction_state.total_dbtc_hashpower as f64 / 1e6);

    // Add position index to player data
    helper::add_dogebtc_position(player_data, position_index)?;
    msg!("🔍 [stake_moondoge] Position index added: {}", position_index);
    msg!("🔍 [stake_moondoge] Player data - Position indices: {:?}", player_data.moondoge_position_indices);
    msg!("🔍 [stake_moondoge] Player data - Total positions: {}", player_data.moondoge_position_indices.len());

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
    let (new_sol_rewards, new_dbtc_rewards, accrued_dbtc_rewards) = update_dbtc_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;

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
    player_data.dogebtc_hashpower += weighted_amount_with_eggs;
    player_data.dogebtc_staked += actual_amount;
    
    // Update faction state with actual_amount (post-tax) and weighted_amount
    faction_state.dbtc_staked += actual_amount;
    faction_state.total_dbtc_hashpower += weighted_amount_with_eggs;
    msg!("   Updated faction state - Total staked: {}, Total hashpower: {}",  faction_state.dbtc_staked as f64 / 1e6, faction_state.total_dbtc_hashpower as f64 / 1e6);

    // -------------- TRANSFER TOKENS -------------- //

    // Transfer tokens from user to custodian
    msg!("💱 Transferring {} mDoge tokens from user to custodian", actual_amount as f64 / 1e6);
    let transfer_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        token_interface::TransferChecked {
            from: ctx.accounts.user_dbtc_account.to_account_info(),
            to: ctx.accounts.dbtc_custodian.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
            mint: ctx.accounts.dbtc_mint.to_account_info(),
        },
    );
    token_interface::transfer_checked(transfer_ctx, amount, ctx.accounts.dbtc_mint.decimals)?;
    msg!("✅ [stake_moondoge] DogeBtc staking successful");    

    // Store faction_id before emitting event
    let faction_id = player_data.faction_id;

    emit!(DogeBtcStaked {
        owner: ctx.accounts.authority.key(),
        player_data: player_data_key,
        faction_id,
        position_index,
        position_key: ctx.accounts.user_position.key(),
        lockup_duration,
        hashpower_contribution: weighted_amount_with_eggs,
        new_sol_rewards,
        new_dbtc_rewards,
        unrefined_dbtc: accrued_dbtc_rewards,
        timestamp: current_ts,
    });
    
    Ok(())
}



// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- UNSTAKE MOONDOGE TOKENS :: User gets DOGE_BTC back ------------------------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Unstake DogeBtc tokens from a position
pub fn unstake_moondoge(ctx: Context<UnstakeDogeBtc>, position_index: u8) -> Result<()> {
    // Store values before mutable borrow (for event emission)
    let position_key = ctx.accounts.user_position.key();
    let player_data_key = ctx.accounts.player_data.key();
    
    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let user_position = &mut ctx.accounts.user_position;
    let current_ts = Clock::get()?.unix_timestamp;
    
    msg!("🔓 [unstake_moondoge] Processing unstake for position {}", position_index);
    
    // Validate the position exists and has funds
    require!(faction_state.faction_id == user_position.faction_id, ErrorCode::InvalidFactionId);
    require!(user_position.staked_amount > 0, ErrorCode::InvalidAmount);
    require!(user_position.position_index == position_index, ErrorCode::InvalidParameters);    
    require!( player_data.moondoge_position_indices.contains(&position_index), ErrorCode::InvalidParameters);
    
    msg!(
        "📊 Position details - Staked: {}, Weighted: {}, Faction: {}, Lockup ends: {}",
        user_position.staked_amount as f64 / 1e6 , 
        user_position.weighted_amount as f64 / 1e6,
        user_position.faction_id,
        user_position.lockup_end_timestamp
    );
    
    // -------------- ACCRUE PENDING REWARDS -------------- //
    
    // Process pending rewards before updating position
    let (new_sol_rewards, new_dbtc_rewards, accrued_dbtc_rewards) = update_dbtc_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;
    
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
    faction_state.dbtc_staked -= staked_amount;
    faction_state.total_dbtc_hashpower -= hashpower_contribution;
    msg!("   New faction totals - Staked: {}, Hashpower: {}", faction_state.dbtc_staked as f64 / 1e6, faction_state.total_dbtc_hashpower as f64 / 1e6);
    
    // Update player data (decrease hashpower and staked amount)
    msg!("📊 Updating player data");
    player_data.dogebtc_hashpower -= hashpower_contribution;
    player_data.dogebtc_staked -= staked_amount;
    msg!("   New player totals - Hashpower: {}, Staked: {}", player_data.dogebtc_hashpower as f64 / 1e6, player_data.dogebtc_staked as f64 / 1e6);
    
    // Remove position from user's active positions
    helper::remove_moondoge_position(player_data, position_index)?;

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
            // Charge emergency tax: 50% to burn, 50% to DBTC vault  
            helper::charge_emergency_tax(
                &ctx.accounts.dbtc_custodian.to_account_info(),
                &ctx.accounts.dbtc_custodian_authority.to_account_info(),
                &ctx.accounts.dbtc_mint.to_account_info(),
                &ctx.accounts.token_program.to_account_info(),
                ctx.bumps.dbtc_custodian_authority,
                penalty_amount,
            )?;
        }
    } else {
        msg!("✅ Normal unstake - lockup period has ended");
    }
       
    // -------------- TRANSFER TOKENS -------------- //

    // Transfer remaining tokens back to user
    if return_amount > 0 {
        msg!("💱 Transferring {} DOGE_BTC tokens to user", return_amount);
        
        // Get PDA signer seeds for the dbtc_custodian authority (global, no faction_id)
        let custodian_authority_seeds = &[
            DBTC_CUSTODIAN_AUTHORITY_SEED.as_ref(),
            &[ctx.bumps.dbtc_custodian_authority],
        ];
        let signer = &[&custodian_authority_seeds[..]];
        
        // Transfer tokens back to user
        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token_interface::TransferChecked {
                from: ctx.accounts.dbtc_custodian.to_account_info(),
                to: ctx.accounts.user_dbtc_account.to_account_info(),
                authority: ctx.accounts.dbtc_custodian_authority.to_account_info(),
                mint: ctx.accounts.dbtc_mint.to_account_info(),
            },
            signer,
        );
        
        token_interface::transfer_checked(
            transfer_ctx,
            return_amount,
            ctx.accounts.dbtc_mint.decimals,
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
    emit!(DogeBtcUnstaked {
        owner: ctx.accounts.authority.key(),
        player_data: player_data_key,
        position_index,
        position_key,
        new_sol_rewards,
        new_dbtc_rewards,
        unrefined_dbtc: accrued_dbtc_rewards,
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
    
    msg!("✅ [unstake_moondoge] Unstake completed successfully");
    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- STAKE LIQUIDITY LP TOKENS :: User gets SOL and dbtc rewards ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Stake LP tokens
/// Users stake LP tokens to a faction and earn SOL and dbtc rewards
/// SOL rewards are distributed per round via join_round function
/// dbtc rewards are distributed per round via end_round function
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
    let (new_sol_rewards, new_dbtc_rewards, accrued_dbtc_rewards) = update_lp_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;

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
        new_dbtc_rewards,
        unrefined_dbtc: accrued_dbtc_rewards,
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
    
    msg!("🔓 [unstake_moondoge] Processing unstake for position {}", position_index);
    
    // Validate the position exists and has funds
    require!(faction_state.faction_id == user_position.faction_id, ErrorCode::InvalidFactionId);
    require!(user_position.staked_amount > 0, ErrorCode::InvalidAmount);
    require!(user_position.position_index == position_index, ErrorCode::InvalidParameters);    
    require!( player_data.moondoge_position_indices.contains(&position_index), ErrorCode::InvalidParameters);
    
    msg!(
        "📊 Position details - Staked: {}, Weighted: {}, Faction: {}, Lockup ends: {}",
        user_position.staked_amount as f64 / 1e6 , 
        user_position.weighted_amount as f64 / 1e6,
        user_position.faction_id,
        user_position.lockup_end_timestamp
    );
    
    // -------------- ACCRUE PENDING REWARDS -------------- //
    
    // Process pending rewards before updating position
    let (new_sol_rewards, new_dbtc_rewards, accrued_dbtc_rewards) = update_lp_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;

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
    helper::remove_moondoge_position(player_data, position_index)?;
        

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
        msg!("💱 Transferring {} DOGE_BTC tokens to user", return_amount);
        
        // Get PDA signer seeds for the dbtc_custodian authority (global, no faction_id)
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
        new_dbtc_rewards,
        unrefined_dbtc: accrued_dbtc_rewards,
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
    
    msg!("✅ [unstake_moondoge] Unstake completed successfully");
    Ok(())
}


// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- CLAIM SOL REWARDS :: User earns SOL rewards from staking ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Claim SOL rewards from staking DogeBtc and LP tokens
pub fn claim_sol_rewards(ctx: Context<ClaimSolRewards>) -> Result<()> {
    msg!("💰 [claim_sol_rewards] Claiming SOL rewards from staking");
    
    require!(ctx.accounts.faction_state.faction_id == ctx.accounts.player_data.faction_id, ErrorCode::InvalidFactionId);
    
    // Store values before mutable borrow (for event emission)
    let player_data_key = ctx.accounts.player_data.key();
    let faction_id = ctx.accounts.faction_state.faction_id;
    
    let faction_state = &ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    
    // Process DogeBtc staking SOL rewards
    let (_st_dbtc_new_sol_rewards, _st_dbtc_new_dbtc_rewards, _st_dbtc_accrued_dbtc_rewards) = update_dbtc_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;
    // Process LP staking SOL rewards
    let (_st_lp_new_sol_rewards, _st_lp_new_dbtc_rewards, _st_lp_accrued_dbtc_rewards) = update_lp_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;
    
    let total_pending_sol_rewards = player_data.pending_sol_rewards;
    require!(total_pending_sol_rewards > 0, ErrorCode::InsufficientFunds);    
    msg!("   Total claimable SOL rewards: {} lamports", total_pending_sol_rewards as f64 / 1e9);
    msg!("   Total claimable DogeBtc rewards: {} dbtc", player_data.pending_dbtc_rewards as f64 / 1e6);
    
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
// ---- CLAIM DBTC REWARDS :: User earns dbtc rewards from staking ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Claim DogeBtc token rewards from staking DogeBtc and LP tokens
/// Implements refining fee: 10% of claimed rewards are redistributed to other unclaimed stakers
/// Also increases power of all staked eggs proportionally to claimed amount
pub fn claim_dbtc_rewards(ctx: Context<ClaimDbtcRewards>) -> Result<()> {
    msg!("💰 [claim_dbtc_rewards] Claiming DogeBtc token rewards with refining fee");
    
    require!(ctx.accounts.faction_state.faction_id == ctx.accounts.player_data.faction_id, ErrorCode::InvalidFactionId);
    
    // Store values before mutable borrow (for event emission)
    let player_data_key = ctx.accounts.player_data.key();
    let faction_id = ctx.accounts.faction_state.faction_id;
    
    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let unrefined_dbtc = &mut ctx.accounts.unrefined_rewards;
    let global_config = &ctx.accounts.global_config;

    // Process DogeBtc staking SOL rewards
    let (_st_dbtc_new_sol_rewards, _st_dbtc_new_dbtc_rewards, _st_dbtc_accrued_dbtc_rewards) = update_dbtc_staking_rewards(player_data, unrefined_dbtc, faction_state)?;
    // Process LP staking SOL rewards
    let (_st_lp_new_sol_rewards, _st_lp_new_dbtc_rewards, _st_lp_accrued_dbtc_rewards) = update_lp_staking_rewards(player_data, unrefined_dbtc, faction_state)?;
    
    require!(player_data.pending_dbtc_rewards > 0, ErrorCode::InsufficientFunds);
    
    // Apply refining fee (10% by default, or configured in global_config)
    let refining_fee_pct = global_config.dbtc_dist_config.refining_fee as u64;
    let refining_fee = (player_data.pending_dbtc_rewards * refining_fee_pct) / M_HUNDRED;
    let claimable_amount = player_data.pending_dbtc_rewards - refining_fee;
    msg!("Refining fee: {} dbtc. Claimable amount: {} dbtc", refining_fee as f64 / 1e6, claimable_amount as f64 / 1e6);
    
    // Apply referral fee first (5% of total)
    let has_referrer = player_data.referral_code != ctx.accounts.system_program.key();
    let referral_fee = if has_referrer {
        let fee = claimable_amount * REFERRAL_FEE_PCT / 100; // 5% referral fee
        msg!("   Referral fee (5%): {} dbtc", fee);
        
        // Add fee to referrer's pending dbtc rewards
        if let Some(referrer_rewards) = &mut ctx.accounts.referrer_rewards {
            referrer_rewards.pending_dbtc_rewards += fee;
            referrer_rewards.total_dbtc_earned += fee;
            msg!("     Added {} dbtc to referrer's rewards", fee);
        }
        fee
    } else {
        0
    };
    
    let claimable_by_user = claimable_amount - referral_fee;
    msg!("Claimable by user: {} dbtc", claimable_by_user as f64 / 1e6);
    
    // Accumulate power points (1 power per 1000 dbtc claimed, minimum 1 power)

    let power_points = calculate_power_points(claimable_by_user);
    player_data.claimable_power += power_points;
    msg!("   Accumulated {} power points (total claimable: {})", power_points, player_data.claimable_power);
    
    // Transfer claimable DogeBtc to user
    if claimable_by_user > 0 {
        msg!("💱 Transferring {} DogeBtc tokens to user", claimable_amount as f64 / 1e6);
        
        // Get PDA signer seeds for the dbtc vault authority
        let vault_authority_seeds = &[
            DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref(),
            &[ctx.bumps.dbtc_vault_authority],
        ];
        let signer = &[&vault_authority_seeds[..]];
        
        let transfer_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token_interface::TransferChecked {
                    from: ctx.accounts.dbtc_token_vault.to_account_info(),
                    to: ctx.accounts.user_dbtc_account.to_account_info(),
                    authority: ctx.accounts.dbtc_vault_authority.to_account_info(),
                    mint: ctx.accounts.dbtc_mint.to_account_info(),
                },
            signer,
        );
        token_interface::transfer_checked(
            transfer_ctx,
            claimable_by_user,
            ctx.accounts.dbtc_mint.decimals,
        )?;
    }    

    // update total claimable dbtc amount 
    unrefined_dbtc.total_dbtc_claimable = unrefined_dbtc.total_dbtc_claimable - player_data.pending_dbtc_rewards;
    player_data.pending_dbtc_rewards = 0;

    // Update total tokens distributed
    let doge_btc_mining = &mut ctx.accounts.doge_btc_mining;
    doge_btc_mining.total_tokens_distributed += claimable_by_user;
    msg!("   Updated total tokens distributed: {} (+{})", doge_btc_mining.total_tokens_distributed as f64 / 1e6, claimable_by_user as f64 / 1e6);
    
    // Store referrer before emitting event
    let referrer_pubkey = if has_referrer { Some(player_data.referral_code) } else { None };
    
    // Redistribute refining fee to all other stakers who haven't claimed
    // This is done by increasing the reward index, which benefits all stakers proportionally
    if refining_fee > 0 {
        msg!("   Redistributing refining fee to other stakers...");
        let increment = helper::mul_div(refining_fee, INDEX_PRECISION, unrefined_dbtc.total_dbtc_claimable)?;
        unrefined_dbtc.unrefining_index += increment;
        msg!("   Updated unrefining index: {} (+{})", unrefined_dbtc.unrefining_index, increment);        
    }
    
    emit!(DbtcRewardsClaimed {
        user: ctx.accounts.authority.key(),
        player_data: player_data_key,
        faction_id,
        dbtc_amount: claimable_by_user,
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

/// Claim referral rewards (SOL and DogeBtc)
pub fn claim_referral_rewards(ctx: Context<ClaimReferralRewards>) -> Result<()> {
    msg!("💰 [claim_referral_rewards] Claiming referral rewards");
    
    let referral_rewards = &mut ctx.accounts.referral_rewards;
    
    let pending_sol = referral_rewards.pending_sol_rewards;
    let pending_dbtc = referral_rewards.pending_dbtc_rewards;
    
    require!(pending_sol > 0 || pending_dbtc > 0, ErrorCode::InsufficientFunds);
    
    msg!("   Pending SOL: {} lamports. Pending DogeBtc: {} dbtc", pending_sol as f64 / 1e9, pending_dbtc as f64 / 1e6);
    
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
    
    // Transfer DogeBtc if any
    if pending_dbtc > 0 {
        let vault_authority_seeds = &[
            DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref(),
            &[ctx.bumps.dbtc_vault_authority],
        ];
        let signer = &[&vault_authority_seeds[..]];
        
        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token_interface::TransferChecked {
                from: ctx.accounts.dbtc_token_vault.to_account_info(),
                to: ctx.accounts.user_dbtc_account.to_account_info(),
                authority: ctx.accounts.dbtc_vault_authority.to_account_info(),
                mint: ctx.accounts.dbtc_mint.to_account_info(),
            },
            signer,
        );
        
        token_interface::transfer_checked(
            transfer_ctx,
            pending_dbtc,
            ctx.accounts.dbtc_mint.decimals,
        )?;
        msg!("   ✓ Transferred {} dbtc", pending_dbtc);
    }
    
    // Update total tokens distributed
    let doge_btc_mining = &mut ctx.accounts.doge_btc_mining;
    doge_btc_mining.total_tokens_distributed += pending_dbtc;
    msg!("   Updated total tokens distributed: {} (+{})", doge_btc_mining.total_tokens_distributed as f64 / 1e6, pending_dbtc as f64 / 1e6);
    
    // Reset pending rewards
    referral_rewards.pending_sol_rewards = 0;
    referral_rewards.pending_dbtc_rewards = 0;
    
    emit!(ReferralRewardsClaimed {
        referrer: ctx.accounts.authority.key(),
        referral_rewards_account: ctx.accounts.referral_rewards.key(),
        sol_amount: pending_sol,
        dbtc_amount: pending_dbtc,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    msg!("✅ [claim_referral_rewards] Claimed referral rewards successfully");
    Ok(())
}

// ----------------------------------------------------------------------------------------
// -------------- HELPER FUNCTIONS ---------------------------------------------------------
// ----------------------------------------------------------------------------------------


pub fn update_dbtc_staking_rewards( player_data: &mut PlayerData, unrefined_rewards: &mut UnrefinedRewards, faction_state: &FactionState) -> Result<(u64, u64, u64)> {
    msg!("💰 Processing pending rewards before position update");
    let mut new_dbtc_rewards = 0;
    let mut new_sol_rewards = 0;
    let mut accrued_dbtc_rewards = 0;
        
    if player_data.dogebtc_hashpower > 0 {
                
        // Calculate SOL rewards using helper function (convert u128 indexes to u64 for calculation)
        new_sol_rewards = helper::calculate_staking_rewards( player_data.dogebtc_hashpower,faction_state.dbtc_sol_reward_index, player_data.dbtc_sol_reward_debt)?;
        player_data.pending_sol_rewards += new_sol_rewards;
        msg!("   Updated pending SOL rewards: {} (+{})", player_data.pending_sol_rewards as f64 / 1e9, new_sol_rewards as f64 / 1e9);

        new_dbtc_rewards = helper::calculate_staking_rewards( player_data.dogebtc_hashpower,faction_state.dbtc_dbtc_reward_index, player_data.dbtc_dbtc_reward_debt)?;
        accrued_dbtc_rewards = helper::add_to_total_claimable(unrefined_rewards, player_data, new_dbtc_rewards);
        msg!("   Updated pending DogeBtc rewards: {} (+{})", player_data.pending_dbtc_rewards as f64 / 1e6, new_dbtc_rewards as f64 / 1e6);
    }

    // Update reward debt to current indexes
    player_data.dbtc_sol_reward_debt = faction_state.dbtc_sol_reward_index;
    player_data.dbtc_dbtc_reward_debt = faction_state.dbtc_dbtc_reward_index;
    
    Ok((new_sol_rewards, new_dbtc_rewards, accrued_dbtc_rewards))
}



pub fn update_lp_staking_rewards( player_data: &mut PlayerData, unrefined_rewards: &mut UnrefinedRewards, faction_state: &FactionState) -> Result<(u64, u64, u64)> {
    msg!("💰 Processing pending rewards before position update");
    let mut new_dbtc_rewards = 0;
    let mut new_sol_rewards = 0;
    let mut accrued_dbtc_rewards = 0;

    if player_data.lp_hashpower > 0 {                    
        // Calculate SOL rewards using helper function (convert u128 indexes to u64 for calculation)
        new_sol_rewards = helper::calculate_staking_rewards( player_data.lp_hashpower,faction_state.lp_sol_reward_index, player_data.lp_sol_reward_debt)?;
        player_data.pending_sol_rewards += new_sol_rewards;
        msg!("   Updated pending SOL rewards: {} (+{})", player_data.pending_sol_rewards as f64 / 1e9, new_sol_rewards as f64 / 1e9);

        new_dbtc_rewards = helper::calculate_staking_rewards( player_data.lp_hashpower,faction_state.lp_dbtc_reward_index, player_data.lp_dbtc_reward_debt)?;
        accrued_dbtc_rewards = helper::add_to_total_claimable(unrefined_rewards, player_data, new_dbtc_rewards);
        msg!("   Updated pending DogeBtc rewards: {} (+{})", player_data.pending_dbtc_rewards as f64 / 1e6, new_dbtc_rewards as f64 / 1e6);

        // Update reward debt to current indexes
        player_data.lp_sol_reward_debt = faction_state.lp_sol_reward_index;
        player_data.lp_dbtc_reward_debt = faction_state.lp_dbtc_reward_index;
    }

    Ok((new_sol_rewards, new_dbtc_rewards, accrued_dbtc_rewards))
}

fn calculate_power_points(claimable_by_user: u64) -> u64 {
    let power_points = if claimable_by_user > 0 {
        let power = claimable_by_user / 10_000; // 100 power per 1 dbtc (with 6 decimals)
        if power == 0 && claimable_by_user > 0 {
            1 // Minimum 1 power if any dbtc claimed
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
// --------- STAKE MOONDOGE ---------
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

#[derive(Accounts)]
#[instruction(amount: u64, lockup_duration: u64, position_index: u8)]
pub struct StakeDogeBtc<'info> {
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
    
    /// CHECK: DOGE_BTC Mint (validated manually)
    pub dbtc_mint: InterfaceAccount<'info, Mint2022>,

    // Token accounts
    #[account(
        mut,
        constraint = user_dbtc_account.mint == dbtc_mint.key() @ ErrorCode::InvalidParameters,
        constraint = user_dbtc_account.owner == authority.key() @ ErrorCode::InvalidOwner,
        constraint = user_dbtc_account.amount >= amount @ ErrorCode::InsufficientFunds,
    )]
    /// User's DogeBtc token account
    pub user_dbtc_account: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        mut,
        seeds = [DBTC_CUSTODIAN_SEED.as_ref()],
        bump,
        constraint = dbtc_custodian.mint == dbtc_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// Token-2022 account that holds staked DOGE_BTC for this faction
    pub dbtc_custodian: InterfaceAccount<'info, TokenAccount2022>,
    
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
// --------- UNSTAKE MOONDOGE ---------
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

#[derive(Accounts)]
#[instruction(position_index: u8)]
pub struct UnstakeDogeBtc<'info> {
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
        seeds = [
            STAKED_POSITION_SEED.as_ref(),
            authority.key().as_ref(),
            &[position_index]
        ],
        bump = user_position.bump,
        constraint = user_position.position_index == position_index @ ErrorCode::InvalidParameters,
        close = authority
    )]
    pub user_position: Box<Account<'info, StakedPosition>>,
    
    /// CHECK: DOGE_BTC Mint (validated manually)
    pub dbtc_mint: InterfaceAccount<'info, Mint2022>,

    // Token accounts
    #[account(
        mut,
        constraint = user_dbtc_account.owner == authority.key() @ ErrorCode::InvalidOwner,
        constraint = user_dbtc_account.mint == dbtc_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// User's DogeBtc token account to receive the unstaked tokens
    pub user_dbtc_account: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        mut,
        seeds = [DBTC_CUSTODIAN_SEED.as_ref()],
        bump,
        constraint = dbtc_custodian.mint == dbtc_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// Token-2022 account that holds staked DOGE_BTC (global for all factions)
    pub dbtc_custodian: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        seeds = [DBTC_CUSTODIAN_AUTHORITY_SEED.as_ref()],
        bump,
    )]
    /// CHECK: Authority of the custodian (PDA that signs for token transfers, global for all factions)
    pub dbtc_custodian_authority: UncheckedAccount<'info>,
    
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
        seeds = [
            LP_STAKED_POSITION_SEED.as_ref(),
            authority.key().as_ref(),
            &[position_index]
        ],
        bump = user_position.bump,
        constraint = user_position.position_index == position_index @ ErrorCode::InvalidParameters,
        close = authority
    )]
    pub user_position: Box<Account<'info, StakedPosition>>,
    
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
// --------- CLAIM DBTC REWARDS ---------
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
    
    /// CHECK: DOGE_BTC Mint (validated manually)
    pub dbtc_mint: InterfaceAccount<'info, Mint2022>,
    
    // Token accounts
    #[account(
        mut,
        constraint = user_dbtc_account.mint == dbtc_mint.key() @ ErrorCode::InvalidParameters,
        constraint = user_dbtc_account.owner == authority.key() @ ErrorCode::InvalidOwner,
    )]
    /// User's DogeBtc token account to receive rewards
    pub user_dbtc_account: InterfaceAccount<'info, TokenAccount2022>,
    
    /// CHECK: DogeBtc mining state (needed for vault PDA derivation)
    #[account(
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,
    
    /// CHECK: DogeBtc token vault (main vault where tokens are deposited)
    #[account(
        mut,
        seeds = [DOGE_BTC_VAULT_SEED.as_ref(), doge_btc_mining.key().as_ref()],
        bump,
        constraint = dbtc_token_vault.mint == dbtc_mint.key() @ ErrorCode::InvalidMint,
    )]
    pub dbtc_token_vault: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        seeds = [DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump
    )]
    /// CHECK: Authority of the token vault (PDA that signs for token transfers)
    pub dbtc_vault_authority: UncheckedAccount<'info>,
    
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
    
    /// CHECK: DOGE_BTC Mint (validated manually)
    pub dbtc_mint: InterfaceAccount<'info, Mint2022>,
    
    // Token accounts
    #[account(
        mut,
        constraint = user_dbtc_account.mint == dbtc_mint.key() @ ErrorCode::InvalidParameters,
        constraint = user_dbtc_account.owner == authority.key() @ ErrorCode::InvalidOwner,
    )]
    /// Referrer's DogeBtc token account to receive rewards
    pub user_dbtc_account: InterfaceAccount<'info, TokenAccount2022>,
    
    /// CHECK: SOL rewards vault (System Account)
    #[account(
        mut,
        seeds = [STAKER_SOL_REWARD_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_rewards_vault: UncheckedAccount<'info>,
    
    /// CHECK: DogeBtc mining state (needed for vault PDA derivation)
    #[account(
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,
    
    /// CHECK: DogeBtc token vault (main vault where tokens are deposited)
    #[account(
        mut,
        seeds = [DOGE_BTC_VAULT_SEED.as_ref(), doge_btc_mining.key().as_ref()],
        bump,
        constraint = dbtc_token_vault.mint == dbtc_mint.key() @ ErrorCode::InvalidMint,
    )]
    pub dbtc_token_vault: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        seeds = [DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump
    )]
    /// CHECK: Authority of the token vault (PDA that signs for token transfers)
    pub dbtc_vault_authority: UncheckedAccount<'info>,
    
    /// Referrer claiming rewards
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    
    /// Token-2022 program for SPL-22 token operations
    pub token_program: Program<'info, Token2022>,
}

