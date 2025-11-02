use anchor_lang::prelude::*;
use anchor_lang::system_program::System;
use anchor_spl::token::{self, Token};

use crate::state::*;
use crate::errors::ErrorCode;
use crate::events::*;

use crate::instructions::helper;

use anchor_spl::token_interface;
use anchor_spl::token_interface::{
    Mint as Mint2022,
    TokenAccount as TokenAccount2022,
};
use anchor_spl::token_2022::Token2022;


// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- INITIALIZE A USER'S ELECTRICITY ACCOUNT ------------------------ ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
 

/// Initialize a user's electricity account with tier-based bonuses
pub fn initialize_electricity_account(ctx: Context<InitializeElectricityAc>) -> Result<()> {
    msg!("🔒 Initializing electricity account");
  
    let electricity_ac = &mut ctx.accounts.electricity_ac;

    // Initialize owner if this is a new account
    if electricity_ac.owner == Pubkey::default() {
        electricity_ac.owner = ctx.accounts.authority.key();
        electricity_ac.moondoge_position_indices = Vec::with_capacity( MAX_ALLOWED_POSITIONS as usize );
        electricity_ac.lp_position_indices = Vec::with_capacity( MAX_ALLOWED_POSITIONS as usize);
        
        // Query moonbase init_type and calculate initial electricity bonus
        // Scope the borrow to release it before CPI call
        let (init_type, initial_electricity) = {
            let moonbase_data = ctx.accounts.facility_user_moonbase.try_borrow_data()?;
            let init_type = get_moonbase_init_type(&moonbase_data)?;        
            let initial_electricity = calculate_initial_electricity_bonus(init_type);
            (init_type, initial_electricity)
        }; // moonbase_data is dropped here, releasing the borrow
        
        msg!("🎁 Tier {} - Initial electricity bonus: {}", init_type, initial_electricity);
        
        // Store free electricity amount
        electricity_ac.free_electricity = initial_electricity;
        
        // Grant electricity to moonbase via proper CPI call
        helper::update_user_electricity_cpi(
            &ctx.accounts.moonbase_program.to_account_info(),
            &ctx.accounts.authority.to_account_info(),
            &ctx.accounts.facility_user_moonbase.to_account_info(),
            &ctx.accounts.facility_mining_state.to_account_info(),
            &ctx.accounts.moonbase_global_config.to_account_info(),
            &ctx.accounts.fee_collector.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            ctx.bumps.fee_collector,
            true,
            initial_electricity,
        )?;
        
        
        msg!("👤 Initializing new user electricity account");
        msg!("🎁 Tier {} bonus: {} free electricity granted", init_type, initial_electricity);
    }
   
    msg!("✅ Electricity account initialized");
    Ok(())
}

/// Extract init_type from moonbase account data
fn get_moonbase_init_type(moonbase_data: &[u8]) -> Result<u8> {
    // UserMoonBaseInstance layout:
    // discriminator(8) + owner(32) + referral(32) + modules_count(1) + active_hashpower(8) 
    // + available_electricity(8) + used_electricity(8) + dbtc_claim_index(16) + claimable_dbtc(8)
    // + bump(1) + faction_id(1) + level(1) + init_type(1)
    const INIT_TYPE_OFFSET: usize = 8 + 32 + 32 + 1 + 8 + 8 + 8 + 16 + 8 + 1 + 1 + 1;
    
    if moonbase_data.len() <= INIT_TYPE_OFFSET {
        return Err(ErrorCode::InvalidInitType.into());
    }
    
    Ok(moonbase_data[INIT_TYPE_OFFSET])
}

/// Calculate initial electricity bonus based on tier
fn calculate_initial_electricity_bonus(init_type: u8) -> u64 {
    match init_type {
        1 => 1000,           // 0.5 SOL tier: 1k electricity
        2 => 5_000,      // 1.42 SOL tier: 5k electricity
        3 => 10_000,      // 2.42 SOL tier: 10k electricity
        4 => 15_000,      // 4.20 SOL tier: 15k electricity
        _ => 0,           // fallback
    }
}


// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- STAKE MOONDOGE TOKENS :: User gets electricity and SOL rewards ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Stake DogeBtc tokens
pub fn stake_moondoge(ctx: Context<StakeDogeBtc>, amount: u64, lockup_duration: u64, position_index: u8) -> Result<()> {
    msg!("🔒 Starting DogeBtc staking - Amount: {}, Lockup: {} days, Position: {}", amount, lockup_duration, position_index);
    // Validate inputs
    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(
        lockup_duration >= ctx.accounts.global_config.min_lockup_days && 
        lockup_duration <= ctx.accounts.global_config.max_lockup_days,
        ErrorCode::InvalidLockupPeriod
    );
    
    // Calculate actual amount after burn tax
    let burn_amount = amount.checked_mul(BURN_TAX_PERCENTAGE).unwrap().checked_div(M_HUNDRED).unwrap();
    let actual_amount = amount.checked_sub(burn_amount).unwrap();
    
    msg!("🔥 mDoge burn tax: {}% - Amount: {}, Burn: {}, Actual amount: {}", 
        BURN_TAX_PERCENTAGE, amount, burn_amount, actual_amount);
    
    // Global moondoge vault
    let dogebtc_vault = &mut ctx.accounts.dogebtc_vault;
    msg!("📊 Current vault state - Total locked: {}, Weighted locked: {}", dogebtc_vault.dbtc_locked, dogebtc_vault.weighted_dbtc_locked);

    // User Position :: Electricity account
    let electricity_ac = &mut ctx.accounts.electricity_ac;
    let user_position = &mut ctx.accounts.user_position;
    let current_ts = Clock::get()?.unix_timestamp;
    
    // Initialize owner if this is a new account
    if electricity_ac.owner == Pubkey::default() {
        electricity_ac.owner = ctx.accounts.authority.key();
        electricity_ac.moondoge_position_indices = Vec::with_capacity( MAX_ALLOWED_POSITIONS as usize );
        electricity_ac.lp_position_indices = Vec::with_capacity( MAX_ALLOWED_POSITIONS as usize);
        msg!("👤 Initializing new user electricity account");
    }

    // Add position index to user electricity account
    helper::add_moondoge_position(electricity_ac, position_index)?;

    // Calculate multiplier based on lockup duration
    let multiplier = helper::calculate_multiplier(
        lockup_duration,
        ctx.accounts.global_config.min_lockup_days,
        ctx.accounts.global_config.max_lockup_days,
        ctx.accounts.global_config.base_multiplier,
        ctx.accounts.global_config.max_multiplier,
    )?;
    msg!("🔢 Multiplier for {} days lockup: {}", lockup_duration, multiplier);    
    
    // Calculate weighted amount for this position
    let mut weighted_amount = amount.checked_mul(multiplier as u64).unwrap().checked_div(M_HUNDRED).unwrap();
    msg!("⚖️ Weighted amount: {} (raw amount: {} × multiplier: {})", weighted_amount, amount, multiplier);
    
    // Process any pending rewards before updating position
    if electricity_ac.total_weighted_moondoge > 0 {
        msg!("💰 Processing pending rewards before position update");
        msg!("   Previous reward debt: {}", electricity_ac.moondoge_reward_debt);
                
        // Calculate reward diff since last update
        let reward_diff = dogebtc_vault.accumulated_sol_per_point.checked_sub(electricity_ac.moondoge_reward_debt).unwrap_or(0);
        msg!("   New accumulated sol per point: {}", dogebtc_vault.accumulated_sol_per_point);
        msg!("   Reward diff: {}", reward_diff);

        // rewards earned = total weighted moondoge * accumulated sol per point - reward debt
        let new_rewards = (electricity_ac.total_weighted_moondoge as u128)
            .checked_mul(reward_diff)
            .unwrap_or(0)
            .checked_div(PRECISION_FACTOR)
            .unwrap_or(0) as u64;
        msg!("   New rewards: {}", new_rewards);

        // add rewards to pending rewards
        electricity_ac.pending_moondoge_rewards = electricity_ac.pending_moondoge_rewards.checked_add(new_rewards).unwrap();
        msg!("   Updated pending DOGE_BTC rewards: {}", electricity_ac.pending_moondoge_rewards);
    }
    
    // If position exists, validate and update
    if user_position.staked_amount > 0 {
        msg!("🔄 Updating existing position - Current amount: {}", user_position.staked_amount);
        // Position should be still locked
        require!(user_position.lockup_end_timestamp > current_ts, ErrorCode::PositionStillLocked);        
        // Update existing position
        let old_weighted_amount = user_position.weighted_amount;        
        // Update staked amount with actual_amount (post-tax)
        user_position.staked_amount = user_position.staked_amount.checked_add(actual_amount).ok_or(ErrorCode::ArithmeticOverflow)?;            
        // Update weighted amount - recalculate the total weighted amount for consistency
        user_position.weighted_amount = user_position.staked_amount.checked_mul(multiplier as u64).unwrap().checked_div(M_HUNDRED).unwrap();            
        
        // Calculate the actual weighted amount difference to add to vault
        let weighted_amount_diff = user_position.weighted_amount.checked_sub(old_weighted_amount).unwrap();
        
        // Update user's total mDoge weighted amount
        electricity_ac.total_weighted_moondoge = electricity_ac.total_weighted_moondoge.checked_sub(old_weighted_amount).unwrap()
                                                    .checked_add(user_position.weighted_amount).unwrap();            

        msg!("   New staked amount: {}", user_position.staked_amount);
        msg!("   New weighted amount: {}", user_position.weighted_amount);
        msg!("   Weighted amount diff: {}", weighted_amount_diff);
        msg!("   New total weighted: {}", electricity_ac.total_weighted_moondoge);
        
        // Use the actual difference for vault updates and electricity calculations
        weighted_amount = weighted_amount_diff;
    } else {
        msg!("🆕 Creating new position {}", position_index);
        // Initialize new position
        user_position.position_index = position_index;
        user_position.staked_amount = actual_amount; // Use actual_amount (post-tax)
        user_position.weighted_amount = weighted_amount;
        user_position.start_timestamp = current_ts;
        user_position.multiplier = multiplier;
        user_position.lockup_duration = lockup_duration;
        
        // Calculate lockup end timestamp
        let seconds_to_add = lockup_duration.checked_mul(DAY_IN_SECONDS).unwrap();
        user_position.lockup_end_timestamp = current_ts.checked_add(seconds_to_add as i64).unwrap();

        msg!("   Lockup end: {} (current: {})", user_position.lockup_end_timestamp, current_ts);
                
        // Update user's total mDoge weighted amount
        electricity_ac.total_weighted_moondoge = electricity_ac.total_weighted_moondoge.checked_add(weighted_amount).unwrap();

        msg!("   Active positions: {}", electricity_ac.active_moondoge_positions);
        msg!("   Total weighted DOGE_BTC: {}", electricity_ac.total_weighted_moondoge);        
    }
    
    // Update global user state & Global reward debt
    electricity_ac.total_moondoge_staked = electricity_ac.total_moondoge_staked.checked_add(actual_amount).unwrap();
    electricity_ac.moondoge_reward_debt = dogebtc_vault.accumulated_sol_per_point;
    
    msg!("💱 Transferring {} DOGE_BTC tokens from user to vault", amount);
    msg!("   From: {}", ctx.accounts.user_dbtc_account.key());
    msg!("   To: {}", ctx.accounts.dbtc_custodian.key());

    // Transfer mDoge tokens from user to moondoge vault
    // Note: We transfer the full amount including burn tax, as the tax will be applied during transfer
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
    
    // Update mDoge vault state with actual_amount (post-tax)
    dogebtc_vault.dbtc_locked = dogebtc_vault.dbtc_locked.checked_add(actual_amount).unwrap();        
    dogebtc_vault.weighted_dbtc_locked = dogebtc_vault.weighted_dbtc_locked.checked_add(weighted_amount).unwrap();

    msg!("⚡ Calculating electricity earnings based on SOL value");        
    
    // Query dBTC price from moonbase mining state using CPI
    let cpi_program_query = ctx.accounts.moonbase_program.to_account_info();
    let cpi_accounts_query = moonbase::cpi::accounts::QueryTokenPrices {
        doge_btc_mining: ctx.accounts.doge_btc_mining_state.to_account_info(),
    };
    let cpi_ctx_query = CpiContext::new(cpi_program_query, cpi_accounts_query);
    let token_prices = moonbase::cpi::query_token_prices(cpi_ctx_query)?;
    let dbtc_price_in_sol = token_prices.get().dbtc_price_in_sol;
    
    msg!("   DOGE_BTC price: {} (9-decimal precision)", dbtc_price_in_sol);
    
    // Convert weighted dBTC amount to SOL value
    // weighted_amount is in DOGE_BTC base units (6 decimals)
    // dbtc_price_in_sol is SOL per DOGE_BTC with 9-decimal precision
    // sol_value = (weighted_amount * dbtc_price_in_sol) / 10^6 / 10^9 * 10^9 = (weighted_amount * dbtc_price_in_sol) / 10^6
    let sol_value = (weighted_amount as u128)
        .checked_mul(dbtc_price_in_sol as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(1_000_000) // Divide by 10^6 to convert from DOGE_BTC base units
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .min(u64::MAX as u128) as u64;
    
    msg!("   SOL value of staked weighted DOGE_BTC: {} lamports", sol_value);
    
    // Calculate electricity based on SOL value
    // electricity_per_weighted_sol is configured in global config
    let electricity_increase = sol_value
        .checked_mul(ctx.accounts.global_config.electricity_per_weighted_sol)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(1_000_000_000) // Divide by 10^9 since sol_value is in lamports
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    msg!("   Electricity increase: {}", electricity_increase);

    // Update user's position electricity per day and total electricity earned
    user_position.electricity_per_day += electricity_increase;
    electricity_ac.electricity_earned = electricity_ac.electricity_earned.checked_add(electricity_increase).unwrap();        
    msg!("   Position electricity per day: {}", user_position.electricity_per_day);
    msg!("   Total electricity earned: {}", electricity_ac.electricity_earned);
    msg!("🔌 Calling MoonBase to update user electricity");
    
    helper::update_user_electricity_cpi(
        &ctx.accounts.moonbase_program.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.facility_user_moonbase.to_account_info(),
        &ctx.accounts.facility_mining_state.to_account_info(),
        &ctx.accounts.moonbase_global_config.to_account_info(),
        &ctx.accounts.fee_collector.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.fee_collector,
        true,
        electricity_increase,
    )?;

    msg!("✅ DogeBtc staking successful");    
    emit!(DogeBtcStaked {
        owner: ctx.accounts.authority.key(),
        amount: actual_amount, // Use actual_amount (post-tax) in the event
        lockup_duration,
        multiplier,
        weighted_amount,
        electricity_earned: electricity_increase,
        position_index,
    });
    
    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- UNSTAKE MOONDOGE TOKENS :: User gets DOGE_BTC back ------------------------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Unstake DogeBtc tokens from a position
pub fn unstake_moondoge(ctx: Context<UnstakeDogeBtc>, position_index: u8) -> Result<()> {
    // Get references to all accounts
    let electricity_ac = &mut ctx.accounts.electricity_ac;
    let dogebtc_vault = &mut ctx.accounts.dogebtc_vault;
    let user_position = &mut ctx.accounts.user_position;
    let current_ts = Clock::get()?.unix_timestamp;
    
    msg!("🔓 Processing unstake for position {}", position_index);
    
    // Validate the position exists and has funds
    require!(user_position.staked_amount > 0, ErrorCode::InvalidAmount);
    
    // Verify position index is in the user's active positions
    require!(
        electricity_ac.moondoge_position_indices.contains(&position_index),
        ErrorCode::PositionNotFound
    );
    
    msg!("📊 Position details - Staked: {}, Weighted: {}, Lockup ends: {}", 
        user_position.staked_amount, 
        user_position.weighted_amount,
        user_position.lockup_end_timestamp);
    
    // Process any pending rewards before unstaking
    if electricity_ac.total_weighted_moondoge > 0 {
        msg!("💰 Processing pending rewards before unstaking");
        
        // Calculate reward diff since last update
        let reward_diff = dogebtc_vault.accumulated_sol_per_point.checked_sub(electricity_ac.moondoge_reward_debt).unwrap_or(0);            
        msg!("   Reward diff: {}", reward_diff);
        
        // Calculate new rewards
        let new_rewards = (electricity_ac.total_weighted_moondoge as u128).checked_mul(reward_diff).unwrap_or(0).checked_div(PRECISION_FACTOR).unwrap_or(0) as u64;            
        msg!("   New rewards: {}", new_rewards);
            
        // Add to pending rewards
        electricity_ac.pending_moondoge_rewards = electricity_ac.pending_moondoge_rewards.checked_add(new_rewards).unwrap_or(electricity_ac.pending_moondoge_rewards);            
        msg!("   Updated pending rewards: {}", electricity_ac.pending_moondoge_rewards);
    }
    
    // Update reward debt to current rate
    electricity_ac.moondoge_reward_debt = dogebtc_vault.accumulated_sol_per_point;
    
    // Calculate return amount based on early withdrawal status
    let is_early_withdrawal = current_ts < user_position.lockup_end_timestamp;
    let original_amount = user_position.staked_amount;
    let mut return_amount = original_amount;
    
    // Handle early withdrawal if needed
    if is_early_withdrawal {
        msg!("⚠️ Early unstake detected! Current time: {}, Lockup end: {}", current_ts, user_position.lockup_end_timestamp);
        
        // Calculate remaining lockup days
        let remaining_seconds = user_position.lockup_end_timestamp - current_ts;
        let remaining_seconds_pct = (M_HUNDRED as i64) * remaining_seconds / (user_position.lockup_end_timestamp - user_position.start_timestamp) as i64;
        msg!("   Lockup remaining: {}%", remaining_seconds_pct);
        
        // Apply emergency tax for early withdrawal
        let calc_penalty_pct = (dogebtc_vault.emergency_tax as u64).checked_mul(remaining_seconds_pct as u64).unwrap().checked_div(M_HUNDRED).unwrap();
        msg!("   Emergency tax percentage: {}%", calc_penalty_pct);
        
        // Apply penalty to return amount
        let penalty_amount = original_amount.checked_mul(calc_penalty_pct as u64).unwrap().checked_div(M_HUNDRED).unwrap();
        return_amount = original_amount.checked_sub(penalty_amount).ok_or(ErrorCode::ArithmeticOverflow)?;            
        msg!("   Total Staked: {}, Returned: {}, Penalty: {}", original_amount, return_amount, penalty_amount);
        
        // Burn penalty tokens by sending to dead address
        if penalty_amount > 0 {
            msg!("🔥 Burning {} penalty tokens", penalty_amount);
            
            // Get PDA signer seeds for the dbtc_custodian
            let custodian_authority_seeds = &[
                DBTC_CUSTODIAN_AUTHORITY_SEED.as_ref(),
                &[ctx.bumps.dbtc_custodian_authority]
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
    
    // Update electricity management
    msg!("⚡ Updating electricity");
    
    // Calculate electricity decrease
    let electricity_decrease = user_position.electricity_per_day;
    msg!("   Electricity decrease: {}", electricity_decrease);    
    // Update user's electricity earned
    electricity_ac.electricity_earned = electricity_ac.electricity_earned.checked_sub(electricity_decrease).unwrap_or(0);    
    msg!("   Updated electricity earned: {}", electricity_ac.electricity_earned);
    
    // Update CPI to decrease electricity in MoonFacility
    helper::update_user_electricity_cpi(
        &ctx.accounts.moonbase_program.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.facility_user_moonbase.to_account_info(),
        &ctx.accounts.facility_mining_state.to_account_info(),
        &ctx.accounts.moonbase_global_config.to_account_info(),
        &ctx.accounts.fee_collector.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.fee_collector,
        false,
        electricity_decrease,
    )?;
    
    // Update vault totals
    msg!("📊 Updating vault totals");
    dogebtc_vault.dbtc_locked = dogebtc_vault.dbtc_locked.checked_sub(original_amount).ok_or(ErrorCode::ArithmeticOverflow)?;        
    dogebtc_vault.weighted_dbtc_locked = dogebtc_vault.weighted_dbtc_locked.checked_sub(user_position.weighted_amount).ok_or(ErrorCode::ArithmeticOverflow)?;    
    msg!("   New vault totals - Locked: {}, Weighted: {}", dogebtc_vault.dbtc_locked, dogebtc_vault.weighted_dbtc_locked);
    
    // Update user global stats
    msg!("📊 Updating user stats");
    electricity_ac.total_moondoge_staked = electricity_ac.total_moondoge_staked.checked_sub(original_amount).ok_or(ErrorCode::ArithmeticOverflow)?; 
    electricity_ac.total_weighted_moondoge = electricity_ac.total_weighted_moondoge.checked_sub(user_position.weighted_amount).ok_or(ErrorCode::ArithmeticOverflow)?;    
    msg!("   New user totals - Staked: {}, Weighted: {}",  electricity_ac.total_moondoge_staked,  electricity_ac.total_weighted_moondoge);
    
    // Remove position from user's active positions
    helper::remove_moondoge_position(electricity_ac, position_index)?;
    msg!("   Updated active positions: {}", electricity_ac.active_moondoge_positions);
    
    // Transfer remaining tokens back to user
    if return_amount > 0 {
        msg!("💱 Transferring {} DOGE_BTC tokens to user", return_amount);
        
        // Get PDA signer seeds for the dbtc_custodian
        let custodian_authority_seeds = &[
            DBTC_CUSTODIAN_AUTHORITY_SEED.as_ref(),
            &[ctx.bumps.dbtc_custodian_authority]
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
        
        token_interface::transfer_checked(transfer_ctx, return_amount, ctx.accounts.dbtc_mint.decimals)?;
    }
    
    // Reset position data
    user_position.staked_amount = 0;
    user_position.weighted_amount = 0;
    user_position.electricity_per_day = 0;
    
    emit!(DogeBtcUnstaked {
        owner: ctx.accounts.authority.key(),
        position_index,
        amount: return_amount,
        weighted_amount: user_position.weighted_amount,
        early_withdrawal: is_early_withdrawal,
    });
    
    msg!("✅ Unstake completed successfully");
    Ok(())
}

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// ---- STAKE LIQUIDITY LP TOKENS :: User gets electricity and SOL rewards ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

/// Stake Liquidity LP tokens
pub fn stake_lp_tokens(ctx: Context<StakeLpTokens>, amount: u64, lockup_duration: u64, position_index: u8) -> Result<()> {
    msg!("🔒 Starting LP token staking - Amount: {}, Lockup: {} days, Position: {}", amount, lockup_duration, position_index);
    // Validate inputs
    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(
        lockup_duration >= ctx.accounts.global_config.min_lockup_days && 
        lockup_duration <= ctx.accounts.global_config.max_lockup_days,
        ErrorCode::InvalidLockupPeriod
    );
    
    // Global liquidity vault
    let liquidity_vault = &mut ctx.accounts.liquidity_vault;
    msg!("📊 Current vault state - Total locked: {}, Weighted locked: {}",  liquidity_vault.lp_tokens_locked, liquidity_vault.weighted_lp_locked);

    // User Position :: Electricity account
    let electricity_ac = &mut ctx.accounts.electricity_ac;
    let user_position = &mut ctx.accounts.user_position;
    let current_ts = Clock::get()?.unix_timestamp;
    
    // Initialize owner if this is a new account
    if electricity_ac.owner == Pubkey::default() {
        electricity_ac.owner = ctx.accounts.authority.key();
        electricity_ac.moondoge_position_indices = Vec::with_capacity(MAX_ALLOWED_POSITIONS as usize);
        electricity_ac.lp_position_indices = Vec::with_capacity(MAX_ALLOWED_POSITIONS as usize);
        msg!("👤 Initializing new user electricity account");
    }

    // Add position index to user electricity account
    helper::add_lp_position(electricity_ac, position_index)?;

    // Calculate multiplier based on lockup duration
    let multiplier = helper::calculate_multiplier(
        lockup_duration,
        ctx.accounts.global_config.min_lockup_days,
        ctx.accounts.global_config.max_lockup_days,
        ctx.accounts.global_config.base_multiplier,
        ctx.accounts.global_config.max_multiplier,
    )?;
    msg!("🔢 Multiplier for {} days lockup: {}", lockup_duration, multiplier);    
    
    // Calculate weighted amount for this position
    let mut weighted_amount = amount.checked_mul(multiplier as u64).unwrap().checked_div(M_HUNDRED).unwrap();
    msg!("⚖️ Weighted amount: {} (raw amount: {} × multiplier: {})", weighted_amount, amount, multiplier);
    
    // Process any pending rewards before updating position
    if electricity_ac.total_weighted_lp > 0 {
        msg!("💰 Processing pending rewards before position update");
        msg!("   Previous reward debt: {}", electricity_ac.lp_reward_debt);
                
        // Calculate reward diff since last update
        let reward_diff = liquidity_vault.accumulated_sol_per_point.checked_sub(electricity_ac.lp_reward_debt).unwrap_or(0);
        msg!("   New accumulated sol per point: {}", liquidity_vault.accumulated_sol_per_point);
        msg!("   Reward diff: {}", reward_diff);

        // rewards earned = total weighted LP * accumulated sol per point - reward debt
        let new_rewards = (electricity_ac.total_weighted_lp as u128).checked_mul(reward_diff).unwrap().checked_div(PRECISION_FACTOR).unwrap_or(0) as u64;
        msg!("   New rewards: {}", new_rewards);

        // add rewards to pending rewards
        electricity_ac.pending_lp_rewards = electricity_ac.pending_lp_rewards.checked_add(new_rewards).unwrap();
        msg!("   Updated pending LP rewards: {}", electricity_ac.pending_lp_rewards);
    }
    
    // If position exists, validate and update
    if user_position.staked_amount > 0 {
        msg!("🔄 Updating existing position - Current amount: {}", user_position.staked_amount);
        // Position should be still locked
        require!(user_position.lockup_end_timestamp > current_ts, ErrorCode::PositionStillLocked);        
        // Update existing position
        let old_weighted_amount = user_position.weighted_amount;        
        // Update staked amount
        user_position.staked_amount = user_position.staked_amount.checked_add(amount).ok_or(ErrorCode::ArithmeticOverflow)?;            
        // Update weighted amount - recalculate the total weighted amount for consistency
        user_position.weighted_amount = user_position.staked_amount.checked_mul(multiplier as u64).unwrap().checked_div(M_HUNDRED).unwrap();            
        
        // Calculate the actual weighted amount difference to add to vault
        let weighted_amount_diff = user_position.weighted_amount.checked_sub(old_weighted_amount).unwrap();
        
        // Update user's total LP weighted amount
        electricity_ac.total_weighted_lp = electricity_ac.total_weighted_lp.checked_sub(old_weighted_amount).unwrap()
                                           .checked_add(user_position.weighted_amount).unwrap();            

        msg!("   New staked amount: {}", user_position.staked_amount);
        msg!("   New weighted amount: {}", user_position.weighted_amount);
        msg!("   Weighted amount diff: {}", weighted_amount_diff);
        msg!("   New total weighted: {}", electricity_ac.total_weighted_lp);
        
        // Use the actual difference for vault updates and electricity calculations
        weighted_amount = weighted_amount_diff;
    } else {
        msg!("🆕 Creating new position {}", position_index);
        // Initialize new position
        user_position.position_index = position_index;
        user_position.staked_amount = amount;
        user_position.weighted_amount = weighted_amount;
        user_position.start_timestamp = current_ts;
        user_position.multiplier = multiplier;
        user_position.lockup_duration = lockup_duration;
        
        // Calculate lockup end timestamp
        let seconds_to_add = lockup_duration.checked_mul(DAY_IN_SECONDS).unwrap();
        user_position.lockup_end_timestamp = current_ts.checked_add(seconds_to_add as i64).unwrap();

        msg!("   Lockup end: {} (current: {})", user_position.lockup_end_timestamp, current_ts);
                
        // Update user's total LP weighted amount
        electricity_ac.total_weighted_lp = electricity_ac.total_weighted_lp.checked_add(weighted_amount).unwrap();

        msg!("   Active positions: {}", electricity_ac.active_lp_positions);
        msg!("   Total weighted LP: {}", electricity_ac.total_weighted_lp);        
    }
    
    // Update global user state & Global reward debt
    electricity_ac.total_lp_tokens_staked = electricity_ac.total_lp_tokens_staked.checked_add(amount).unwrap();    
    electricity_ac.lp_reward_debt = liquidity_vault.accumulated_sol_per_point;
    
    msg!("💱 Transferring {} LP tokens from user to vault", amount);
    msg!("   From: {}", ctx.accounts.user_lp_account.key());
    msg!("   To: {}", ctx.accounts.liquidity_custodian.key());

    // Transfer LP tokens from user to liquidity vault
    let transfer_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        token::Transfer {
            from: ctx.accounts.user_lp_account.to_account_info(),
            to: ctx.accounts.liquidity_custodian.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        },
    );
    token::transfer(transfer_ctx, amount)?;
    
    // Update LP vault state
    liquidity_vault.lp_tokens_locked = liquidity_vault.lp_tokens_locked.checked_add(amount).unwrap();        
    liquidity_vault.weighted_lp_locked = liquidity_vault.weighted_lp_locked.checked_add(weighted_amount).unwrap();

    msg!("⚡ Calculating electricity earnings based on SOL value with 50% LP bonus");        
    
    // Query LP token price from moonbase mining state using CPI
    let cpi_program_query = ctx.accounts.moonbase_program.to_account_info();
    let cpi_accounts_query = moonbase::cpi::accounts::QueryTokenPrices {
        doge_btc_mining: ctx.accounts.doge_btc_mining_state.to_account_info(),
    };
    let cpi_ctx_query = CpiContext::new(cpi_program_query, cpi_accounts_query);
    let token_prices = moonbase::cpi::query_token_prices(cpi_ctx_query)?;
    let lp_token_price_in_sol = token_prices.get().lp_token_price_in_sol;
    
    msg!("   LP token price: {} (9-decimal precision)", lp_token_price_in_sol);
    
    // Convert weighted LP amount to SOL value
    // weighted_amount is in LP token base units (9 decimals, same as standard SPL)
    // lp_token_price_in_sol is SOL per LP token with 9-decimal precision
    // sol_value = (weighted_amount * lp_token_price_in_sol) / 10^9 / 10^9 * 10^9 = (weighted_amount * lp_token_price_in_sol) / 10^9
    let sol_value = (weighted_amount as u128)
        .checked_mul(lp_token_price_in_sol as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(1_000_000_000) // Divide by 10^9 to convert from LP base units
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .min(u64::MAX as u128) as u64;
    
    msg!("   SOL value of staked weighted LP: {} lamports", sol_value);
    
    // Calculate base electricity based on SOL value
    let base_electricity = sol_value
        .checked_mul(ctx.accounts.global_config.electricity_per_weighted_sol)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(1_000_000_000) // Divide by 10^9 since sol_value is in lamports
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Apply 50% bonus for LP staking: multiply by 150 and divide by 100
    let electricity_increase = base_electricity
        .checked_mul(150)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    msg!("   Base electricity: {}, with 50% LP bonus: {}", base_electricity, electricity_increase);

    // Update user's position electricity per day and total electricity earned
    user_position.electricity_per_day += electricity_increase;
    electricity_ac.electricity_earned = electricity_ac.electricity_earned.checked_add(electricity_increase).unwrap();        
    msg!("   Position electricity per day: {}", user_position.electricity_per_day);
    msg!("   Total electricity earned: {}", electricity_ac.electricity_earned);
    msg!("🔌 Calling MoonBase to update user electricity");
    
    helper::update_user_electricity_cpi(
        &ctx.accounts.moonbase_program.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.facility_user_moonbase.to_account_info(),
        &ctx.accounts.facility_mining_state.to_account_info(),
        &ctx.accounts.moonbase_global_config.to_account_info(),
        &ctx.accounts.fee_collector.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.fee_collector,
        true,
        electricity_increase,
    )?;

    msg!("✅ LP token staking successful");    
    emit!(LiquidityStaked {
        owner: ctx.accounts.authority.key(),
        amount,
        lockup_duration,
        multiplier,
        weighted_amount,
        electricity_earned: electricity_increase,
        position_index,
    });
    
    Ok(())
}

/// Unstake Liquidity LP tokens from a position
pub fn unstake_lp_tokens(ctx: Context<UnstakeLpTokens>, position_index: u8) -> Result<()> {
    // Get references to all accounts
    let electricity_ac = &mut ctx.accounts.electricity_ac;
    let liquidity_vault = &mut ctx.accounts.liquidity_vault;
    let user_position = &mut ctx.accounts.user_position;
    let current_ts = Clock::get()?.unix_timestamp;
    
    msg!("🔓 Processing unstake for position {}", position_index);
    
    // Validate the position exists and has funds
    require!(user_position.staked_amount > 0, ErrorCode::InvalidAmount);
    
    // Verify position index is in the user's active positions
    require!(
        electricity_ac.lp_position_indices.contains(&position_index),
        ErrorCode::PositionNotFound
    );
    
    msg!("📊 Position details - Staked: {}, Weighted: {}, Lockup ends: {}", 
        user_position.staked_amount, 
        user_position.weighted_amount,
        user_position.lockup_end_timestamp);
    
    // Process any pending rewards before unstaking
    if electricity_ac.total_weighted_lp > 0 {
        msg!("💰 Processing pending rewards before unstaking");
        
        // Calculate reward diff since last update
        let reward_diff = liquidity_vault.accumulated_sol_per_point.checked_sub(electricity_ac.lp_reward_debt).unwrap_or(0);            
        msg!("   Reward diff: {}", reward_diff);
        
        // Calculate new rewards
        let new_rewards = (electricity_ac.total_weighted_lp as u128)
            .checked_mul(reward_diff)
            .unwrap_or(0)
            .checked_div(PRECISION_FACTOR)
            .unwrap_or(0) as u64;            
        msg!("   New rewards: {}", new_rewards);
            
        // Add to pending rewards
        electricity_ac.pending_lp_rewards = electricity_ac.pending_lp_rewards
            .checked_add(new_rewards)
            .unwrap_or(electricity_ac.pending_lp_rewards);            
        msg!("   Updated pending rewards: {}", electricity_ac.pending_lp_rewards);
    }
    
    // Update reward debt to current rate
    electricity_ac.lp_reward_debt = liquidity_vault.accumulated_sol_per_point;
    
    // Calculate return amount based on early withdrawal status
    let is_early_withdrawal = current_ts < user_position.lockup_end_timestamp;
    let original_amount = user_position.staked_amount;
    let mut return_amount = original_amount;
    
    // Handle early withdrawal if needed - fixed 10% penalty
    if is_early_withdrawal {
        msg!("⚠️ Early unstake detected! Current time: {}, Lockup end: {}", current_ts, user_position.lockup_end_timestamp);
       
        // Calculate remaining lockup days
        let remaining_seconds = user_position.lockup_end_timestamp - current_ts;
        let remaining_seconds_pct = (M_HUNDRED as i64) * remaining_seconds / (user_position.lockup_end_timestamp - user_position.start_timestamp) as i64;
        msg!("   Lockup remaining: {}%", remaining_seconds_pct);   
        
        // Apply emergency tax for early withdrawal
        let calc_penalty_pct = (liquidity_vault.emergency_tax as u64).checked_mul(remaining_seconds_pct as u64).unwrap().checked_div(M_HUNDRED).unwrap();
        msg!("   Emergency tax percentage: {}%", calc_penalty_pct);

        // Apply penalty to return amount
        let penalty_amount = original_amount.checked_mul(calc_penalty_pct).unwrap().checked_div(M_HUNDRED).unwrap();
        return_amount = original_amount.checked_sub(penalty_amount).ok_or(ErrorCode::ArithmeticOverflow)?;            
        msg!("   Total Staked: {}, Returned: {}, Penalty: {}", original_amount, return_amount, penalty_amount);
        
        // If early unstake, send penalty tokens to treasury
        if penalty_amount > 0 {
            msg!("💸 Sending {} penalty tokens to treasury", penalty_amount);
            
            // Get PDA signer seeds for the liquidity vault
            let custodian_authority_seeds = &[
                LIQUIDITY_CUSTODIAN_AUTHORITY_SEED.as_ref(),
                &[ctx.bumps.liquidity_custodian_authority]
            ];
            let signer = &[&custodian_authority_seeds[..]];
            
            // Burn penalty tokens from liquidity custodian
            let burn_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Burn {
                    mint: ctx.accounts.lp_mint.to_account_info(), // Mint of LP token
                    from: ctx.accounts.liquidity_custodian.to_account_info(), // Token account to burn from
                    authority: ctx.accounts.liquidity_custodian_authority.to_account_info(), // PDA authority
                },
                signer,
            );
            token::burn(burn_ctx, penalty_amount)?;
            
            // Emit early withdrawal event
            emit!(EarlyLiquidityUnstakePenalty {
                owner: ctx.accounts.authority.key(),
                position_index,
                penalty_amount,
                penalty_tax_pct: calc_penalty_pct,
                return_amount,
                timestamp: current_ts,
            });
        }
    } else {
        msg!("✅ Normal unstake - lockup period has ended");
    }
    
    // Update electricity management
    msg!("⚡ Updating electricity");
    
    // Calculate electricity decrease
    let electricity_decrease = user_position.electricity_per_day;
    msg!("   Electricity decrease: {}", electricity_decrease);    
    // Update user's electricity earned
    electricity_ac.electricity_earned = electricity_ac.electricity_earned.checked_sub(electricity_decrease).unwrap_or(0);
    
    // Update CPI to decrease electricity in MoonFacility
    helper::update_user_electricity_cpi(
        &ctx.accounts.moonbase_program.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.facility_user_moonbase.to_account_info(),
        &ctx.accounts.facility_mining_state.to_account_info(),
        &ctx.accounts.moonbase_global_config.to_account_info(),
        &ctx.accounts.fee_collector.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.fee_collector,
        false,
        electricity_decrease,
    )?;
    
    // Update vault totals
    msg!("📊 Updating vault totals");
    liquidity_vault.lp_tokens_locked = liquidity_vault.lp_tokens_locked.checked_sub(original_amount).ok_or(ErrorCode::ArithmeticOverflow)?;        
    liquidity_vault.weighted_lp_locked = liquidity_vault.weighted_lp_locked.checked_sub(user_position.weighted_amount).ok_or(ErrorCode::ArithmeticOverflow)?;    
    msg!("   New vault totals - Locked: {}, Weighted: {}", liquidity_vault.lp_tokens_locked, liquidity_vault.weighted_lp_locked);
    
    // Update user global stats
    msg!("📊 Updating user stats");
    electricity_ac.total_lp_tokens_staked = electricity_ac.total_lp_tokens_staked.checked_sub(original_amount).ok_or(ErrorCode::ArithmeticOverflow)?; 
    electricity_ac.total_weighted_lp = electricity_ac.total_weighted_lp.checked_sub(user_position.weighted_amount).ok_or(ErrorCode::ArithmeticOverflow)?;    
    msg!("   New user totals - Staked: {}, Weighted: {}", electricity_ac.total_lp_tokens_staked, electricity_ac.total_weighted_lp);
    
    // Remove position from user's active positions
    helper::remove_lp_position(electricity_ac, position_index)?;
    msg!("   Updated active positions: {}", electricity_ac.active_lp_positions);
    
    // Transfer remaining tokens back to user
    if return_amount > 0 {
        msg!("💱 Transferring {} LP tokens to user", return_amount);
        
        // Get PDA signer seeds for the liquidity_custodian
        let custodian_authority_seeds = &[
            LIQUIDITY_CUSTODIAN_AUTHORITY_SEED.as_ref(),
            &[ctx.bumps.liquidity_custodian_authority]
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
    user_position.electricity_per_day = 0;
    
    emit!(LiquidityUnstaked {
        owner: ctx.accounts.authority.key(),
        position_index,
        amount: return_amount,
        weighted_amount: user_position.weighted_amount,
        early_withdrawal: is_early_withdrawal,
    });
    
    msg!("✅ Unstake completed successfully");
    Ok(())
}


// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- --------- ---------
// ---- CLAIM SOL REWARDS :: User earns SOL rewards from staking mDoge and LP tokens ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- --------- ---------

/// Claim SOL rewards from staking mDoge and LP tokens
pub fn claim_sol_rewards(ctx: Context<ClaimSolRewards>) -> Result<()> {
    msg!("🔒 Starting claim SOL rewards");

    // Global moondoge vault
    let dogebtc_vault = &mut ctx.accounts.dogebtc_vault;
    let liquidity_vault = &mut ctx.accounts.liquidity_vault;

    // User Position :: Electricity account
    let electricity_ac = &mut ctx.accounts.electricity_ac;
      
    // Process any pending SOL rewards before claim (DOGE_BTC staking)
    if electricity_ac.total_weighted_moondoge > 0 {
        msg!("💰 Processing pending DOGE_BTC rewards before claim");
                
        // Calculate reward diff since last update
        let reward_diff = dogebtc_vault.accumulated_sol_per_point.checked_sub(electricity_ac.moondoge_reward_debt).unwrap_or(0);
        msg!("   Previous reward debt: {}", electricity_ac.moondoge_reward_debt);
        msg!("   New accumulated sol per point: {}", dogebtc_vault.accumulated_sol_per_point);
        msg!("   Reward diff: {}", reward_diff);

        // rewards earned = total weighted moondoge * accumulated sol per point - reward debt
        let new_rewards = (electricity_ac.total_weighted_moondoge as u128)
            .checked_mul(reward_diff)
            .unwrap_or(0)
            .checked_div(PRECISION_FACTOR)
            .unwrap_or(0) as u64;
        msg!("   New rewards: {}", new_rewards);

        // add rewards to pending rewards
        electricity_ac.pending_moondoge_rewards = electricity_ac.pending_moondoge_rewards.checked_add(new_rewards).unwrap();
        msg!("   Updated pending DOGE_BTC rewards: {}", electricity_ac.pending_moondoge_rewards);
    }

    // Process any pending rewards before claim (LP staking)
    if electricity_ac.total_weighted_lp > 0 {
        msg!("💰 Processing pending LP rewards before claim");

        let reward_diff = liquidity_vault.accumulated_sol_per_point.checked_sub(electricity_ac.lp_reward_debt).unwrap_or(0);
        msg!("   Previous reward debt: {}", electricity_ac.lp_reward_debt);
        msg!("   New accumulated sol per point: {}", liquidity_vault.accumulated_sol_per_point);
        msg!("   Reward diff: {}", reward_diff);

        let new_rewards = (electricity_ac.total_weighted_lp as u128)
            .checked_mul(reward_diff)
            .unwrap_or(0)
            .checked_div(PRECISION_FACTOR)
            .unwrap_or(0) as u64;            
        msg!("   New rewards: {}", new_rewards);

        // Add pending LP rewards to total rewards
        electricity_ac.pending_lp_rewards = electricity_ac.pending_lp_rewards.checked_add(new_rewards).unwrap();
        msg!("   Updated pending LP rewards: {}", electricity_ac.pending_lp_rewards);
    }

    // Update reward debt to current rate
    electricity_ac.moondoge_reward_debt = dogebtc_vault.accumulated_sol_per_point;
    electricity_ac.lp_reward_debt = liquidity_vault.accumulated_sol_per_point;
    
    // Transfer pending rewards to user
    let receiver = &ctx.accounts.authority;

    // Transfer DOGE_BTC staking rewards to user
    **ctx.accounts.dbtc_sol_vault.try_borrow_mut_lamports()? -= electricity_ac.pending_moondoge_rewards;
    **receiver.try_borrow_mut_lamports()? += electricity_ac.pending_moondoge_rewards;

    // Transfer LP staking rewards to user
    **ctx.accounts.liquidity_sol_vault.try_borrow_mut_lamports()? -= electricity_ac.pending_lp_rewards;
    **receiver.try_borrow_mut_lamports()? += electricity_ac.pending_lp_rewards;

    emit!(SolRewardsClaimed {
        owner: ctx.accounts.authority.key(),
        moondoge_rewards: electricity_ac.pending_moondoge_rewards,  
        lp_rewards: electricity_ac.pending_lp_rewards,
    });

    // Add pending rewards to total rewards
    electricity_ac.total_sol_claimed = electricity_ac.total_sol_claimed.checked_add(electricity_ac.pending_moondoge_rewards).unwrap();
    electricity_ac.total_sol_claimed = electricity_ac.total_sol_claimed.checked_add(electricity_ac.pending_lp_rewards).unwrap();

    // Reset pending rewards to 0
    electricity_ac.pending_moondoge_rewards = 0;
    electricity_ac.pending_lp_rewards = 0;
     


    msg!("✅ Claimed SOL rewards");        
    Ok(())
}

/// Update user's pending SOL rewards calculation
/// This function recalculates pending rewards based on current vault state
pub fn update_pending_rewards(ctx: Context<UpdatePendingRewards>) -> Result<()> {
    let electricity_ac = &mut ctx.accounts.electricity_ac;
    let dogebtc_vault = &ctx.accounts.dogebtc_vault;
    let liquidity_vault = &ctx.accounts.liquidity_vault;
    
    msg!("📊 Updating pending rewards for user: {}", ctx.accounts.authority.key());
    
    // Update pending DogeBtc rewards
    if electricity_ac.total_weighted_moondoge > 0 {
        let pending_moondoge = (electricity_ac.total_weighted_moondoge as u128 * dogebtc_vault.accumulated_sol_per_point / PRECISION_FACTOR as u128) as u64;
        electricity_ac.pending_moondoge_rewards = pending_moondoge.saturating_sub(electricity_ac.moondoge_reward_debt as u64);
        msg!("💰 Updated pending DogeBtc rewards: {}", electricity_ac.pending_moondoge_rewards);
    } else {
        electricity_ac.pending_moondoge_rewards = 0;
    }
    
    // Update pending LP rewards
    if electricity_ac.total_weighted_lp > 0 {
        let pending_lp = (electricity_ac.total_weighted_lp as u128 * liquidity_vault.accumulated_sol_per_point / PRECISION_FACTOR as u128) as u64;
        electricity_ac.pending_lp_rewards = pending_lp.saturating_sub(electricity_ac.lp_reward_debt as u64);
        msg!("💰 Updated pending LP rewards: {}", electricity_ac.pending_lp_rewards);
    } else {
        electricity_ac.pending_lp_rewards = 0;
    }
    
    msg!("✅ Pending rewards update completed successfully");
    
    Ok(())
}





// ----------------------------------------------------------------------------------------
// ------------ ACCOUNT STRUCTS ----------------------------------------------------------
// ----------------------------------------------------------------------------------------


// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --------- CREATE ELECTRICITY ACCOUNT ---------
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

#[derive(Accounts)]
pub struct InitializeElectricityAc<'info> {
    // Global config and vaults
    #[account(
        seeds = [ME_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    // User accounts
    #[account(
        init,
        payer = authority,
        space = UserMoonElectricity::LEN,
        seeds = [USER_ELECTRICITY_SEED, authority.key().as_ref()],
        bump
    )]
    pub electricity_ac: Account<'info, UserMoonElectricity>,
     
    #[account(mut)]
    /// CHECK: User instance in MoonFacility - will be updated via CPI
    pub facility_user_moonbase: UncheckedAccount<'info>,
    
    /// CHECK: MoonFacility mining state
    pub facility_mining_state: UncheckedAccount<'info>,
    
    /// CHECK: MoonFacility global config
    pub moonbase_global_config: UncheckedAccount<'info>,
    
    /// Fee collector PDA for CPI authority
    #[account(
        seeds = [b"fee_collector"],
        bump
    )]
    /// CHECK: Fee collector PDA
    pub fee_collector: UncheckedAccount<'info>,
    
    /// MoonBase program for CPI
    /// CHECK: MoonBase program
    pub moonbase_program: UncheckedAccount<'info>,

    /// User who is initializing electricity account
    #[account(mut)]
    pub authority: Signer<'info>,
    
    /// System program for creating accounts
    pub system_program: Program<'info, System>,    
}


// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --------- STAKE MOONDOGE ---------
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

#[derive(Accounts)]
#[instruction(amount: u64, lockup_duration: u64, position_index: u8)]
pub struct StakeDogeBtc<'info> {
    // Global config and vaults
    #[account(
        seeds = [ME_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        mut,
        seeds = [DOGE_BTC_VAULT_SEED.as_ref()],
        bump = dogebtc_vault.bump
    )]
    pub dogebtc_vault: Account<'info, DogeBtcVault>,
    
    // User accounts
    #[account(
        init_if_needed,
        payer = authority,
        space = UserMoonElectricity::LEN,
        seeds = [USER_ELECTRICITY_SEED, authority.key().as_ref()],
        bump
    )]
    pub electricity_ac: Account<'info, UserMoonElectricity>,
    
    #[account(
        init_if_needed,
        payer = authority,
        space = DogeBtcPosition::LEN,
        seeds = [
            DBTC_POSITION_SEED,
            authority.key().as_ref(),
            &[position_index]
        ],
        bump
    )]
    pub user_position: Account<'info, DogeBtcPosition>,
    
    /// CHECK: DOGE_BTC Mint
    pub dbtc_mint: InterfaceAccount<'info, Mint2022>,

    // Token accounts
    #[account(
        mut,
        constraint = user_dbtc_account.mint == dogebtc_vault.dbtc_mint @ ErrorCode::InvalidTokenMint,
        constraint = user_dbtc_account.owner == authority.key() @ ErrorCode::InvalidTokenOwner,
        constraint = user_dbtc_account.amount >= amount @ ErrorCode::InsufficientFunds,
    )]
    /// User's DogeBtc token account
    pub user_dbtc_account: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        mut,
        seeds = [DBTC_CUSTODIAN_SEED.as_ref(), dogebtc_vault.key().as_ref()],
        bump,
        constraint = dbtc_custodian.mint == dogebtc_vault.dbtc_mint @ ErrorCode::InvalidTokenMint,
    )]
    /// Token-2022 account that holds staked DOGE_BTC
    pub dbtc_custodian: InterfaceAccount<'info, TokenAccount2022>,
        
    //MoonBase accounts
    /// MoonBase global configuration
    #[account(constraint = *moonbase_global_config.owner == moonbase_program.key() @ ErrorCode::InvalidProgramOwner)]
    /// CHECK: Verified in CPI
    pub moonbase_global_config: UncheckedAccount<'info>,
    
    #[account(mut)]
    /// CHECK: User instance in MoonFacility
    pub facility_user_moonbase: UncheckedAccount<'info>,
    
    #[account(mut)]
    /// CHECK: Mining state in MoonFacility
    pub facility_mining_state: UncheckedAccount<'info>,
    
    /// CHECK: DogeBtc mining state for querying token prices
    #[account(constraint = *doge_btc_mining_state.owner == moonbase_program.key() @ ErrorCode::InvalidProgramOwner)]
    pub doge_btc_mining_state: UncheckedAccount<'info>,
    
    #[account(
        mut,
        seeds = [FEE_COLLECTOR_SEED.as_ref()],
        bump,
    )]
    /// CHECK: Used for CPI to MoonFacility
    pub fee_collector: UncheckedAccount<'info>,
    
    /// MoonBase program that handles mining operations
    pub moonbase_program: Program<'info, moonbase::program::Moonbase>,
    
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
    // Global config and vaults
    #[account(
        seeds = [ME_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        mut,
        seeds = [DOGE_BTC_VAULT_SEED.as_ref()],
        bump = dogebtc_vault.bump
    )]
    pub dogebtc_vault: Account<'info, DogeBtcVault>,
    
    // User accounts
    #[account(
        mut,
        seeds = [USER_ELECTRICITY_SEED, authority.key().as_ref()],
        bump
    )]
    pub electricity_ac: Account<'info, UserMoonElectricity>,
    
    #[account(
        mut,
        seeds = [
            DBTC_POSITION_SEED,
            authority.key().as_ref(),
            &[position_index]
        ],
        bump,
        constraint = user_position.position_index == position_index
    )]
    pub user_position: Account<'info, DogeBtcPosition>,
    
    /// CHECK: DOGE_BTC Mint
    #[account(mut)]
    pub dbtc_mint: InterfaceAccount<'info, Mint2022>,

    // Token accounts
    #[account(
        mut,
        constraint = user_dbtc_account.owner == authority.key() @ ErrorCode::InvalidTokenOwner,
        constraint = user_dbtc_account.mint == dogebtc_vault.dbtc_mint @ ErrorCode::InvalidTokenMint,
    )]
    /// User's DogeBtc token account to receive the unstaked tokens
    pub user_dbtc_account: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        mut,
        seeds = [DBTC_CUSTODIAN_SEED.as_ref(), dogebtc_vault.key().as_ref()],
        bump,
        constraint = dbtc_custodian.mint == dogebtc_vault.dbtc_mint @ ErrorCode::InvalidTokenMint,
    )]
    /// Token-2022 account that holds staked DOGE_BTC
    pub dbtc_custodian: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        seeds = [DBTC_CUSTODIAN_AUTHORITY_SEED.as_ref()],
        bump,
    )]
    /// Authority of the custodian
    /// CHECK: This is a PDA that acts as the authority for the token account
    pub dbtc_custodian_authority: UncheckedAccount<'info>,
    
    // MoonBase accounts
    /// MoonBase global configuration
    #[account(constraint = *moonbase_global_config.owner == moonbase_program.key() @ ErrorCode::InvalidProgramOwner)]
    /// CHECK: Verified in CPI
    pub moonbase_global_config: UncheckedAccount<'info>,
    
    #[account(mut)]
    /// CHECK: User instance in MoonFacility
    pub facility_user_moonbase: UncheckedAccount<'info>,
    
    #[account(mut)]
    /// CHECK: Mining state in MoonFacility
    pub facility_mining_state: UncheckedAccount<'info>,
    
    #[account(
        mut,
        seeds = [FEE_COLLECTOR_SEED.as_ref()],
        bump,
    )]
    /// CHECK: Used for CPI to MoonFacility
    pub fee_collector: UncheckedAccount<'info>,
    
    /// MoonBase program that handles mining operations
    pub moonbase_program: Program<'info, moonbase::program::Moonbase>,
    
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
    // Global config and vaults
    #[account(
        seeds = [ME_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        mut,
        seeds = [LIQUIDITY_VAULT_SEED.as_ref()],
        bump = liquidity_vault.bump
    )]
    pub liquidity_vault: Account<'info, LiquidityVault>,
    
    // User accounts
    #[account(
        init_if_needed,
        payer = authority,
        space = UserMoonElectricity::LEN,
        seeds = [USER_ELECTRICITY_SEED, authority.key().as_ref()],
        bump
    )]
    pub electricity_ac: Account<'info, UserMoonElectricity>,
    
    #[account(
        init_if_needed,
        payer = authority,
        space = LiquidityPosition::LEN,
        seeds = [
            LP_POSITION_SEED,
            authority.key().as_ref(),
            &[position_index]
        ],
        bump
    )]
    pub user_position: Account<'info, LiquidityPosition>,
    
    // Token accounts
    #[account(
        mut,
        constraint = user_lp_account.mint == liquidity_vault.lp_token_mint @ ErrorCode::InvalidTokenMint,
        constraint = user_lp_account.owner == authority.key() @ ErrorCode::InvalidTokenOwner,
        constraint = user_lp_account.amount >= amount @ ErrorCode::InsufficientFunds,
    )]
    /// User's LP token account
    pub user_lp_account: Account<'info, token::TokenAccount>,
    
    #[account(
        mut,
        seeds = [LIQUIDITY_CUSTODIAN_SEED.as_ref(), liquidity_vault.key().as_ref()],
        bump,
        constraint = liquidity_custodian.mint == liquidity_vault.lp_token_mint @ ErrorCode::InvalidTokenMint,
    )]
    /// Token account that holds staked LP tokens
    pub liquidity_custodian: Account<'info, token::TokenAccount>,
        
    //MoonBase accounts
    /// MoonBase global configuration
    #[account(constraint = *moonbase_global_config.owner == moonbase_program.key() @ ErrorCode::InvalidProgramOwner)]
    /// CHECK: Verified in CPI
    pub moonbase_global_config: UncheckedAccount<'info>,
    
    #[account(mut)]
    /// CHECK: User instance in MoonFacility
    pub facility_user_moonbase: UncheckedAccount<'info>,
    
    #[account(mut)]
    /// CHECK: Mining state in MoonFacility
    pub facility_mining_state: UncheckedAccount<'info>,
    
    /// CHECK: DogeBtc mining state for querying token prices
    #[account(constraint = *doge_btc_mining_state.owner == moonbase_program.key() @ ErrorCode::InvalidProgramOwner)]
    pub doge_btc_mining_state: UncheckedAccount<'info>,
    
    #[account(
        mut,
        seeds = [FEE_COLLECTOR_SEED.as_ref()],
        bump,
    )]
    /// CHECK: Used for CPI to MoonFacility
    pub fee_collector: UncheckedAccount<'info>,
    
    /// MoonBase program that handles mining operations
    pub moonbase_program: Program<'info, moonbase::program::Moonbase>,
    
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
    // Global config and vaults
    #[account(
        seeds = [ME_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        mut,
        seeds = [LIQUIDITY_VAULT_SEED.as_ref()],
        bump = liquidity_vault.bump
    )]
    pub liquidity_vault: Account<'info, LiquidityVault>,
    
    // User accounts
    #[account(
        mut,
        seeds = [USER_ELECTRICITY_SEED, authority.key().as_ref()],
        bump
    )]
    pub electricity_ac: Account<'info, UserMoonElectricity>,
    
    #[account(
        mut,
        seeds = [
            LP_POSITION_SEED,
            authority.key().as_ref(),
            &[position_index]
        ],
        bump,
        constraint = user_position.position_index == position_index
    )]
    pub user_position: Account<'info, LiquidityPosition>,
    
    // Token accounts
    #[account(
        mut,
        constraint = user_lp_account.owner == authority.key() @ ErrorCode::InvalidTokenOwner,
        constraint = user_lp_account.mint == liquidity_vault.lp_token_mint @ ErrorCode::InvalidTokenMint,
    )]
    /// User's LP token account to receive the unstaked tokens
    pub user_lp_account: Account<'info, token::TokenAccount>,
    
    #[account(
        mut,
        seeds = [LIQUIDITY_CUSTODIAN_SEED.as_ref(), liquidity_vault.key().as_ref()],
        bump,
        constraint = liquidity_custodian.mint == liquidity_vault.lp_token_mint @ ErrorCode::InvalidTokenMint,
    )]
    /// Token account that holds staked LP tokens
    pub liquidity_custodian: Account<'info, token::TokenAccount>,
    
    #[account(
        seeds = [LIQUIDITY_CUSTODIAN_AUTHORITY_SEED.as_ref()],
        bump,
    )]
    /// Authority of the custodian
    /// CHECK: This is a PDA that acts as the authority for the token account
    pub liquidity_custodian_authority: UncheckedAccount<'info>,
    
    /// LP Mint
    #[account(mut)]
    pub lp_mint: Account<'info, token::Mint>,
    
    // MoonBase accounts
    /// MoonBase global configuration
    #[account(constraint = *moonbase_global_config.owner == moonbase_program.key() @ ErrorCode::InvalidProgramOwner)]
    /// CHECK: Verified in CPI
    pub moonbase_global_config: UncheckedAccount<'info>,
    
    #[account(mut)]
    /// CHECK: User instance in MoonFacility
    pub facility_user_moonbase: UncheckedAccount<'info>,
    
    #[account(mut)]
    /// CHECK: Mining state in MoonFacility
    pub facility_mining_state: UncheckedAccount<'info>,
    
    #[account(
        mut,
        seeds = [FEE_COLLECTOR_SEED.as_ref()],
        bump,
    )]
    /// CHECK: Used for CPI to MoonFacility
    pub fee_collector: UncheckedAccount<'info>,
    
    /// MoonBase program that handles mining operations
    pub moonbase_program: Program<'info, moonbase::program::Moonbase>,
    
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
    // Global config and vaults
    #[account(
        seeds = [ME_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [DOGE_BTC_VAULT_SEED.as_ref()],
        bump 
    )]
    pub dogebtc_vault: Account<'info, DogeBtcVault>,
    
    #[account(
        mut,
        seeds = [LIQUIDITY_VAULT_SEED.as_ref()],
        bump 
    )]
    pub liquidity_vault: Account<'info, LiquidityVault>,
    
    // User accounts
    #[account(
        mut,
        seeds = [USER_ELECTRICITY_SEED, authority.key().as_ref()],
        bump
    )]
    pub electricity_ac: Account<'info, UserMoonElectricity>,
    
    #[account(
        mut,
        seeds = [DBTC_SOL_VAULT_SEED.as_ref()],
        bump
    )]
    /// CHECK: This is the PDA that custodies SOL for DogeBtc stakers
    pub dbtc_sol_vault: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [LP_SOL_VAULT_SEED.as_ref()],
        bump
    )]
    /// CHECK: This is the PDA that custodies SOL for LP stakers
    pub liquidity_sol_vault: UncheckedAccount<'info>,

    /// User who is unstaking tokens
    #[account(mut)]
    pub authority: Signer<'info>
}

/// Account struct for updating pending rewards
#[derive(Accounts)]
pub struct UpdatePendingRewards<'info> {
    // Global config and vaults
    #[account(
        seeds = [ME_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        seeds = [DOGE_BTC_VAULT_SEED.as_ref()],
        bump 
    )]
    pub dogebtc_vault: Account<'info, DogeBtcVault>,
    
    #[account(
        seeds = [LIQUIDITY_VAULT_SEED.as_ref()],
        bump 
    )]
    pub liquidity_vault: Account<'info, LiquidityVault>,
    
    // User accounts
    #[account(
        mut,
        seeds = [USER_ELECTRICITY_SEED, authority.key().as_ref()],
        bump
    )]
    pub electricity_ac: Account<'info, UserMoonElectricity>,

    /// User requesting the rewards update
    #[account(mut)]
    pub authority: Signer<'info>
}