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
pub fn stake_moondoge(
    ctx: Context<StakeDogeBtc>,
    faction_id: u8,
    amount: u64,
    lockup_duration: u64,
    position_index: u8,
) -> Result<()> {
    let hashpower_config = &ctx.accounts.hashpower_config;
    msg!(
        "🔒 Starting DogeBtc staking - Amount: {}, Lockup: {} days, Position: {}",
        amount,
        lockup_duration,
        position_index
    );
    // Validate inputs
    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(
        lockup_duration >= hashpower_config.min_lockup_days
            && lockup_duration <= hashpower_config.max_lockup_days,
        ErrorCode::InvalidLockupPeriod
    );

    
    
    // Calculate actual amount after burn tax
    let burn_amount = amount* BURN_TAX_PERCENTAGE / M_HUNDRED;
    let actual_amount = amount - burn_amount;
    
    msg!("🔥 mDoge burn tax: {}% - Amount: {}, Burn: {}, Actual amount: {}", BURN_TAX_PERCENTAGE, amount, burn_amount, actual_amount);  
    
    // Global moondoge vault
    let faction_ac = &mut ctx.accounts.faction_ac;
    msg!("📊 Current vault state - Total locked: {}, Weighted locked: {}", faction_ac.dbtc_locked, faction_ac.total_dbtc_hashpower);
    assert!(faction_ac.faction_id == faction_id, ErrorCode::InvalidFactionId);

    // User Position :: Electricity account
    let player_ac = &mut ctx.accounts.player_ac;
    let user_position = &mut ctx.accounts.user_position;
    let current_ts = Clock::get()?.unix_timestamp;
    
    // Initialize owner if this is a new account
    assert!(player_ac.owner ==  ctx.accounts.authority.key(), ErrorCode::InvalidOwner);

    // Add position index to user electricity account
    helper::add_dogebtc_position(player_ac, position_index)?;

    // Calculate multiplier based on lockup duration
    let multiplier = helper::calculate_multiplier(lockup_duration, hashpower_config.min_lockup_days, hashpower_config.max_lockup_days, hashpower_config.base_multiplier, hashpower_config.max_multiplier)?;
    msg!( "🔢 Multiplier for {} days lockup: {}", lockup_duration, multiplier );
    
    // Calculate weighted amount for this position
    let mut weighted_amount = amount * multiplier as u64 / M_HUNDRED;
    msg!( "⚖️ Weighted amount: {} (raw amount: {} × multiplier: {})", weighted_amount, amount, multiplier);
    
    // ======>>>>>>> PROCESS ANY PENDING REWARDS BEFORE UPDATING POSITION ======>>>>>>>
    if player_ac.dogebtc_hashpower > 0 {
        msg!("💰 Processing pending rewards before position update");
                
        // rewards earned = total weighted moondoge * accumulated sol per point - reward debt
        let new_sol_rewards = helper::calculate_staking_rewards(player_ac.dogebtc_hashpower, faction_ac.dbtc_sol_reward_index, player_ac.dbtc_sol_reward_debt)?;
        player_ac.pending_sol_rewards = player_ac.pending_sol_rewards + new_sol_rewards;
        msg!( "   Updated pending SOL rewards: {}", player_ac.pending_sol_rewards);

        let new_dbtc_rewards = helper::calculate_staking_rewards(player_ac.dogebtc_hashpower, faction_ac.dbtc_dbtc_reward_index, player_ac.dbtc_dbtc_reward_debt)?;
        player_ac.pending_dbtc_rewards = player_ac.pending_dbtc_rewards + new_dbtc_rewards;
        msg!( "   Updated pending DogeBtc rewards: {}", player_ac.pending_dbtc_rewards);
    }
    
    // If position exists, validate and update
    if user_position.staked_amount > 0 {    
        msg!(  "🔄 Updating existing position - Current amount: {}", user_position.staked_amount);
        // Position should be still locked
        require!( user_position.lockup_end_timestamp > current_ts, ErrorCode::PositionStillLocked);

        // Update staked amount with actual_amount (post-tax)
        user_position.staked_amount = user_position.staked_amount + actual_amount;
        user_position.weighted_amount = user_position.weighted_amount + weighted_amount;        
        msg!("   Position staked amount: {}, weighted amount: {}", user_position.staked_amount, user_position.weighted_amount);
    } else {
        msg!("🆕 Creating new position {}", position_index);
        // Initialize new position
        helper::init_position(user_position, faction_id, position_index, actual_amount, weighted_amount, lockup_duration, current_ts, multiplier)?;                
    }
    
    // Update global user state & Global reward debt
    player_ac.dogebtc_hashpower = player_ac.dogebtc_hashpower+ weighted_amount;
    player_ac.dogebtc_staked = player_ac.dogebtc_staked + actual_amount;
    player_ac.dbtc_sol_reward_debt = faction_ac.dbtc_sol_reward_index;
    player_ac.dbtc_dbtc_reward_debt = faction_ac.dbtc_dbtc_reward_index;

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
    faction_ac.dbtc_locked = faction_ac
        .dbtc_locked
        .checked_add(actual_amount)
        .unwrap();
    faction_ac.total_dbtc_hashpower = faction_ac
        .total_dbtc_hashpower
        .checked_add(weighted_amount)
        .unwrap();

    // Calculate egg multiplier (100 = 1.0x if no egg, or egg's multiplier if provided)
    let egg_multiplier = if let Some(egg_account) = ctx.accounts.dragon_egg_metadata.as_ref() {
        // Manually deserialize DragonEggMetadata to get multiplier
        let egg_data = egg_account.try_borrow_data()?;
        let egg_metadata: moonbase::state::DragonEggMetadata = 
            AccountDeserialize::try_deserialize(&mut &egg_data[..])?;
        egg_metadata.multiplier as u128
    } else {
        100u128 // Default 1.0x multiplier
    };
    
    msg!("🥚 Dragon Egg multiplier: {} ({}x)", egg_multiplier, egg_multiplier as f64 / 100.0);
    
    // Calculate final weighted stake with egg multiplier
    // final_weighted_stake = (weighted_amount * egg_multiplier) / 100
    let final_weighted_stake = (weighted_amount as u128)
        .checked_mul(egg_multiplier)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)? as u128;
    
    msg!(
        "⚖️ Final weighted stake with egg multiplier: {} (base: {} × egg: {}%)",
        final_weighted_stake,
        weighted_amount,
        egg_multiplier
    );
    
    // Update personal hashpower in moonbase program via CPI
    msg!("🔌 Calling MoonBase to update personal hashpower");
    
    helper::update_personal_hashpower_cpi(
        &ctx.accounts.moonbase_program.to_account_info(),
        &ctx.accounts.player_data.to_account_info(),
        &ctx.accounts.faction_state.to_account_info(),
        &ctx.accounts.mooneconomy_program.to_account_info(),
        final_weighted_stake as i128,
        ctx.accounts.authority.key(),
    )?;

    msg!("✅ DogeBtc staking successful");    
    emit!(DogeBtcStaked {
        owner: ctx.accounts.authority.key(),
        amount: actual_amount, // Use actual_amount (post-tax) in the event
        lockup_duration,
        multiplier,
        weighted_amount,
        total_hashpower_contribution: 0, // Electricity system removed
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
    let player_ac = &mut ctx.accounts.player_ac;
    let faction_ac = &mut ctx.accounts.faction_ac;
    let user_position = &mut ctx.accounts.user_position;
    let current_ts = Clock::get()?.unix_timestamp;
    
    msg!("🔓 Processing unstake for position {}", position_index);
    
    // Validate the position exists and has funds
    require!(user_position.staked_amount > 0, ErrorCode::InvalidAmount);
    
    // Verify position index is in the user's active positions
    require!(
        player_ac
            .moondoge_position_indices
            .contains(&position_index),
        ErrorCode::PositionNotFound
    );
    
    msg!(
        "📊 Position details - Staked: {}, Weighted: {}, Lockup ends: {}",
        user_position.staked_amount, 
        user_position.weighted_amount,
        user_position.lockup_end_timestamp
    );
    
    // ======>>>>>>> PROCESS ANY PENDING REWARDS BEFORE UPDATING POSITION ======>>>>>>>
    if player_ac.dogebtc_hashpower > 0 {
        msg!("💰 Processing pending rewards before unstaking");
                    
        // rewards earned = total weighted moondoge * accumulated sol per point - reward debt
        let new_sol_rewards = helper::calculate_staking_rewards(player_ac.dogebtc_hashpower, faction_ac.dbtc_sol_reward_index, player_ac.dbtc_sol_reward_debt)?;
        player_ac.pending_sol_rewards = player_ac.pending_sol_rewards + new_sol_rewards;
        msg!( "   Updated pending SOL rewards: {}", player_ac.pending_sol_rewards);

        let new_dbtc_rewards = helper::calculate_staking_rewards(player_ac.dogebtc_hashpower, faction_ac.dbtc_dbtc_reward_index, player_ac.dbtc_dbtc_reward_debt)?;
        player_ac.pending_dbtc_rewards = player_ac.pending_dbtc_rewards + new_dbtc_rewards;
        msg!( "   Updated pending DogeBtc rewards: {}", player_ac.pending_dbtc_rewards);
    }
   
    // Update reward debt to current rate
    player_ac.dbtc_sol_reward_debt = faction_ac.dbtc_sol_reward_index;
    player_ac.dbtc_dbtc_reward_debt = faction_ac.dbtc_dbtc_reward_index;
    
    // Calculate return amount based on early withdrawal status
    let is_early_withdrawal = current_ts < user_position.lockup_end_timestamp;
    let original_amount = user_position.staked_amount;
    let mut return_amount = original_amount;
    
    // Handle early withdrawal if needed
    if is_early_withdrawal {
        msg!(
            "⚠️ Early unstake detected! Current time: {}, Lockup end: {}",
            current_ts,
            user_position.lockup_end_timestamp
        );
        
        // Calculate remaining lockup days
        let remaining_seconds = user_position.lockup_end_timestamp - current_ts;
        let remaining_seconds_pct = (M_HUNDRED as i64) * remaining_seconds
            / (user_position.lockup_end_timestamp - user_position.start_timestamp) as i64;
        msg!("   Lockup remaining: {}%", remaining_seconds_pct);
        
        // Apply emergency tax for early withdrawal
        let calc_penalty_pct = (hashpower_config.emergency_tax * remaining_seconds_pct as u64) / M_HUNDRED;
        msg!("   Emergency tax percentage: {}%", calc_penalty_pct);
        
        // Apply penalty to return amount
        let penalty_amount = (original_amount * calc_penalty_pct as u64) / M_HUNDRED;
        return_amount = original_amount - penalty_amount;
        msg!(
            "   Total Staked: {}, Returned: {}, Penalty: {}",
            original_amount,
            return_amount,
            penalty_amount
        );
        
        // Burn penalty tokens by sending to dead address
        if penalty_amount > 0 {
            msg!("🔥 Burning {} penalty tokens", penalty_amount);
            
            // Get PDA signer seeds for the dbtc_custodian
            let custodian_authority_seeds = &[
                DBTC_CUSTODIAN_AUTHORITY_SEED.as_ref(),
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
    
    // Calculate egg multiplier (100 = 1.0x if no egg, or egg's multiplier if provided)
    let egg_multiplier = if let Some(egg_account) = ctx.accounts.dragon_egg_metadata.as_ref() {
        // Manually deserialize DragonEggMetadata to get multiplier
        let egg_data = egg_account.try_borrow_data()?;
        let egg_metadata: moonbase::state::DragonEggMetadata = 
            AccountDeserialize::try_deserialize(&mut &egg_data[..])?;
        egg_metadata.multiplier as u128
    } else {
        100u128 // Default 1.0x multiplier
    };
    
    msg!("🥚 Dragon Egg multiplier: {} ({}x)", egg_multiplier, egg_multiplier as f64 / 100.0);
    
    // Calculate final weighted stake being removed with egg multiplier
    // final_weighted_stake = (weighted_amount * egg_multiplier) / 100
    let final_weighted_stake = (user_position.weighted_amount as u128)
        .checked_mul(egg_multiplier)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)? as u128;
    
    msg!(
        "⚖️ Final weighted stake being removed with egg multiplier: {} (base: {} × egg: {}%)",
        final_weighted_stake,
        user_position.weighted_amount,
        egg_multiplier
    );
    
    // Update personal hashpower in moonbase program via CPI (negative amount for unstaking)
    msg!("🔌 Calling MoonBase to decrease personal hashpower");
 
    // Update vault totals
    msg!("📊 Updating vault totals");
    faction_ac.dbtc_locked = faction_ac
        .dbtc_locked
        .checked_sub(original_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    faction_ac.total_dbtc_hashpower = faction_ac
        .total_dbtc_hashpower
        .checked_sub(user_position.weighted_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   New vault totals - Locked: {}, Weighted: {}",
        faction_ac.dbtc_locked,
        faction_ac.total_dbtc_hashpower
    );
    
    // Update user global stats
    msg!("📊 Updating user stats");
    player_ac.total_moondoge_staked = player_ac
        .total_moondoge_staked
        .checked_sub(original_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    player_ac.total_weighted_dogebtc = player_ac
        .total_weighted_dogebtc
        .checked_sub(user_position.weighted_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   New user totals - Staked: {}, Weighted: {}",
        player_ac.total_moondoge_staked,
        player_ac.total_weighted_dogebtc
    );
    
    // Remove position from user's active positions
    helper::remove_moondoge_position(player_ac, position_index)?;
    msg!(
        "   Updated active positions: {}",
        player_ac.active_moondoge_positions
    );
    
    // Transfer remaining tokens back to user
    if return_amount > 0 {
        msg!("💱 Transferring {} DOGE_BTC tokens to user", return_amount);
        
        // Get PDA signer seeds for the dbtc_custodian
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
    user_position.hashpower_contribution = 0;
    
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

// // --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------
// // ---- STAKE LIQUIDITY LP TOKENS :: User gets electricity and SOL rewards ------
// // --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- ---------

// /// Stake Liquidity LP tokens
// pub fn stake_lp_tokens(
//     ctx: Context<StakeLpTokens>,
//     amount: u64,
//     lockup_duration: u64,
//     position_index: u8,
// ) -> Result<()> {
//     msg!(
//         "🔒 Starting LP token staking - Amount: {}, Lockup: {} days, Position: {}",
//         amount,
//         lockup_duration,
//         position_index
//     );
//     // Validate inputs
//     require!(amount > 0, ErrorCode::InvalidAmount);
//     require!(
//         lockup_duration >= ctx.accounts.hashpower_config.min_lockup_days
//             && lockup_duration <= ctx.accounts.hashpower_config.max_lockup_days,
//         ErrorCode::InvalidLockupPeriod
//     );
    
//     // Global liquidity vault
//     let liquidity_vault = &mut ctx.accounts.liquidity_vault;
//     msg!(
//         "📊 Current vault state - Total locked: {}, Weighted locked: {}",
//         liquidity_vault.lp_tokens_locked,
//         liquidity_vault.weighted_lp_locked
//     );

//     // User Position :: Electricity account
//     let player_ac = &mut ctx.accounts.player_ac;
//     let user_position = &mut ctx.accounts.user_position;
//     let current_ts = Clock::get()?.unix_timestamp;
    
//     // Initialize owner if this is a new account
//     if player_ac.owner == Pubkey::default() {
//         player_ac.owner = ctx.accounts.authority.key();
//         player_ac.moondoge_position_indices =
//             Vec::with_capacity(MAX_ALLOWED_POSITIONS as usize);
//         player_ac.lp_position_indices = Vec::with_capacity(MAX_ALLOWED_POSITIONS as usize);
//         msg!("👤 Initializing new user electricity account");
//     }

//     // Add position index to user electricity account
//     helper::add_lp_position(player_ac, position_index)?;

//     // Calculate multiplier based on lockup duration
//     let multiplier = helper::calculate_multiplier(
//         lockup_duration,
//         ctx.accounts.hashpower_config.min_lockup_days,
//         ctx.accounts.hashpower_config.max_lockup_days,
//         ctx.accounts.hashpower_config.base_multiplier,
//         ctx.accounts.hashpower_config.max_multiplier,
//     )?;
//     msg!(
//         "🔢 Multiplier for {} days lockup: {}",
//         lockup_duration,
//         multiplier
//     );
    
//     // Calculate weighted amount for this position
//     let mut weighted_amount = amount
//         .checked_mul(multiplier as u64)
//         .unwrap()
//         .checked_div(M_HUNDRED)
//         .unwrap();
//     msg!(
//         "⚖️ Weighted amount: {} (raw amount: {} × multiplier: {})",
//         weighted_amount,
//         amount,
//         multiplier
//     );
    
//     // Process any pending rewards before updating position
//     if player_ac.total_weighted_lp > 0 {
//         msg!("💰 Processing pending rewards before position update");
//         msg!("   Previous reward debt: {}", player_ac.lp_reward_debt);
                
//         // Calculate reward diff since last update
//         let reward_diff = liquidity_vault
//             .accumulated_sol_per_point
//             .checked_sub(player_ac.lp_reward_debt)
//             .unwrap_or(0);
//         msg!(
//             "   New accumulated sol per point: {}",
//             liquidity_vault.accumulated_sol_per_point
//         );
//         msg!("   Reward diff: {}", reward_diff);

//         // rewards earned = total weighted LP * accumulated sol per point - reward debt
//         let new_rewards = (player_ac.total_weighted_lp as u128)
//             .checked_mul(reward_diff)
//             .unwrap()
//             .checked_div(PRECISION_FACTOR)
//             .unwrap_or(0) as u64;
//         msg!("   New rewards: {}", new_rewards);

//         // add rewards to pending rewards
//         player_ac.pending_lp_rewards = player_ac
//             .pending_lp_rewards
//             .checked_add(new_rewards)
//             .unwrap();
//         msg!(
//             "   Updated pending LP rewards: {}",
//             player_ac.pending_lp_rewards
//         );
//     }
    
//     // If position exists, validate and update
//     if user_position.staked_amount > 0 {
//         msg!(
//             "🔄 Updating existing position - Current amount: {}",
//             user_position.staked_amount
//         );
//         // Position should be still locked
//         require!(
//             user_position.lockup_end_timestamp > current_ts,
//             ErrorCode::PositionStillLocked
//         );
//         // Update existing position
//         let old_weighted_amount = user_position.weighted_amount;        
//         // Update staked amount
//         user_position.staked_amount = user_position
//             .staked_amount
//             .checked_add(amount)
//             .ok_or(ErrorCode::ArithmeticOverflow)?;
//         // Update weighted amount - recalculate the total weighted amount for consistency
//         user_position.weighted_amount = user_position
//             .staked_amount
//             .checked_mul(multiplier as u64)
//             .unwrap()
//             .checked_div(M_HUNDRED)
//             .unwrap();
        
//         // Calculate the actual weighted amount difference to add to vault
//         let weighted_amount_diff = user_position
//             .weighted_amount
//             .checked_sub(old_weighted_amount)
//             .unwrap();
        
//         // Update user's total LP weighted amount
//         player_ac.total_weighted_lp = player_ac
//             .total_weighted_lp
//             .checked_sub(old_weighted_amount)
//             .unwrap()
//             .checked_add(user_position.weighted_amount)
//             .unwrap();

//         msg!("   New staked amount: {}", user_position.staked_amount);
//         msg!("   New weighted amount: {}", user_position.weighted_amount);
//         msg!("   Weighted amount diff: {}", weighted_amount_diff);
//         msg!(
//             "   New total weighted: {}",
//             player_ac.total_weighted_lp
//         );
        
//         // Use the actual difference for vault updates and electricity calculations
//         weighted_amount = weighted_amount_diff;
//     } else {
//         msg!("🆕 Creating new position {}", position_index);
//         // Initialize new position
//         user_position.position_index = position_index;
//         user_position.staked_amount = amount;
//         user_position.weighted_amount = weighted_amount;
//         user_position.start_timestamp = current_ts;
//         user_position.multiplier = multiplier;
//         user_position.lockup_duration = lockup_duration;
        
//         // Calculate lockup end timestamp
//         let seconds_to_add = lockup_duration.checked_mul(DAY_IN_SECONDS).unwrap();
//         user_position.lockup_end_timestamp = current_ts.checked_add(seconds_to_add as i64).unwrap();

//         msg!(
//             "   Lockup end: {} (current: {})",
//             user_position.lockup_end_timestamp,
//             current_ts
//         );
                
//         // Update user's total LP weighted amount
//         player_ac.total_weighted_lp = player_ac
//             .total_weighted_lp
//             .checked_add(weighted_amount)
//             .unwrap();

//         msg!(
//             "   Active positions: {}",
//             player_ac.active_lp_positions
//         );
//         msg!("   Total weighted LP: {}", player_ac.total_weighted_lp);        
//     }
    
//     // Update global user state & Global reward debt
//     player_ac.total_lp_tokens_staked = player_ac
//         .total_lp_tokens_staked
//         .checked_add(amount)
//         .unwrap();
//     player_ac.lp_reward_debt = liquidity_vault.accumulated_sol_per_point;
    
//     msg!("💱 Transferring {} LP tokens from user to vault", amount);
//     msg!("   From: {}", ctx.accounts.user_lp_account.key());
//     msg!("   To: {}", ctx.accounts.liquidity_custodian.key());

//     // Transfer LP tokens from user to liquidity vault
//     let transfer_ctx = CpiContext::new(
//         ctx.accounts.token_program.to_account_info(),
//         token::Transfer {
//             from: ctx.accounts.user_lp_account.to_account_info(),
//             to: ctx.accounts.liquidity_custodian.to_account_info(),
//             authority: ctx.accounts.authority.to_account_info(),
//         },
//     );
//     token::transfer(transfer_ctx, amount)?;
    
//     // Update LP vault state
//     liquidity_vault.lp_tokens_locked = liquidity_vault
//         .lp_tokens_locked
//         .checked_add(amount)
//         .unwrap();
//     liquidity_vault.weighted_lp_locked = liquidity_vault
//         .weighted_lp_locked
//         .checked_add(weighted_amount)
//         .unwrap();

//     msg!("⚡ Calculating hashpower with egg multiplier");
    
//     // Calculate egg multiplier (100 = 1.0x if no egg, or egg's multiplier if provided)
//     let egg_multiplier = if let Some(egg_account) = ctx.accounts.dragon_egg_metadata.as_ref() {
//         // Manually deserialize DragonEggMetadata to get multiplier
//         let egg_data = egg_account.try_borrow_data()?;
//         let egg_metadata: moonbase::state::DragonEggMetadata = 
//             AccountDeserialize::try_deserialize(&mut &egg_data[..])?;
//         egg_metadata.multiplier as u128
//     } else {
//         100u128 // Default 1.0x multiplier
//     };
    
//     msg!("🥚 Dragon Egg multiplier: {} ({}x)", egg_multiplier, egg_multiplier as f64 / 100.0);
    
//     // Calculate final weighted stake with egg multiplier
//     // final_weighted_stake = (weighted_amount * egg_multiplier) / 100
//     let final_weighted_stake = (weighted_amount as u128)
//         .checked_mul(egg_multiplier)
//         .ok_or(ErrorCode::ArithmeticOverflow)?
//         .checked_div(100)
//         .ok_or(ErrorCode::ArithmeticOverflow)? as u128;
    
//     msg!(
//         "⚖️ Final weighted stake with egg multiplier: {} (base: {} × egg: {}%)",
//         final_weighted_stake,
//         weighted_amount,
//         egg_multiplier
//     );
    
//     // Update personal hashpower in moonbase program via CPI
//     msg!("🔌 Calling MoonBase to update personal hashpower");
    
//     helper::update_personal_hashpower_cpi(
//         &ctx.accounts.moonbase_program.to_account_info(),
//         &ctx.accounts.player_data.to_account_info(),
//         &ctx.accounts.faction_state.to_account_info(),
//         &ctx.accounts.mooneconomy_program.to_account_info(),
//         final_weighted_stake as i128,
//         ctx.accounts.authority.key(),
//     )?;

//     msg!("✅ LP token staking successful");    
//     emit!(LiquidityStaked {
//         owner: ctx.accounts.authority.key(),
//         amount,
//         lockup_duration,
//         multiplier,
//         weighted_amount,
//         total_hashpower_contribution: 0, // Electricity system removed
//         position_index,
//     });
    
//     Ok(())
// }

// /// Unstake Liquidity LP tokens from a position
// pub fn unstake_lp_tokens(ctx: Context<UnstakeLpTokens>, position_index: u8) -> Result<()> {
//     // Get references to all accounts
//     let player_ac = &mut ctx.accounts.player_ac;
//     let liquidity_vault = &mut ctx.accounts.liquidity_vault;
//     let user_position = &mut ctx.accounts.user_position;
//     let current_ts = Clock::get()?.unix_timestamp;
    
//     msg!("🔓 Processing unstake for position {}", position_index);
    
//     // Validate the position exists and has funds
//     require!(user_position.staked_amount > 0, ErrorCode::InvalidAmount);
    
//     // Verify position index is in the user's active positions
//     require!(
//         player_ac.lp_position_indices.contains(&position_index),
//         ErrorCode::PositionNotFound
//     );
    
//     msg!(
//         "📊 Position details - Staked: {}, Weighted: {}, Lockup ends: {}",
//         user_position.staked_amount, 
//         user_position.weighted_amount,
//         user_position.lockup_end_timestamp
//     );
    
//     // Process any pending rewards before unstaking
//     if player_ac.total_weighted_lp > 0 {
//         msg!("💰 Processing pending rewards before unstaking");
        
//         // Calculate reward diff since last update
//         let reward_diff = liquidity_vault
//             .accumulated_sol_per_point
//             .checked_sub(player_ac.lp_reward_debt)
//             .unwrap_or(0);
//         msg!("   Reward diff: {}", reward_diff);
        
//         // Calculate new rewards
//         let new_rewards = (player_ac.total_weighted_lp as u128)
//             .checked_mul(reward_diff)
//             .unwrap_or(0)
//             .checked_div(PRECISION_FACTOR)
//             .unwrap_or(0) as u64;            
//         msg!("   New rewards: {}", new_rewards);
            
//         // Add to pending rewards
//         player_ac.pending_lp_rewards = player_ac
//             .pending_lp_rewards
//             .checked_add(new_rewards)
//             .unwrap_or(player_ac.pending_lp_rewards);            
//         msg!(
//             "   Updated pending rewards: {}",
//             player_ac.pending_lp_rewards
//         );
//     }
    
//     // Update reward debt to current rate
//     player_ac.lp_reward_debt = liquidity_vault.accumulated_sol_per_point;
    
//     // Calculate return amount based on early withdrawal status
//     let is_early_withdrawal = current_ts < user_position.lockup_end_timestamp;
//     let original_amount = user_position.staked_amount;
//     let mut return_amount = original_amount;
    
//     // Handle early withdrawal if needed - fixed 10% penalty
//     if is_early_withdrawal {
//         msg!(
//             "⚠️ Early unstake detected! Current time: {}, Lockup end: {}",
//             current_ts,
//             user_position.lockup_end_timestamp
//         );
       
//         // Calculate remaining lockup days
//         let remaining_seconds = user_position.lockup_end_timestamp - current_ts;
//         let remaining_seconds_pct = (M_HUNDRED as i64) * remaining_seconds
//             / (user_position.lockup_end_timestamp - user_position.start_timestamp) as i64;
//         msg!("   Lockup remaining: {}%", remaining_seconds_pct);   
        
//         // Apply emergency tax for early withdrawal
//         let calc_penalty_pct = (liquidity_vault.emergency_tax as u64)
//             .checked_mul(remaining_seconds_pct as u64)
//             .unwrap()
//             .checked_div(M_HUNDRED)
//             .unwrap();
//         msg!("   Emergency tax percentage: {}%", calc_penalty_pct);

//         // Apply penalty to return amount
//         let penalty_amount = original_amount
//             .checked_mul(calc_penalty_pct)
//             .unwrap()
//             .checked_div(M_HUNDRED)
//             .unwrap();
//         return_amount = original_amount
//             .checked_sub(penalty_amount)
//             .ok_or(ErrorCode::ArithmeticOverflow)?;
//         msg!(
//             "   Total Staked: {}, Returned: {}, Penalty: {}",
//             original_amount,
//             return_amount,
//             penalty_amount
//         );
        
//         // If early unstake, send penalty tokens to treasury
//         if penalty_amount > 0 {
//             msg!("💸 Sending {} penalty tokens to treasury", penalty_amount);
            
//             // Get PDA signer seeds for the liquidity vault
//             let custodian_authority_seeds = &[
//                 LIQUIDITY_CUSTODIAN_AUTHORITY_SEED.as_ref(),
//                 &[ctx.bumps.liquidity_custodian_authority],
//             ];
//             let signer = &[&custodian_authority_seeds[..]];
            
//             // Burn penalty tokens from liquidity custodian
//             let burn_ctx = CpiContext::new_with_signer(
//                 ctx.accounts.token_program.to_account_info(),
//                 token::Burn {
//                     mint: ctx.accounts.lp_mint.to_account_info(), // Mint of LP token
//                     from: ctx.accounts.liquidity_custodian.to_account_info(), // Token account to burn from
//                     authority: ctx.accounts.liquidity_custodian_authority.to_account_info(), // PDA authority
//                 },
//                 signer,
//             );
//             token::burn(burn_ctx, penalty_amount)?;
            
//             // Emit early withdrawal event
//             emit!(EarlyLiquidityUnstakePenalty {
//                 owner: ctx.accounts.authority.key(),
//                 position_index,
//                 penalty_amount,
//                 penalty_tax_pct: calc_penalty_pct,
//                 return_amount,
//                 timestamp: current_ts,
//             });
//         }
//     } else {
//         msg!("✅ Normal unstake - lockup period has ended");
//     }
    
//     // Calculate egg multiplier (100 = 1.0x if no egg, or egg's multiplier if provided)
//     let egg_multiplier = if let Some(egg_account) = ctx.accounts.dragon_egg_metadata.as_ref() {
//         // Manually deserialize DragonEggMetadata to get multiplier
//         let egg_data = egg_account.try_borrow_data()?;
//         let egg_metadata: moonbase::state::DragonEggMetadata = 
//             AccountDeserialize::try_deserialize(&mut &egg_data[..])?;
//         egg_metadata.multiplier as u128
//     } else {
//         100u128 // Default 1.0x multiplier
//     };
    
//     msg!("🥚 Dragon Egg multiplier: {} ({}x)", egg_multiplier, egg_multiplier as f64 / 100.0);
    
//     // Calculate final weighted stake being removed with egg multiplier
//     // final_weighted_stake = (weighted_amount * egg_multiplier) / 100
//     let final_weighted_stake = (user_position.weighted_amount as u128)
//         .checked_mul(egg_multiplier)
//         .ok_or(ErrorCode::ArithmeticOverflow)?
//         .checked_div(100)
//         .ok_or(ErrorCode::ArithmeticOverflow)? as u128;
    
//     msg!(
//         "⚖️ Final weighted stake being removed with egg multiplier: {} (base: {} × egg: {}%)",
//         final_weighted_stake,
//         user_position.weighted_amount,
//         egg_multiplier
//     );
    
//     // Update personal hashpower in moonbase program via CPI (negative amount for unstaking)
//     msg!("🔌 Calling MoonBase to decrease personal hashpower");
    
//     helper::update_personal_hashpower_cpi(
//         &ctx.accounts.moonbase_program.to_account_info(),
//         &ctx.accounts.player_data.to_account_info(),
//         &ctx.accounts.faction_state.to_account_info(),
//         &ctx.accounts.mooneconomy_program.to_account_info(),
//         -(final_weighted_stake as i128),
//         ctx.accounts.authority.key(),
//     )?;
    
//     // Update vault totals
//     msg!("📊 Updating vault totals");
//     liquidity_vault.lp_tokens_locked = liquidity_vault
//         .lp_tokens_locked
//         .checked_sub(original_amount)
//         .ok_or(ErrorCode::ArithmeticOverflow)?;
//     liquidity_vault.weighted_lp_locked = liquidity_vault
//         .weighted_lp_locked
//         .checked_sub(user_position.weighted_amount)
//         .ok_or(ErrorCode::ArithmeticOverflow)?;
//     msg!(
//         "   New vault totals - Locked: {}, Weighted: {}",
//         liquidity_vault.lp_tokens_locked,
//         liquidity_vault.weighted_lp_locked
//     );
    
//     // Update user global stats
//     msg!("📊 Updating user stats");
//     player_ac.total_lp_tokens_staked = player_ac
//         .total_lp_tokens_staked
//         .checked_sub(original_amount)
//         .ok_or(ErrorCode::ArithmeticOverflow)?;
//     player_ac.total_weighted_lp = player_ac
//         .total_weighted_lp
//         .checked_sub(user_position.weighted_amount)
//         .ok_or(ErrorCode::ArithmeticOverflow)?;
//     msg!(
//         "   New user totals - Staked: {}, Weighted: {}",
//         player_ac.total_lp_tokens_staked,
//         player_ac.total_weighted_lp
//     );
    
//     // Remove position from user's active positions
//     helper::remove_lp_position(player_ac, position_index)?;
//     msg!(
//         "   Updated active positions: {}",
//         player_ac.active_lp_positions
//     );
    
//     // Transfer remaining tokens back to user
//     if return_amount > 0 {
//         msg!("💱 Transferring {} LP tokens to user", return_amount);
        
//         // Get PDA signer seeds for the liquidity_custodian
//         let custodian_authority_seeds = &[
//             LIQUIDITY_CUSTODIAN_AUTHORITY_SEED.as_ref(),
//             &[ctx.bumps.liquidity_custodian_authority],
//         ];
//         let signer = &[&custodian_authority_seeds[..]];
        
//         // Transfer tokens back to user
//         let transfer_ctx = CpiContext::new_with_signer(
//             ctx.accounts.token_program.to_account_info(),
//             token::Transfer {
//                 from: ctx.accounts.liquidity_custodian.to_account_info(),
//                 to: ctx.accounts.user_lp_account.to_account_info(),
//                 authority: ctx.accounts.liquidity_custodian_authority.to_account_info(),
//             },
//             signer,
//         );
        
//         token::transfer(transfer_ctx, return_amount)?;
//     }
    
//     // Reset position data
//     user_position.staked_amount = 0;
//     user_position.weighted_amount = 0;
//     user_position.hashpower_contribution = 0;
    
//     emit!(LiquidityUnstaked {
//         owner: ctx.accounts.authority.key(),
//         position_index,
//         amount: return_amount,
//         weighted_amount: user_position.weighted_amount,
//         early_withdrawal: is_early_withdrawal,
//     });
    
//     msg!("✅ Unstake completed successfully");
//     Ok(())
// }

// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- --------- ---------
// ---- CLAIM SOL REWARDS :: User earns SOL rewards from staking mDoge and LP tokens ------
// --------- --------- xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx --------- --------- ---------

/// Claim passive rewards (SOL and DogeBtc) from staking mDoge and LP tokens
pub fn claim_passive_rewards(ctx: Context<ClaimPassiveRewards>) -> Result<()> {
    msg!("🔒 Starting claim passive rewards (SOL + DogeBtc)");

    // Global moondoge vault
    let faction_ac = &mut ctx.accounts.faction_ac;
    let liquidity_vault = &mut ctx.accounts.liquidity_vault;

    // User Position :: Electricity account
    let player_ac = &mut ctx.accounts.player_ac;
      
    // Process any pending SOL rewards before claim (DOGE_BTC staking)
    if player_ac.total_weighted_dogebtc > 0 {
        msg!("💰 Processing pending DOGE_BTC rewards before claim");
                
        // Calculate reward diff since last update
        let reward_diff = faction_ac
            .accumulated_sol_per_point
            .checked_sub(player_ac.moondoge_reward_debt)
            .unwrap_or(0);
        msg!(
            "   Previous reward debt: {}",
            player_ac.moondoge_reward_debt
        );
        msg!(
            "   New accumulated sol per point: {}",
            faction_ac.accumulated_sol_per_point
        );
        msg!("   Reward diff: {}", reward_diff);

        // rewards earned = total weighted moondoge * accumulated sol per point - reward debt
        let new_rewards = (player_ac.total_weighted_dogebtc as u128)
            .checked_mul(reward_diff)
            .unwrap_or(0)
            .checked_div(PRECISION_FACTOR)
            .unwrap_or(0) as u64;
        msg!("   New rewards: {}", new_rewards);

        // add rewards to pending rewards
        player_ac.pending_dogebtc_rewards = player_ac
            .pending_dogebtc_rewards
            .checked_add(new_rewards)
            .unwrap();
        msg!(
            "   Updated pending DOGE_BTC rewards: {}",
            player_ac.pending_dogebtc_rewards
        );
    }

    // Process any pending rewards before claim (LP staking)
    if player_ac.total_weighted_lp > 0 {
        msg!("💰 Processing pending LP rewards before claim");

        let reward_diff = liquidity_vault
            .accumulated_sol_per_point
            .checked_sub(player_ac.lp_reward_debt)
            .unwrap_or(0);
        msg!("   Previous reward debt: {}", player_ac.lp_reward_debt);
        msg!(
            "   New accumulated sol per point: {}",
            liquidity_vault.accumulated_sol_per_point
        );
        msg!("   Reward diff: {}", reward_diff);

        let new_rewards = (player_ac.total_weighted_lp as u128)
            .checked_mul(reward_diff)
            .unwrap_or(0)
            .checked_div(PRECISION_FACTOR)
            .unwrap_or(0) as u64;            
        msg!("   New rewards: {}", new_rewards);

        // Add pending LP rewards to total rewards
        player_ac.pending_lp_rewards = player_ac
            .pending_lp_rewards
            .checked_add(new_rewards)
            .unwrap();
        msg!(
            "   Updated pending LP rewards: {}",
            player_ac.pending_lp_rewards
        );
    }

    // Process any pending DogeBtc token rewards before claim (DOGE_BTC staking)
    if player_ac.total_weighted_dogebtc > 0 {
        msg!("💰 Processing pending DogeBtc token rewards before claim");
        
        // Calculate reward diff since last update
        let dbtc_reward_diff = faction_ac
            .accumulated_dbtc_per_point
            .checked_sub(player_ac.moondoge_dbtc_reward_debt)
            .unwrap_or(0);
        msg!(
            "   Previous DogeBtc reward debt: {}",
            player_ac.moondoge_dbtc_reward_debt
        );
        msg!(
            "   New accumulated dbtc per point: {}",
            faction_ac.accumulated_dbtc_per_point
        );
        msg!("   DogeBtc reward diff: {}", dbtc_reward_diff);

        // DogeBtc rewards earned = total weighted moondoge * accumulated dbtc per point - reward debt
        let new_dbtc_rewards = (player_ac.total_weighted_dogebtc as u128)
            .checked_mul(dbtc_reward_diff)
            .unwrap_or(0)
            .checked_div(PRECISION_FACTOR)
            .unwrap_or(0) as u64;
        msg!("   New DogeBtc rewards: {}", new_dbtc_rewards);

        // Add DogeBtc rewards to pending rewards
        player_ac.pending_moondoge_dbtc_rewards = player_ac
            .pending_moondoge_dbtc_rewards
            .checked_add(new_dbtc_rewards)
            .unwrap();
        msg!(
            "   Updated pending DogeBtc token rewards: {}",
            player_ac.pending_moondoge_dbtc_rewards
        );
    }
    
    // Update reward debt to current rate
    player_ac.moondoge_reward_debt = faction_ac.accumulated_sol_per_point;
    player_ac.lp_reward_debt = liquidity_vault.accumulated_sol_per_point;
    player_ac.moondoge_dbtc_reward_debt = faction_ac.accumulated_dbtc_per_point;
    
    let moondoge_rewards = player_ac.pending_dogebtc_rewards;
    let lp_rewards = player_ac.pending_lp_rewards;
    let moondoge_dbtc_rewards = player_ac.pending_moondoge_dbtc_rewards;

    // Transfer DOGE_BTC staking rewards to user using system transfer
    if moondoge_rewards > 0 {
        let dbtc_vault_seeds = &[DBTC_SOL_VAULT_SEED.as_ref(), &[ctx.bumps.dbtc_sol_vault]];
        let signer_seeds = &[&dbtc_vault_seeds[..]];

        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.dbtc_sol_vault.to_account_info(),
                    to: ctx.accounts.authority.to_account_info(),
                },
                signer_seeds,
            ),
            moondoge_rewards,
        )?;

        msg!(
            "💰 Transferred {} SOL from DOGE_BTC staking vault",
            moondoge_rewards as f64 / 1e9
        );
    }

    // Transfer LP staking rewards to user using system transfer
    if lp_rewards > 0 {
        let lp_vault_seeds = &[LP_SOL_VAULT_SEED.as_ref(), &[ctx.bumps.liquidity_sol_vault]];
        let signer_seeds = &[&lp_vault_seeds[..]];

        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.liquidity_sol_vault.to_account_info(),
                    to: ctx.accounts.authority.to_account_info(),
                },
                signer_seeds,
            ),
            lp_rewards,
        )?;

        msg!(
            "💰 Transferred {} SOL from LP staking vault",
            lp_rewards as f64 / 1e9
        );
    }

    // Transfer DogeBtc token rewards to user
    if moondoge_dbtc_rewards > 0 {
        msg!("💰 Transferring {} DogeBtc tokens to user", moondoge_dbtc_rewards);
        
        // Get PDA signer seeds for the dbtc_staker_reward_vault_authority
        let dbtc_staker_vault_authority_seeds = &[DBTC_STAKER_REWARD_VAULT_AUTHORITY_SEED.as_ref(), &[ctx.bumps.dbtc_staker_reward_vault_authority]];
        let signer_seeds = &[&dbtc_staker_vault_authority_seeds[..]];
        
        // Transfer DogeBtc tokens from staker reward vault to user
        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token_interface::TransferChecked {
                from: ctx.accounts.dbtc_staker_reward_vault.to_account_info(),
                to: ctx.accounts.user_dbtc_token_account.to_account_info(),
                authority: ctx.accounts.dbtc_staker_reward_vault_authority.to_account_info(),
                mint: ctx.accounts.dbtc_mint.to_account_info(),
            },
            signer_seeds,
        );
        
        token_interface::transfer_checked(
            transfer_ctx,
            moondoge_dbtc_rewards,
            ctx.accounts.dbtc_mint.decimals,
        )?;
        
        msg!(
            "💰 Transferred {} DogeBtc tokens from staker reward vault",
            moondoge_dbtc_rewards
        );
    }
    
    emit!(PassiveRewardsClaimed {
        owner: ctx.accounts.authority.key(),
        moondoge_sol_rewards: moondoge_rewards,
        lp_sol_rewards: lp_rewards,
        moondoge_dbtc_rewards,
    });

    // Add pending rewards to total rewards
    player_ac.total_sol_claimed = player_ac
        .total_sol_claimed
        .checked_add(moondoge_rewards)
        .unwrap();
    player_ac.total_sol_claimed = player_ac
        .total_sol_claimed
        .checked_add(lp_rewards)
        .unwrap();

    // Reset pending rewards to 0
    player_ac.pending_dogebtc_rewards = 0;
    player_ac.pending_lp_rewards = 0;
    player_ac.pending_moondoge_dbtc_rewards = 0;

    msg!("✅ Claimed passive rewards (SOL + DogeBtc)");        
    Ok(())
}

