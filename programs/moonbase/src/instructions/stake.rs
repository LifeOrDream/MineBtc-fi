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

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- INITIALIZE A USER'S ELECTRICITY ACCOUNT ------------------------ ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

// Old initialize_player_account function removed - no longer needed for Faction Surge system
// Users now initialize via initialize_player in moonbase program

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- STAKE MOONDOGE TOKENS :: User gets electricity and SOL rewards ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------


/// Stake DogeBtc tokens
/// Users stake DogeBtc tokens to a faction and earn SOL and dbtc rewards
/// SOL rewards are distributed per round via join_round function
/// dbtc rewards are distributed per round via end_round function
pub fn stake_moondoge(
    ctx: Context<StakeDogeBtc>,
    faction_id: u8,
    amount: u64,
    lockup_duration: u64,
    position_index: u8,
) -> Result<()> {
    msg!(
        "🔒 [stake_moondoge] Starting DogeBtc staking - Amount: {}, Lockup: {} days, Position: {}, Faction: {}",
        amount,
        lockup_duration,
        position_index,
        faction_id
    );
    
    // Validate inputs
    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(
        lockup_duration >= ctx.accounts.hashpower_config.min_lockup_days
            && lockup_duration <= ctx.accounts.hashpower_config.max_lockup_days,
        ErrorCode::InvalidParameters
    );
    require!(faction_id < NUM_FACTIONS as u8, ErrorCode::InvalidFactionId);
    
    // Calculate actual amount after burn tax
    let burn_amount = amount * BURN_TAX_PERCENTAGE / M_HUNDRED;
    let actual_amount = amount - burn_amount;
    
    msg!("🔥 mDoge burn tax: {}% - Amount: {}, Burn: {}, Actual amount: {}", 
        BURN_TAX_PERCENTAGE, amount, burn_amount, actual_amount);  
    
    // Get faction state
    let faction_state = &mut ctx.accounts.faction_state;
    require!(faction_state.faction_id == faction_id, ErrorCode::InvalidFactionId);
    msg!("📊 Current faction state - Total staked: {}, Total hashpower: {}", 
        faction_state.dbtc_staked, faction_state.total_dbtc_hashpower);

    // Get player data
    let player_data = &mut ctx.accounts.player_data;
    let hashpower_config = &ctx.accounts.hashpower_config;
    let current_ts = Clock::get()?.unix_timestamp;
    
    // Get or create position
    let user_position = &mut ctx.accounts.user_position;

    // Add position index to player data
    helper::add_dogebtc_position(player_data, position_index)?;

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
    msg!("⚖️ Weighted amount: {} (actual amount: {} × multiplier: {}%)", 
        weighted_amount, actual_amount, multiplier);
    
    // Process pending rewards before updating position
    if player_data.dogebtc_hashpower > 0 {
        msg!("💰 Processing pending rewards before position update");
                
        // Calculate SOL rewards using helper function (convert u128 indexes to u64 for calculation)
        let sol_reward_index_u64 = faction_state.dbtc_sol_reward_index.min(u64::MAX as u128) as u64;
        let sol_reward_debt_u64 = player_data.dbtc_sol_reward_debt.min(u64::MAX as u128) as u64;
        let new_sol_rewards = helper::calculate_staking_rewards(
            player_data.dogebtc_hashpower,
            sol_reward_index_u64,
            sol_reward_debt_u64
        )?;
        player_data.pending_sol_rewards += new_sol_rewards;
        msg!("   Updated pending SOL rewards: {} (+{})", player_data.pending_sol_rewards, new_sol_rewards);

        // Calculate dbtc rewards (using u128 indexes)
        let dbtc_reward_diff = faction_state.dbtc_dbtc_reward_index
            .saturating_sub(player_data.dbtc_dbtc_reward_debt);
        msg!("   Calculated dbtc reward diff: {} (will be claimable separately)", dbtc_reward_diff);
    }
    
    // If position exists, validate and update
    if user_position.staked_amount > 0 {    
        msg!("🔄 Updating existing position - Current amount: {}", user_position.staked_amount);
        require!(user_position.faction_id == faction_id, ErrorCode::InvalidFactionId);
        require!(user_position.lockup_end_timestamp > current_ts, ErrorCode::InvalidParameters);

        // Update staked amount with actual_amount (post-tax)
        user_position.staked_amount += actual_amount;
        user_position.weighted_amount += weighted_amount;        
        msg!("   Position updated - staked: {}, weighted: {}", 
            user_position.staked_amount, user_position.weighted_amount);
    } else {
        msg!("🆕 Creating new position {}", position_index);
        // Initialize new position
        helper::init_position(
            user_position, 
            faction_id, 
            position_index, 
            actual_amount, 
            weighted_amount, 
            lockup_duration, 
            current_ts, 
            multiplier
        )?;                
    }
    
    // Update player data state
    player_data.dogebtc_hashpower += weighted_amount;
    player_data.dogebtc_staked += actual_amount;
    
    // Update reward debt to current indexes
    player_data.dbtc_sol_reward_debt = faction_state.dbtc_sol_reward_index;
    player_data.dbtc_dbtc_reward_debt = faction_state.dbtc_dbtc_reward_index;
    msg!("   Updated player reward debts - SOL: {}, dbtc: {}", 
        player_data.dbtc_sol_reward_debt, player_data.dbtc_dbtc_reward_debt);

    msg!("💱 Transferring {} DOGE_BTC tokens from user to custodian", amount);
    msg!("   From: {}", ctx.accounts.user_dbtc_account.key());
    msg!("   To: {}", ctx.accounts.dbtc_custodian.key());

    // Transfer tokens from user to custodian
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
    
    // Update faction state with actual_amount (post-tax) and weighted_amount
    faction_state.dbtc_staked += actual_amount;
    faction_state.total_dbtc_hashpower += weighted_amount;
    msg!("   Updated faction state - Total staked: {}, Total hashpower: {}", 
        faction_state.dbtc_staked, faction_state.total_dbtc_hashpower);

    msg!("✅ [stake_moondoge] DogeBtc staking successful");    
    emit!(DogeBtcStaked {
        owner: ctx.accounts.authority.key(),
        amount: actual_amount,
        lockup_duration,
        multiplier,
        weighted_amount,
        total_hashpower_contribution: weighted_amount,
        position_index,
    });
    
    Ok(())
}



// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- UNSTAKE MOONDOGE TOKENS :: User gets DOGE_BTC back ------------------------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Unstake DogeBtc tokens from a position
pub fn unstake_moondoge(ctx: Context<UnstakeDogeBtc>, position_index: u8) -> Result<()> {
    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let user_position = &mut ctx.accounts.user_position;
    let current_ts = Clock::get()?.unix_timestamp;
    
    msg!("🔓 [unstake_moondoge] Processing unstake for position {}", position_index);
    
    // Validate the position exists and has funds
    require!(user_position.staked_amount > 0, ErrorCode::InvalidAmount);
    require!(user_position.position_index == position_index, ErrorCode::InvalidParameters);
    
    // Verify position index is in the user's active positions
    require!(
        player_data.moondoge_position_indices.contains(&position_index),
        ErrorCode::InvalidParameters
    );
    
    msg!(
        "📊 Position details - Staked: {}, Weighted: {}, Faction: {}, Lockup ends: {}",
        user_position.staked_amount, 
        user_position.weighted_amount,
        user_position.faction_id,
        user_position.lockup_end_timestamp
    );
    
    // Process pending rewards before updating position
    if player_data.dogebtc_hashpower > 0 {
        msg!("💰 Processing pending rewards before unstaking");
                    
        // Calculate SOL rewards using helper function (convert u128 indexes to u64 for calculation)
        let sol_reward_index_u64 = faction_state.dbtc_sol_reward_index.min(u64::MAX as u128) as u64;
        let sol_reward_debt_u64 = player_data.dbtc_sol_reward_debt.min(u64::MAX as u128) as u64;
        let new_sol_rewards = helper::calculate_staking_rewards(
            player_data.dogebtc_hashpower,
            sol_reward_index_u64,
            sol_reward_debt_u64
        )?;
        player_data.pending_sol_rewards += new_sol_rewards;
        msg!("   Updated pending SOL rewards: {} (+{})", player_data.pending_sol_rewards, new_sol_rewards);

        // Calculate dbtc rewards (using u128 indexes)
        let dbtc_reward_diff = faction_state.dbtc_dbtc_reward_index
            .saturating_sub(player_data.dbtc_dbtc_reward_debt);
        msg!("   Calculated dbtc reward diff: {} (will be claimable separately)", dbtc_reward_diff);
    }
   
    // Update reward debt to current indexes
    player_data.dbtc_sol_reward_debt = faction_state.dbtc_sol_reward_index;
    player_data.dbtc_dbtc_reward_debt = faction_state.dbtc_dbtc_reward_index;
    
    // Calculate return amount based on early withdrawal status
    let is_early_withdrawal = current_ts < user_position.lockup_end_timestamp;
    let original_amount = user_position.staked_amount;
    let original_weighted = user_position.weighted_amount;
    let mut return_amount = original_amount;
    
    // Handle early withdrawal if needed
    if is_early_withdrawal {
        msg!(
            "⚠️ Early unstake detected! Current time: {}, Lockup end: {}",
            current_ts,
            user_position.lockup_end_timestamp
        );
        
        // Calculate remaining lockup percentage
        let total_lockup_seconds = user_position.lockup_end_timestamp - user_position.start_timestamp;
        let remaining_seconds = user_position.lockup_end_timestamp - current_ts;
        let remaining_seconds_pct = if total_lockup_seconds > 0 {
            (M_HUNDRED as i64 * remaining_seconds / total_lockup_seconds) as u64
        } else {
            0
        };
        msg!("   Lockup remaining: {}%", remaining_seconds_pct);
        
        // Apply emergency tax for early withdrawal (if configured in global_config)
        // Note: emergency_tax might not exist in GlobalConfig, using 0 for now
        let emergency_tax = 0u64; // TODO: Add emergency_tax to GlobalConfig if needed
        let calc_penalty_pct = emergency_tax * remaining_seconds_pct / M_HUNDRED;
        let penalty_amount = original_amount * calc_penalty_pct / M_HUNDRED;
        return_amount = original_amount - penalty_amount;
        msg!(
            "   Total Staked: {}, Returned: {}, Penalty: {}",
            original_amount,
            return_amount,
            penalty_amount
        );
        
        // Burn penalty tokens if any
        if penalty_amount > 0 {
            msg!("🔥 Burning {} penalty tokens", penalty_amount);
            
            // Get PDA signer seeds for the dbtc_custodian authority
            let custodian_authority_seeds = &[
                DBTC_CUSTODIAN_AUTHORITY_SEED.as_ref(),
                &[faction_state.faction_id],
                &[ctx.bumps.dbtc_custodian_authority],
            ];
            let signer = &[&custodian_authority_seeds[..]];
            
            // Use proper Token-2022 burn instruction
            let burn_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token_interface::Burn {
                    mint: ctx.accounts.dbtc_mint.to_account_info(),
                    from: ctx.accounts.dbtc_custodian.to_account_info(),
                    authority: ctx.accounts.dbtc_custodian_authority.to_account_info(),
                },
                signer,
            );            
            token_interface::burn(burn_ctx, penalty_amount)?;
            
            // Emit emergency withdrawal event
            emit!(EmergencyWithdrawal {
                owner: ctx.accounts.authority.key(),
                position_index,
                original_amount,
                penalty_amount,
                returned_amount: return_amount,
                penalty_tax_pct: calc_penalty_pct,
                timestamp: current_ts,
            });
        }
    } else {
        msg!("✅ Normal unstake - lockup period has ended");
    }
    
    // Update faction state (decrease staked amount and hashpower)
    msg!("📊 Updating faction state");
    faction_state.dbtc_staked -= original_amount;
    faction_state.total_dbtc_hashpower -= original_weighted;
    msg!(
        "   New faction totals - Staked: {}, Hashpower: {}",
        faction_state.dbtc_staked,
        faction_state.total_dbtc_hashpower
    );
    
    // Update player data (decrease hashpower and staked amount)
    msg!("📊 Updating player data");
    player_data.dogebtc_hashpower -= original_weighted;
    player_data.dogebtc_staked -= original_amount;
    msg!(
        "   New player totals - Hashpower: {}, Staked: {}",
        player_data.dogebtc_hashpower,
        player_data.dogebtc_staked
    );
    
    // Remove position from user's active positions
    helper::remove_moondoge_position(player_data, position_index)?;
    msg!("   Updated active positions: {}", player_data.active_moondoge_positions);
    
    // Transfer remaining tokens back to user
    if return_amount > 0 {
        msg!("💱 Transferring {} DOGE_BTC tokens to user", return_amount);
        
        // Get PDA signer seeds for the dbtc_custodian authority
        let custodian_authority_seeds = &[
            DBTC_CUSTODIAN_AUTHORITY_SEED.as_ref(),
            &[user_position.faction_id],
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
    
    emit!(DogeBtcUnstaked {
        owner: ctx.accounts.authority.key(),
        position_index,
        amount: return_amount,
        weighted_amount: original_weighted,
        early_withdrawal: is_early_withdrawal,
    });
    
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
    faction_id: u8,
    amount: u64,
    lockup_duration: u64,
    position_index: u8,
) -> Result<()> {
    msg!(
        "🔒 [stake_lp_tokens] Starting LP token staking - Amount: {}, Lockup: {} days, Position: {}, Faction: {}",
        amount,
        lockup_duration,
        position_index,
        faction_id
    );
    
    // Validate inputs
    require!(amount > 0, ErrorCode::InvalidAmount);
    let hashpower_config = &ctx.accounts.hashpower_config;
    require!(
        lockup_duration >= hashpower_config.min_lockup_days
            && lockup_duration <= hashpower_config.max_lockup_days,
        ErrorCode::InvalidParameters
    );
    require!(faction_id < NUM_FACTIONS as u8, ErrorCode::InvalidFactionId);
    
    // Get faction state
    let faction_state = &mut ctx.accounts.faction_state;
    require!(faction_state.faction_id == faction_id, ErrorCode::InvalidFactionId);
    msg!("📊 Current faction state - Total LP staked: {}, Total LP hashpower: {}", 
        faction_state.total_lp_hashpower, faction_state.total_lp_hashpower);

    // Get player data
    let player_data = &mut ctx.accounts.player_data;
    let current_ts = Clock::get()?.unix_timestamp;
    
    // Get or create position
    let user_position = &mut ctx.accounts.user_position;
    
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
    let weighted_amount = amount * multiplier as u64 / M_HUNDRED;
    msg!("⚖️ Weighted amount: {} (amount: {} × multiplier: {}%)", 
        weighted_amount, amount, multiplier);
    
    // Process pending rewards before updating position
    if player_data.lp_hashpower > 0 {
        msg!("💰 Processing pending rewards before position update");
        
        // Calculate SOL rewards using helper function (convert u128 indexes to u64 for calculation)
        let sol_reward_index_u64 = faction_state.lp_sol_reward_index.min(u64::MAX as u128) as u64;
        let sol_reward_debt_u64 = player_data.lp_sol_reward_debt.min(u64::MAX as u128) as u64;
        let new_sol_rewards = helper::calculate_staking_rewards(
            player_data.lp_hashpower,
            sol_reward_index_u64,
            sol_reward_debt_u64
        )?;
        player_data.pending_sol_rewards += new_sol_rewards;
        msg!("   Updated pending SOL rewards: {} (+{})", player_data.pending_sol_rewards, new_sol_rewards);

        // Calculate dbtc rewards (using u128 indexes)
        let dbtc_reward_diff = faction_state.lp_dbtc_reward_index
            .saturating_sub(player_data.lp_dbtc_reward_debt);
        msg!("   Calculated dbtc reward diff: {} (will be claimable separately)", dbtc_reward_diff);
    }
    
    // If position exists, validate and update
    if user_position.staked_amount > 0 {    
        msg!("🔄 Updating existing position - Current amount: {}", user_position.staked_amount);
        require!(user_position.faction_id == faction_id, ErrorCode::InvalidFactionId);
        require!(user_position.lockup_end_timestamp > current_ts, ErrorCode::InvalidParameters);

        // Update staked amount
        user_position.staked_amount += amount;
        user_position.weighted_amount += weighted_amount;        
        msg!("   Position updated - staked: {}, weighted: {}", 
            user_position.staked_amount, user_position.weighted_amount);
    } else {
        msg!("🆕 Creating new position {}", position_index);
        // Initialize new position
        helper::init_position(
            user_position, 
            faction_id, 
            position_index, 
            amount, 
            weighted_amount, 
            lockup_duration, 
            current_ts, 
            multiplier
        )?;                
    }
    
    // Update player data state
    player_data.lp_hashpower += weighted_amount;
    player_data.lp_staked += amount;
    
    // Update reward debt to current indexes
    player_data.lp_sol_reward_debt = faction_state.lp_sol_reward_index;
    player_data.lp_dbtc_reward_debt = faction_state.lp_dbtc_reward_index;
    msg!("   Updated player reward debts - SOL: {}, dbtc: {}", 
        player_data.lp_sol_reward_debt, player_data.lp_dbtc_reward_debt);

    msg!("💱 Transferring {} LP tokens from user to custodian", amount);
    msg!("   From: {}", ctx.accounts.user_lp_account.key());
    msg!("   To: {}", ctx.accounts.liquidity_custodian.key());

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
    
    // Update faction state with amount and weighted_amount
    faction_state.total_lp_hashpower += weighted_amount;
    msg!("   Updated faction state - Total LP hashpower: {}", faction_state.total_lp_hashpower);

    msg!("✅ [stake_lp_tokens] LP token staking successful");    
    emit!(LiquidityStaked {
        owner: ctx.accounts.authority.key(),
        amount,
        lockup_duration,
        multiplier,
        weighted_amount,
        total_hashpower_contribution: weighted_amount,
        position_index,
    });
    
    Ok(())
}

/// Unstake LP tokens from a position
pub fn unstake_lp_tokens(ctx: Context<UnstakeLpTokens>, position_index: u8) -> Result<()> {
    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let user_position = &mut ctx.accounts.user_position;
    let current_ts = Clock::get()?.unix_timestamp;
    
    msg!("🔓 [unstake_lp_tokens] Processing unstake for position {}", position_index);
    
    // Validate the position exists and has funds
    require!(user_position.staked_amount > 0, ErrorCode::InvalidAmount);
    require!(user_position.position_index == position_index, ErrorCode::InvalidParameters);
    
    // Verify position index is in the user's active positions
    require!(
        player_data.lp_position_indices.contains(&position_index),
        ErrorCode::InvalidParameters
    );
    
    msg!(
        "📊 Position details - Staked: {}, Weighted: {}, Faction: {}, Lockup ends: {}",
        user_position.staked_amount, 
        user_position.weighted_amount,
        user_position.faction_id,
        user_position.lockup_end_timestamp
    );
    
    // Process pending rewards before updating position
    if player_data.lp_hashpower > 0 {
        msg!("💰 Processing pending rewards before unstaking");
        
        // Calculate SOL rewards using helper function (convert u128 indexes to u64 for calculation)
        let sol_reward_index_u64 = faction_state.lp_sol_reward_index.min(u64::MAX as u128) as u64;
        let sol_reward_debt_u64 = player_data.lp_sol_reward_debt.min(u64::MAX as u128) as u64;
        let new_sol_rewards = helper::calculate_staking_rewards(
            player_data.lp_hashpower,
            sol_reward_index_u64,
            sol_reward_debt_u64
        )?;
        player_data.pending_sol_rewards += new_sol_rewards;
        msg!("   Updated pending SOL rewards: {} (+{})", player_data.pending_sol_rewards, new_sol_rewards);

        // Calculate dbtc rewards (using u128 indexes)
        let dbtc_reward_diff = faction_state.lp_dbtc_reward_index
            .saturating_sub(player_data.lp_dbtc_reward_debt);
        msg!("   Calculated dbtc reward diff: {} (will be claimable separately)", dbtc_reward_diff);
    }
   
    // Update reward debt to current indexes
    player_data.lp_sol_reward_debt = faction_state.lp_sol_reward_index;
    player_data.lp_dbtc_reward_debt = faction_state.lp_dbtc_reward_index;
    
    // Calculate return amount based on early withdrawal status
    let is_early_withdrawal = current_ts < user_position.lockup_end_timestamp;
    let original_amount = user_position.staked_amount;
    let original_weighted = user_position.weighted_amount;
    let mut return_amount = original_amount;
    
    // Handle early withdrawal if needed
    if is_early_withdrawal {
        msg!(
            "⚠️ Early unstake detected! Current time: {}, Lockup end: {}",
            current_ts,
            user_position.lockup_end_timestamp
        );
        
        // Calculate remaining lockup percentage
        let total_lockup_seconds = user_position.lockup_end_timestamp - user_position.start_timestamp;
        let remaining_seconds = user_position.lockup_end_timestamp - current_ts;
        let remaining_seconds_pct = if total_lockup_seconds > 0 {
            (M_HUNDRED as i64 * remaining_seconds / total_lockup_seconds) as u64
        } else {
            0
        };
        msg!("   Lockup remaining: {}%", remaining_seconds_pct);
        
        // Apply emergency tax for early withdrawal (if configured in global_config)
        let emergency_tax = 0u64; // TODO: Add emergency_tax to GlobalConfig if needed
        let calc_penalty_pct = emergency_tax * remaining_seconds_pct / M_HUNDRED;
        let penalty_amount = original_amount * calc_penalty_pct / M_HUNDRED;
        return_amount = original_amount - penalty_amount;
        msg!(
            "   Total Staked: {}, Returned: {}, Penalty: {}",
            original_amount,
            return_amount,
            penalty_amount
        );
        
        // Burn penalty tokens if any
        if penalty_amount > 0 {
            msg!("🔥 Burning {} penalty tokens", penalty_amount);
            
            // Get PDA signer seeds for the liquidity_custodian authority
            let custodian_authority_seeds = &[
                LIQUIDITY_CUSTODIAN_AUTHORITY_SEED.as_ref(),
                &[faction_state.faction_id],
                &[ctx.bumps.liquidity_custodian_authority],
            ];
            let signer = &[&custodian_authority_seeds[..]];
            
            // Use proper Token burn instruction
            let burn_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Burn {
                    mint: ctx.accounts.lp_mint.to_account_info(),
                    from: ctx.accounts.liquidity_custodian.to_account_info(),
                    authority: ctx.accounts.liquidity_custodian_authority.to_account_info(),
                },
                signer,
            );            
            token::burn(burn_ctx, penalty_amount)?;
            
            // Emit emergency withdrawal event
            emit!(EmergencyWithdrawal {
                owner: ctx.accounts.authority.key(),
                position_index,
                original_amount,
                penalty_amount,
                returned_amount: return_amount,
                penalty_tax_pct: calc_penalty_pct,
                timestamp: current_ts,
            });
        }
    } else {
        msg!("✅ Normal unstake - lockup period has ended");
    }
    
    // Update faction state (decrease LP hashpower)
    msg!("📊 Updating faction state");
    faction_state.total_lp_hashpower -= original_weighted;
    msg!("   New faction totals - LP Hashpower: {}", faction_state.total_lp_hashpower);
    
    // Update player data (decrease hashpower and staked amount)
    msg!("📊 Updating player data");
    player_data.lp_hashpower -= original_weighted;
    player_data.lp_staked -= original_amount;
    msg!(
        "   New player totals - LP Hashpower: {}, LP Staked: {}",
        player_data.lp_hashpower,
        player_data.lp_staked
    );
    
    // Remove position from user's active positions
    helper::remove_lp_position(player_data, position_index)?;
    msg!("   Updated active positions: {}", player_data.active_lp_positions);
    
    // Transfer remaining tokens back to user
    if return_amount > 0 {
        msg!("💱 Transferring {} LP tokens to user", return_amount);
        
        // Get PDA signer seeds for the liquidity_custodian authority
        let custodian_authority_seeds = &[
            LIQUIDITY_CUSTODIAN_AUTHORITY_SEED.as_ref(),
            &[user_position.faction_id],
            &[ctx.bumps.liquidity_custodian_authority],
        ];
        let signer = &[&custodian_authority_seeds[..]];
        
        // Transfer tokens back to user
        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.liquidity_custodian.to_account_info(),
                to: ctx.accounts.user_lp_account.to_account_info(),
                authority: ctx.accounts.liquidity_custodian_authority.to_account_info(),
            },
            signer,
        );
        
        token::transfer(transfer_ctx, return_amount)?;
    }
    
    // Reset position data
    user_position.staked_amount = 0;
    user_position.weighted_amount = 0;
    
    emit!(LiquidityUnstaked {
        owner: ctx.accounts.authority.key(),
        position_index,
        amount: return_amount,
        weighted_amount: original_weighted,
        early_withdrawal: is_early_withdrawal,
    });
    
    msg!("✅ [unstake_lp_tokens] Unstake completed successfully");
    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- CLAIM SOL REWARDS :: User earns SOL rewards from staking ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Claim SOL rewards from staking DogeBtc and LP tokens