/// Update user's pending SOL rewards calculation
/// This function recalculates pending rewards based on current vault state
pub fn update_pending_rewards(ctx: Context<UpdatePendingRewards>) -> Result<()> {
    let player_ac = &mut ctx.accounts.player_ac;
    let faction_ac = &ctx.accounts.faction_ac;
    let liquidity_vault = &ctx.accounts.liquidity_vault;
    
    msg!(
        "📊 Updating pending rewards for user: {}",
        ctx.accounts.authority.key()
    );
    
    // Update pending DogeBtc rewards
    if player_ac.total_weighted_dogebtc > 0 {
        let pending_moondoge = (player_ac.total_weighted_dogebtc as u128
            * faction_ac.accumulated_sol_per_point
            / PRECISION_FACTOR as u128) as u64;
        player_ac.pending_dogebtc_rewards =
            pending_moondoge.saturating_sub(player_ac.moondoge_reward_debt as u64);
        msg!(
            "💰 Updated pending DogeBtc rewards: {}",
            player_ac.pending_dogebtc_rewards
        );
    } else {
        player_ac.pending_dogebtc_rewards = 0;
    }
    
    // Update pending LP rewards
    if player_ac.total_weighted_lp > 0 {
        let pending_lp = (player_ac.total_weighted_lp as u128
            * liquidity_vault.accumulated_sol_per_point
            / PRECISION_FACTOR as u128) as u64;
        player_ac.pending_lp_rewards =
            pending_lp.saturating_sub(player_ac.lp_reward_debt as u64);
        msg!(
            "💰 Updated pending LP rewards: {}",
            player_ac.pending_lp_rewards
        );
    } else {
        player_ac.pending_lp_rewards = 0;
    }
    
    msg!("✅ Pending rewards update completed successfully");
    
    Ok(())
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
    // Global config and vaults
    #[account(
        seeds = [ME_CONFIG_SEED.as_ref()],
        bump
    )]
    pub hashpower_config: Account<'info, HashpowerConfig>,
    
    #[account(
        mut,
        seeds = [DOGE_BTC_VAULT_SEED.as_ref()],
        bump = faction_ac.bump
    )]
    pub faction_ac: Account<'info, DogeBtcVault>,
    
    // User accounts
    #[account(
        init_if_needed,
        payer = authority,
        space = UserMoonElectricity::LEN,
        seeds = [USER_ELECTRICITY_SEED, authority.key().as_ref()],
        bump
    )]
    pub player_ac: Account<'info, UserMoonElectricity>,
    
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
    pub user_position: Account<'info, StakedPosition>,
    
    /// CHECK: DOGE_BTC Mint
    pub dbtc_mint: InterfaceAccount<'info, Mint2022>,

    // Token accounts
    #[account(
        mut,
        constraint = user_dbtc_account.mint == faction_ac.dbtc_mint @ ErrorCode::InvalidTokenMint,
        constraint = user_dbtc_account.owner == authority.key() @ ErrorCode::InvalidTokenOwner,
        constraint = user_dbtc_account.amount >= amount @ ErrorCode::InsufficientFunds,
    )]
    /// User's DogeBtc token account
    pub user_dbtc_account: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        mut,
        seeds = [DBTC_CUSTODIAN_SEED.as_ref(), faction_ac.key().as_ref()],
        bump,
        constraint = dbtc_custodian.mint == faction_ac.dbtc_mint @ ErrorCode::InvalidTokenMint,
    )]
    /// Token-2022 account that holds staked DOGE_BTC
    pub dbtc_custodian: InterfaceAccount<'info, TokenAccount2022>,
    
    /// MoonBase program that handles hashpower updates
    pub moonbase_program: Program<'info, moonbase::program::Moonbase>,
    
    /// CHECK: Optional Dragon Egg metadata account (for multiplier calculation)
    pub dragon_egg_metadata: Option<UncheckedAccount<'info>>,
    
    /// CHECK: Player data account in MoonBase (for hashpower tracking)
    #[account(mut)]
    pub player_data: UncheckedAccount<'info>,
    
    /// CHECK: Faction state account in MoonBase (for hashpower tracking)
    #[account(mut)]
    pub faction_state: UncheckedAccount<'info>,
    
    /// CHECK: MoonEconomy program (for CPI verification)
    pub mooneconomy_program: AccountInfo<'info>,
    
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
    pub hashpower_config: Account<'info, HashpowerConfig>,
    
    #[account(
        mut,
        seeds = [DOGE_BTC_VAULT_SEED.as_ref()],
        bump = faction_ac.bump
    )]
    pub faction_ac: Account<'info, DogeBtcVault>,
    
    // User accounts
    #[account(
        mut,
        seeds = [USER_ELECTRICITY_SEED, authority.key().as_ref()],
        bump
    )]
    pub player_ac: Account<'info, UserMoonElectricity>,
    
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
        constraint = user_dbtc_account.mint == faction_ac.dbtc_mint @ ErrorCode::InvalidTokenMint,
    )]
    /// User's DogeBtc token account to receive the unstaked tokens
    pub user_dbtc_account: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        mut,
        seeds = [DBTC_CUSTODIAN_SEED.as_ref(), faction_ac.key().as_ref()],
        bump,
        constraint = dbtc_custodian.mint == faction_ac.dbtc_mint @ ErrorCode::InvalidTokenMint,
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
    
    /// MoonBase program that handles hashpower updates
    pub moonbase_program: Program<'info, moonbase::program::Moonbase>,
    
    /// CHECK: Optional Dragon Egg metadata account (for multiplier calculation)
    pub dragon_egg_metadata: Option<UncheckedAccount<'info>>,
    
    /// CHECK: Player data account in MoonBase (for hashpower tracking)
    #[account(mut)]
    pub player_data: UncheckedAccount<'info>,
    
    /// CHECK: Faction state account in MoonBase (for hashpower tracking)
    #[account(mut)]
    pub faction_state: UncheckedAccount<'info>,
    
    /// CHECK: MoonEconomy program (for CPI verification)
    pub mooneconomy_program: AccountInfo<'info>,
    
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
    pub hashpower_config: Account<'info, HashpowerConfig>,
    
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
    pub player_ac: Account<'info, UserMoonElectricity>,
    
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
    
    /// MoonBase program that handles hashpower updates
    pub moonbase_program: Program<'info, moonbase::program::Moonbase>,
    
    /// CHECK: Optional Dragon Egg metadata account (for multiplier calculation)
    pub dragon_egg_metadata: Option<UncheckedAccount<'info>>,
    
    /// CHECK: Player data account in MoonBase (for hashpower tracking)
    #[account(mut)]
    pub player_data: UncheckedAccount<'info>,
    
    /// CHECK: Faction state account in MoonBase (for hashpower tracking)
    #[account(mut)]
    pub faction_state: UncheckedAccount<'info>,
    
    /// CHECK: MoonEconomy program (for CPI verification)
    pub mooneconomy_program: AccountInfo<'info>,
    
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
    pub hashpower_config: Account<'info, HashpowerConfig>,
    
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
    pub player_ac: Account<'info, UserMoonElectricity>,
    
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
    
    /// MoonBase program that handles hashpower updates
    pub moonbase_program: Program<'info, moonbase::program::Moonbase>,
    
    /// CHECK: Optional Dragon Egg metadata account (for multiplier calculation)
    pub dragon_egg_metadata: Option<UncheckedAccount<'info>>,
    
    /// CHECK: Player data account in MoonBase (for hashpower tracking)
    #[account(mut)]
    pub player_data: UncheckedAccount<'info>,
    
    /// CHECK: Faction state account in MoonBase (for hashpower tracking)
    #[account(mut)]
    pub faction_state: UncheckedAccount<'info>,
    
    /// CHECK: MoonEconomy program (for CPI verification)
    pub mooneconomy_program: AccountInfo<'info>,
    
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
pub struct ClaimPassiveRewards<'info> {
    // Global config and vaults
    #[account(
        seeds = [ME_CONFIG_SEED.as_ref()],
        bump
    )]
    pub hashpower_config: Account<'info, HashpowerConfig>,

    #[account(
        mut,
        seeds = [DOGE_BTC_VAULT_SEED.as_ref()],
        bump 
    )]
    pub faction_ac: Account<'info, DogeBtcVault>,
    
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
    pub player_ac: Account<'info, UserMoonElectricity>,
    
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
    
    #[account(
        mut,
        seeds = [DBTC_STAKER_REWARD_VAULT_SEED.as_ref()],
        bump
    )]
    /// CHECK: This is the PDA that custodies DogeBtc tokens for stakers
    pub dbtc_staker_reward_vault: UncheckedAccount<'info>,
    
    #[account(
        seeds = [DBTC_STAKER_REWARD_VAULT_AUTHORITY_SEED.as_ref()],
        bump
    )]
    /// CHECK: This is the PDA authority for the staker reward vault
    pub dbtc_staker_reward_vault_authority: UncheckedAccount<'info>,
    
    /// CHECK: User's DogeBtc token account to receive rewards
    #[account(
        mut,
        constraint = user_dbtc_token_account.owner == authority.key() @ ErrorCode::InvalidTokenOwner,
        constraint = user_dbtc_token_account.mint == faction_ac.dbtc_mint @ ErrorCode::InvalidTokenMint,
    )]
    pub user_dbtc_token_account: InterfaceAccount<'info, TokenAccount2022>,
    
    /// CHECK: DogeBtc mint
    pub dbtc_mint: InterfaceAccount<'info, Mint2022>,

    /// User who is claiming rewards
    #[account(mut)]
    pub authority: Signer<'info>,

    /// System program for SOL transfers
    pub system_program: Program<'info, System>,
    
    /// Token-2022 program for DogeBtc token transfers
    pub token_program: Program<'info, Token2022>,
}

/// Account struct for updating pending rewards
#[derive(Accounts)]
pub struct UpdatePendingRewards<'info> {
    // Global config and vaults
    #[account(
        seeds = [ME_CONFIG_SEED.as_ref()],
        bump
    )]
    pub hashpower_config: Account<'info, HashpowerConfig>,

    #[account(
        seeds = [DOGE_BTC_VAULT_SEED.as_ref()],
        bump 
    )]
    pub faction_ac: Account<'info, DogeBtcVault>,
    
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
    pub player_ac: Account<'info, UserMoonElectricity>,

    /// User requesting the rewards update
    #[account(mut)]
    pub authority: Signer<'info>,
}

// Old CPI wrapper functions removed - no longer needed for Faction Surge system
// (claim_dbtc_tokens_wrapper, claim_referral_rewards_wrapper, claim_attraction_xp_wrapper)