pub fn claim_sol_rewards(ctx: Context<ClaimSolRewards>, faction_id: u8) -> Result<()> {
    msg!("💰 [claim_sol_rewards] Claiming SOL rewards from staking");
    
    require!(faction_id < NUM_FACTIONS as u8, ErrorCode::InvalidFactionId);
    require!(ctx.accounts.faction_state.faction_id == faction_id, ErrorCode::InvalidFactionId);
    
    let faction_state = &ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    
    // Process DogeBtc staking SOL rewards
    if player_data.dogebtc_hashpower > 0 {
        msg!("   Processing DogeBtc staking SOL rewards");
        let sol_reward_index_u64 = faction_state.dbtc_sol_reward_index.min(u64::MAX as u128) as u64;
        let sol_reward_debt_u64 = player_data.dbtc_sol_reward_debt.min(u64::MAX as u128) as u64;
        let rewards = helper::calculate_staking_rewards(
            player_data.dogebtc_hashpower,
            sol_reward_index_u64,
            sol_reward_debt_u64
        )?;
        player_data.pending_sol_rewards += rewards;
        msg!("     DogeBtc staking SOL rewards: {} (+{})", player_data.pending_sol_rewards, rewards);
    }
    
    // Process LP staking SOL rewards
    if player_data.lp_hashpower > 0 {
        msg!("   Processing LP staking SOL rewards");
        let sol_reward_index_u64 = faction_state.lp_sol_reward_index.min(u64::MAX as u128) as u64;
        let sol_reward_debt_u64 = player_data.lp_sol_reward_debt.min(u64::MAX as u128) as u64;
        let rewards = helper::calculate_staking_rewards(
            player_data.lp_hashpower,
            sol_reward_index_u64,
            sol_reward_debt_u64
        )?;
        player_data.pending_sol_rewards += rewards;
        msg!("     LP staking SOL rewards: {} (+{})", player_data.pending_sol_rewards, rewards);
    }
    
    let total_pending = player_data.pending_sol_rewards;
    require!(total_pending > 0, ErrorCode::InsufficientFunds);
    
    msg!("   Total claimable SOL rewards: {} lamports", total_pending);
    
    // Check if user has a referrer (not system referral account)
    let has_referrer = player_data.referral_code != ctx.accounts.system_program.key();
    let (referral_fee, player_sol) = if has_referrer {
        let fee = total_pending * 5 / 100; // 5% referral fee
        msg!("     Referral fee (5%): {} lamports", fee);
        
        // Add fee to referrer's pending SOL rewards
        if let Some(referrer_rewards) = &mut ctx.accounts.referrer_rewards {
            referrer_rewards.pending_sol_rewards += fee;
            referrer_rewards.total_sol_earned += fee;
            msg!("     Added {} SOL to referrer's rewards", fee);
        }
        (fee, total_pending - fee)
    } else {
        (0, total_pending)
    };
    
    // Transfer SOL rewards to user (after referral fee)
    **ctx.accounts.authority.to_account_info().try_borrow_mut_lamports()? += player_sol;
    **ctx.accounts.sol_rewards_vault.to_account_info().try_borrow_mut_lamports()? -= total_pending;
    
    // Update reward debts to prevent double-claiming
    player_data.dbtc_sol_reward_debt = faction_state.dbtc_sol_reward_index;
    player_data.lp_sol_reward_debt = faction_state.lp_sol_reward_index;
    
    // Reset pending rewards
    player_data.pending_sol_rewards = 0;
    
    msg!("✅ [claim_sol_rewards] Claimed {} SOL (fee: {} to referrer)", player_sol as f64 / 1e9, referral_fee as f64 / 1e9);
    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- CLAIM DBTC REWARDS :: User earns dbtc rewards from staking ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Claim DogeBtc token rewards from staking DogeBtc and LP tokens
/// Implements refining fee: 10% of claimed rewards are redistributed to other unclaimed stakers
/// Also increases power of all staked eggs proportionally to claimed amount
pub fn claim_dbtc_rewards(ctx: Context<ClaimDbtcRewards>, faction_id: u8) -> Result<()> {
    msg!("💰 [claim_dbtc_rewards] Claiming DogeBtc token rewards with refining fee");
    
    require!(faction_id < NUM_FACTIONS as u8, ErrorCode::InvalidFactionId);
    require!(ctx.accounts.faction_state.faction_id == faction_id, ErrorCode::InvalidFactionId);
    
    let faction_state = &mut ctx.accounts.faction_state;
    let player_data = &mut ctx.accounts.player_data;
    let global_config = &ctx.accounts.global_config;
    
    // Calculate DogeBtc rewards from DogeBtc staking
    let dbtc_from_dbtc_staking = if player_data.dogebtc_hashpower > 0 {
        msg!("   Calculating DogeBtc rewards from DogeBtc staking");
        let dbtc_reward_index_u64 = faction_state.dbtc_dbtc_reward_index.min(u64::MAX as u128) as u64;
        let dbtc_reward_debt_u64 = player_data.dbtc_dbtc_reward_debt.min(u64::MAX as u128) as u64;
        let rewards = helper::calculate_staking_rewards(
            player_data.dogebtc_hashpower,
            dbtc_reward_index_u64,
            dbtc_reward_debt_u64
        )?;
        msg!("     DogeBtc from DogeBtc staking: {}", rewards);
        rewards
    } else {
        0
    };
    
    // Calculate DogeBtc rewards from LP staking
    let dbtc_from_lp_staking = if player_data.lp_hashpower > 0 {
        msg!("   Calculating DogeBtc rewards from LP staking");
        let dbtc_reward_index_u64 = faction_state.lp_dbtc_reward_index.min(u64::MAX as u128) as u64;
        let dbtc_reward_debt_u64 = player_data.lp_dbtc_reward_debt.min(u64::MAX as u128) as u64;
        let rewards = helper::calculate_staking_rewards(
            player_data.lp_hashpower,
            dbtc_reward_index_u64,
            dbtc_reward_debt_u64
        )?;
        msg!("     DogeBtc from LP staking: {}", rewards);
        rewards
    } else {
        0
    };
    
    let total_dbtc_rewards = dbtc_from_dbtc_staking + dbtc_from_lp_staking + player_data.pending_dbtc_rewards;
    require!(total_dbtc_rewards > 0, ErrorCode::InsufficientFunds);
    
    msg!("   Total claimable DogeBtc rewards: {} (staking: {}, pending: {})", 
        total_dbtc_rewards, 
        dbtc_from_dbtc_staking + dbtc_from_lp_staking,
        player_data.pending_dbtc_rewards);
    
    // Apply referral fee first (5% of total)
    let has_referrer = player_data.referral_code != ctx.accounts.system_program.key();
    let referral_fee = if has_referrer {
        let fee = total_dbtc_rewards * 5 / 100; // 5% referral fee
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
    
    let after_referral = total_dbtc_rewards - referral_fee;
    
    // Apply refining fee (10% by default, or configured in global_config)
    let refining_fee_pct = global_config.dbtc_dist_config.refining_fee;
    let refining_fee = after_referral * refining_fee_pct as u64 / M_HUNDRED;
    let claimable_amount = after_referral - refining_fee;
    
    msg!("   Refining fee ({}%): {} dbtc", refining_fee_pct, refining_fee);
    msg!("   Claimable after fees: {} dbtc (referral: {}, refining: {})", 
        claimable_amount, referral_fee, refining_fee);
    
    // Redistribute refining fee to all other stakers who haven't claimed
    // This is done by increasing the reward index, which benefits all stakers proportionally
    if refining_fee > 0 {
        msg!("   Redistributing refining fee to other stakers...");
        
        // Calculate total hashpower (excluding this user to avoid self-redistribution)
        let total_other_dbtc_hashpower = faction_state.total_dbtc_hashpower - player_data.dogebtc_hashpower;
        let total_other_lp_hashpower = faction_state.total_lp_hashpower - player_data.lp_hashpower;
        let total_other_hashpower = total_other_dbtc_hashpower + total_other_lp_hashpower;
        
        if total_other_hashpower > 0 {
            // Split refining fee proportionally between DogeBtc and LP stakers
            let dbtc_staker_share = refining_fee / 2;
            let lp_staker_share = refining_fee - dbtc_staker_share;
            
            // Update DogeBtc staker reward index
            if total_other_dbtc_hashpower > 0 {
                let dbtc_refining_delta = helper::mul_div(dbtc_staker_share, INDEX_PRECISION, total_other_dbtc_hashpower)?;
                faction_state.dbtc_dbtc_reward_index += dbtc_refining_delta;
                msg!("     DogeBtc staker refining fee distributed: {} dbtc (index +{})", dbtc_staker_share, dbtc_refining_delta);
            }
            
            // Update LP staker reward index
            if total_other_lp_hashpower > 0 {
                let lp_refining_delta = helper::mul_div(lp_staker_share, INDEX_PRECISION, total_other_lp_hashpower)?;
                faction_state.lp_dbtc_reward_index += lp_refining_delta;
                msg!("     LP staker refining fee distributed: {} dbtc (index +{})", lp_staker_share, lp_refining_delta);
            }
        } else {
            msg!("     ⚠️ No other stakers to redistribute refining fee to");
        }
    }
    
    // Transfer claimable DogeBtc to user
    if claimable_amount > 0 {
        msg!("💱 Transferring {} DogeBtc tokens to user", claimable_amount);
        
        // Get PDA signer seeds for the dbtc emission vault authority
        let emission_vault_authority_seeds = &[
            b"dbtc-emission-vault-authority".as_ref(),
            &[ctx.bumps.dbtc_emission_vault_authority],
        ];
        let signer = &[&emission_vault_authority_seeds[..]];
        
        // Transfer DogeBtc tokens from emission vault to user
        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token_interface::TransferChecked {
                from: ctx.accounts.dbtc_emission_vault.to_account_info(),
                to: ctx.accounts.user_dbtc_account.to_account_info(),
                authority: ctx.accounts.dbtc_emission_vault_authority.to_account_info(),
                mint: ctx.accounts.dbtc_mint.to_account_info(),
            },
            signer,
        );
        
        token_interface::transfer_checked(
            transfer_ctx,
            claimable_amount,
            ctx.accounts.dbtc_mint.decimals,
        )?;
    }
    
    // Update reward debts to prevent double-claiming
    player_data.dbtc_dbtc_reward_debt = faction_state.dbtc_dbtc_reward_index;
    player_data.lp_dbtc_reward_debt = faction_state.lp_dbtc_reward_index;
    
    // Update player stats
    player_data.total_dbtc_won += claimable_amount;
    
    // Reset pending_dbtc_rewards (includes round rewards that were pending)
    player_data.pending_dbtc_rewards = 0;
    
    // Increase power of all staked eggs proportionally to claimed amount
    // Power increase = claimable_amount / 1000 (configurable ratio)
    if !player_data.staked_eggs.is_empty() && claimable_amount > 0 {
        msg!("   Increasing power of {} staked eggs...", player_data.staked_eggs.len());
        let power_increase_per_egg = (claimable_amount / 1000) as u32; // 1 token = 0.001 power per egg
        if power_increase_per_egg > 0 {
            msg!("     Power increase per egg: {}", power_increase_per_egg);
            // Note: Actual egg metadata update would require passing all staked egg accounts
            // For now, we track this in the claim event
            msg!("     ⚠️ TODO: Update egg metadata power (requires egg accounts in context)");
        }
    }
    
    msg!("✅ [claim_dbtc_rewards] Claimed {} dbtc (referral fee: {}, refining fee: {})", 
        claimable_amount, referral_fee, refining_fee);
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
    
    msg!("   Pending SOL: {} lamports", pending_sol);
    msg!("   Pending dbtc: {} tokens", pending_dbtc);
    
    // Transfer SOL if any
    if pending_sol > 0 {
        **ctx.accounts.authority.to_account_info().try_borrow_mut_lamports()? += pending_sol;
        **ctx.accounts.sol_rewards_vault.to_account_info().try_borrow_mut_lamports()? -= pending_sol;
        msg!("   ✓ Transferred {} SOL", pending_sol as f64 / 1e9);
    }
    
    // Transfer DogeBtc if any
    if pending_dbtc > 0 {
        let vault_authority_seeds = &[
            b"dbtc-emission-vault-authority".as_ref(),
            &[ctx.bumps.dbtc_emission_vault_authority],
        ];
        let signer = &[&vault_authority_seeds[..]];
        
        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token_interface::TransferChecked {
                from: ctx.accounts.dbtc_emission_vault.to_account_info(),
                to: ctx.accounts.user_dbtc_account.to_account_info(),
                authority: ctx.accounts.dbtc_emission_vault_authority.to_account_info(),
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
    
    // Reset pending rewards
    referral_rewards.pending_sol_rewards = 0;
    referral_rewards.pending_dbtc_rewards = 0;
    
    msg!("✅ [claim_referral_rewards] Claimed referral rewards successfully");
    Ok(())
}


// ----------------------------------------------------------------------------------------
// ------------ ACCOUNT STRUCTS ----------------------------------------------------------
// ----------------------------------------------------------------------------------------

// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --------- STAKE MOONDOGE ---------
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

#[derive(Accounts)]
#[instruction(faction_id: u8, amount: u64, lockup_duration: u64, position_index: u8)]
pub struct StakeDogeBtc<'info> {
    // Global config
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    // Hashpower config (contains lockup and multiplier settings)
    #[account(
        seeds = [b"hashpower-config"],
        bump
    )]
    pub hashpower_config: Account<'info, HashpowerConfig>,
    
    // Faction state
    #[account(
        mut,
        seeds = [FACTION_STATE_SEED.as_ref(), &[faction_id]],
        bump
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
        seeds = [DBTC_CUSTODIAN_SEED.as_ref(), &[faction_id]],
        bump,
        constraint = dbtc_custodian.mint == dbtc_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// Token-2022 account that holds staked DOGE_BTC for this faction
    pub dbtc_custodian: InterfaceAccount<'info, TokenAccount2022>,
    
    /// CHECK: Optional Dragon Egg metadata account (for multiplier calculation)
    pub dragon_egg_metadata: Option<UncheckedAccount<'info>>,
    
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
        seeds = [FACTION_STATE_SEED.as_ref(), &[user_position.faction_id]],
        bump = faction_state.bump
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
        mut,
        seeds = [
            STAKED_POSITION_SEED.as_ref(),
            authority.key().as_ref(),
            &[position_index]
        ],
        bump = user_position.bump,
        constraint = user_position.position_index == position_index @ ErrorCode::InvalidParameters
    )]
    pub user_position: Account<'info, StakedPosition>,
    
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
        seeds = [DBTC_CUSTODIAN_SEED.as_ref(), &[user_position.faction_id]],
        bump,
        constraint = dbtc_custodian.mint == dbtc_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// Token-2022 account that holds staked DOGE_BTC for this faction
    pub dbtc_custodian: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        seeds = [DBTC_CUSTODIAN_AUTHORITY_SEED.as_ref(), &[user_position.faction_id]],
        bump,
    )]
    /// CHECK: Authority of the custodian (PDA that signs for token transfers)
    pub dbtc_custodian_authority: UncheckedAccount<'info>,
    
    /// CHECK: Optional Dragon Egg metadata account (for multiplier calculation)
    pub dragon_egg_metadata: Option<UncheckedAccount<'info>>,
    
    /// User who is unstaking tokens
    #[account(mut)]
    pub authority: Signer<'info>,
    
    /// System program for creating accounts
    pub system_program: Program<'info, System>,
    
    /// Token-2022 program for SPL-22 token operations
    pub token_program: Program<'info, Token2022>,
}

#[derive(Accounts)]
#[instruction(faction_id: u8, amount: u64, lockup_duration: u64, position_index: u8)]
pub struct StakeLpTokens<'info> {
    // Hashpower config (contains lockup and multiplier settings)
    #[account(
        seeds = [b"hashpower-config"],
        bump
    )]
    pub hashpower_config: Account<'info, HashpowerConfig>,
    
    // Faction state
    #[account(
        mut,
        seeds = [FACTION_STATE_SEED.as_ref(), &[faction_id]],
        bump
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
            STAKED_POSITION_SEED.as_ref(),
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
        seeds = [b"lp-custodian", &[faction_id]],
        bump,
        constraint = liquidity_custodian.mint == lp_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// Token account that holds staked LP tokens for this faction
    pub liquidity_custodian: Account<'info, token::TokenAccount>,
    
    /// CHECK: Optional Dragon Egg metadata account (for multiplier calculation)
    pub dragon_egg_metadata: Option<UncheckedAccount<'info>>,
    
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
        seeds = [FACTION_STATE_SEED.as_ref(), &[user_position.faction_id]],
        bump
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
        mut,
        seeds = [
            STAKED_POSITION_SEED.as_ref(),
            authority.key().as_ref(),
            &[position_index]
        ],
        bump = user_position.bump,
        constraint = user_position.position_index == position_index @ ErrorCode::InvalidParameters
    )]
    pub user_position: Account<'info, StakedPosition>,
    
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
        seeds = [b"lp-custodian", &[user_position.faction_id]],
        bump,
        constraint = liquidity_custodian.mint == lp_mint.key() @ ErrorCode::InvalidParameters,
    )]
    /// Token account that holds staked LP tokens for this faction
    pub liquidity_custodian: Account<'info, token::TokenAccount>,
    
    #[account(
        seeds = [b"lp-custodian-authority", &[user_position.faction_id]],
        bump,
    )]
    /// CHECK: Authority of the custodian (PDA that signs for token transfers)
    pub liquidity_custodian_authority: UncheckedAccount<'info>,
    
    /// CHECK: Optional Dragon Egg metadata account (for multiplier calculation)
    pub dragon_egg_metadata: Option<UncheckedAccount<'info>>,
    
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
#[instruction(faction_id: u8)]
pub struct ClaimSolRewards<'info> {
    // Faction state
    #[account(
        seeds = [FACTION_STATE_SEED.as_ref(), &[faction_id]],
        bump = faction_state.bump
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
#[instruction(faction_id: u8)]
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
        seeds = [FACTION_STATE_SEED.as_ref(), &[faction_id]],
        bump = faction_state.bump
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
    
    /// CHECK: DogeBtc emission vault
    #[account(
        mut,
        seeds = [DBTC_EMISSION_VAULT_SEED.as_ref()],
        bump
    )]
    pub dbtc_emission_vault: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        seeds = [b"dbtc-emission-vault-authority".as_ref()],
        bump
    )]
    /// CHECK: Authority of the emission vault (PDA that signs for token transfers)
    pub dbtc_emission_vault_authority: UncheckedAccount<'info>,
    
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
    
    /// CHECK: DogeBtc emission vault
    #[account(
        mut,
        seeds = [DBTC_EMISSION_VAULT_SEED.as_ref()],
        bump
    )]
    pub dbtc_emission_vault: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        seeds = [b"dbtc-emission-vault-authority".as_ref()],
        bump
    )]
    /// CHECK: Authority of the emission vault (PDA that signs for token transfers)
    pub dbtc_emission_vault_authority: UncheckedAccount<'info>,
    
    /// Referrer claiming rewards
    #[account(mut)]
    pub authority: Signer<'info>,
    
    /// Token-2022 program for SPL-22 token operations
    pub token_program: Program<'info, Token2022>,
}

