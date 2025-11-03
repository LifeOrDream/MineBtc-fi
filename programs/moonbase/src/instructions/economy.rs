use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use anchor_lang::system_program;
use mpl_core::{
    instructions::CreateCollectionV1CpiBuilder,
    ID as MPL_CORE_PROGRAM_ID,
};

use crate::errors::ErrorCode;

use anchor_spl::token_interface::{
    self as token_if,           // gives you CPI helpers such as `token_if::transfer`
    Mint as Mint2022,
    TokenAccount as TokenAccount2022,
};
use anchor_spl::token_2022::Token2022;          // ← the PROGRAM-ID wrapper (implements Id)

// Import Raydium CP-Swap for CPI calls
use raydium_cp_swap;




/// Update DBTC distribution rate based on price oracle
/// This function can be called by anyone every 30 minutes
/// Distribution rate updated every 4 hours with 3% deadband
pub fn update_dbtc_dist_per_slot_internal(ctx: Context<UpdateMdogeDistPerSlot>, lp_token_amount: u64) -> Result<()> {
    let doge_btc_mining = &mut ctx.accounts.doge_btc_mining;
    let current_time = Clock::get()?.unix_timestamp;
    
    // Check if admin override is being used (when lp_token_amount > 0)
    if lp_token_amount > 0 {
        // Verify that the authority is provided and matches the global config
        require!(ctx.accounts.authority.is_some(), ErrorCode::Unauthorized);
        let authority = ctx.accounts.authority.as_ref().unwrap();
        require!(
            ctx.accounts.global_config.ext_authority == authority.key(),
            ErrorCode::Unauthorized
        );
        msg!("🔧 Admin override: Using LP token amount {}", lp_token_amount);
    } else {
        msg!("🔄 Using automatic LP calculation");
    }
    
    // Check if at least 30 minutes has passed since last update
    let thirty_mins = THIRTY_MINS as i64;
    if current_time < doge_btc_mining.last_rate_update + thirty_mins {
        msg!("⏰ Update too early - must wait at least 30 minutes between updates");
        msg!("   Current time: {}, Next update allowed: {}, remaining seconds: {}", current_time, doge_btc_mining.last_rate_update + thirty_mins, (doge_btc_mining.last_rate_update + thirty_mins - current_time));
        return Ok(());
    }
    
    msg!("🔄 Starting DOGE_BTC distribution rate update");
    msg!("   Current time: {}", current_time);
    msg!("   Last update: {}", doge_btc_mining.last_rate_update);
    msg!("   Current dist rate: {}", doge_btc_mining.current_dist_rate);
    
    // Calculate DBTC for liquidity based on current distribution rate and slots
    let dbtc_for_liquidity = doge_btc_mining.current_dist_rate.checked_mul(doge_btc_mining.slots_for_swap).ok_or(ErrorCode::ArithmeticOverflow)?;
        
    msg!("   Price snapshot {}/8: Swapping {} DOGE_BTC for SOL", 
         doge_btc_mining.price_history.len() + 1, dbtc_for_liquidity);
    
    // Perform swap via Raydium CPI to get current exchange rate
    let sol_received = perform_dbtc_to_sol_swap(
        &ctx.accounts.raydium_program,
        &ctx.accounts.pool_state,
        &ctx.accounts.amm_config,
        &ctx.accounts.authority_pda,
        &ctx.accounts.raydium_authority,
        &ctx.accounts.dbtc_vault,
        &ctx.accounts.sol_vault,
        &ctx.accounts.dbtc_token_account,
        &ctx.accounts.sol_token_account,
        &ctx.accounts.dbtc_mint,
        &ctx.accounts.sol_mint,
        &ctx.accounts.observation_state,
        &ctx.accounts.token_program_2022,
        &ctx.accounts.token_program,
        dbtc_for_liquidity,
        doge_btc_mining.vault_auth_bump,
    )?;
    
    // Calculate current price (SOL per DOGE_BTC) with proper decimal handling
    // sol_received is in WSOL base units (9 decimals), dbtc_for_liquidity is in DOGE_BTC base units (6 decimals)
    // 
    // Formula: Price = (sol_received / 10^9) / (dbtc_for_liquidity / 10^6)
    // Simplified: Price = (sol_received * 10^6) / (dbtc_for_liquidity * 10^9)
    // To store with 9-decimal precision: multiply by 10^9
    // Final: Price = (sol_received * 10^6 * 10^9) / (dbtc_for_liquidity * 10^9) = (sol_received * 10^6) / dbtc_for_liquidity
    let current_price = if dbtc_for_liquidity > 0 {
        // Prevent overflow by checking limits
        if sol_received > crate::state::MAX_SAFE_U64 || dbtc_for_liquidity > crate::state::MAX_SAFE_U64 {
            msg!("⚠️ Price calculation values too large, using fallback");
            0
        } else {
            // Calculate: (sol_received * 10^9) / dbtc_for_liquidity
            // This gives us SOL per DOGE_BTC stored with 9-decimal precision
            (sol_received as u128)
                .checked_mul(1_000_000_000) // Scale by 10^9 for full precision
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(dbtc_for_liquidity as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .min(u64::MAX as u128) as u64
        }
    } else {
        0
    };
    
    // Calculate human-readable price for logging
    // Convert back to actual SOL per DOGE_BTC
    let actual_price = current_price as f64 / 1_000_000_000.0;
    msg!("   Swap details: {} DOGE_BTC base units → {} WSOL base units", dbtc_for_liquidity, sol_received);
    msg!("   Human readable: {} DOGE_BTC → {:.9} SOL", dbtc_for_liquidity / 1_000_000, sol_received as f64 / 1_000_000_000.0);
    msg!("   Current price: {} (9-decimal precision), Actual: {:.9} SOL per DOGE_BTC", 
         current_price, actual_price);
    
    // Add current price to history
    let price_entry = PriceEntry {
        timestamp: current_time,
        price: current_price,
    };
    
    // Add price entry to history
    doge_btc_mining.price_history.push(price_entry);
    
    // Accumulate SOL for POL
    doge_btc_mining.sol_for_pol = doge_btc_mining.sol_for_pol.checked_add(sol_received).unwrap();
    
    // Calculate ongoing weighted average (even before 4 hours)
    let mut weighted_sum: u128 = 0;
    let mut total_weights: u128 = 0;
    
    for (i, entry) in doge_btc_mining.price_history.iter().enumerate() {
        let weight = (i + 1) as u128; // Weight from 1 to 8
        
        let price_contribution = (entry.price as u128)
            .checked_mul(weight)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        weighted_sum = weighted_sum
            .checked_add(price_contribution)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        total_weights = total_weights
            .checked_add(weight)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    
    let current_weighted_avg = if total_weights > 0 {
        (weighted_sum / total_weights).min(u64::MAX as u128) as u64
    } else {
        current_price
    };
    
    // Update recent price with current weighted average
    doge_btc_mining.recent_price = current_weighted_avg;
    
    msg!("   💰 Accumulated {} WSOL for POL, total reserve: {}", sol_received, doge_btc_mining.sol_for_pol);
    msg!("   📊 Ongoing weighted average: {} (from {} snapshots)", current_weighted_avg, doge_btc_mining.price_history.len());
    msg!("   🎯 Track price (last rate change): {}", doge_btc_mining.track_price);
    
    // Update timestamp for next snapshot
    doge_btc_mining.last_rate_update = current_time;
        
    // ----------------------------------------------------
    // Check if 4 hours have passed AND we have 8 price entries
    // Only then check if distribution rate should change
    // ----------------------------------------------------
    let four_hours = FOUR_HOURS as i64;
    let time_since_last = doge_btc_mining.price_history.first()
        .map(|e| current_time - e.timestamp)
        .unwrap_or(0);
    
    if doge_btc_mining.price_history.len() < 8 || time_since_last < four_hours {
        msg!("   ⏰ Not ready for rate update: {} snapshots, {} seconds elapsed (need 8 snapshots over 4 hours)", 
             doge_btc_mining.price_history.len(), time_since_last);
        return Ok(());
    }
    
    // ----------------------------------------------------
    // 4 hours completed - Check if rate should change
    // ----------------------------------------------------
    msg!("   ✅ 4-hour cycle complete with {} snapshots", doge_btc_mining.price_history.len());
    
    let new_avg_price = current_weighted_avg;
    
    // Initialize track_price if first time
    if doge_btc_mining.track_price == 0 {
        doge_btc_mining.track_price = new_avg_price;
        msg!("   🎯 Initialized track_price: {}", doge_btc_mining.track_price);
    }
    
    // Calculate price change percentage from BOTH recent and track prices
    // Use the LARGER change to determine if we should update
    let change_from_track = calculate_price_change_pct(doge_btc_mining.track_price, new_avg_price);
    
    // For recent_price, use the oldest entry in history (4 hours ago)
    let recent_comparison_price = doge_btc_mining.price_history.first()
        .map(|e| e.price)
        .unwrap_or(new_avg_price);
    let change_from_recent = calculate_price_change_pct(recent_comparison_price, new_avg_price);
    
    msg!("   📊 Price changes: from track_price ({}): {}%, from 4h ago ({}): {}%", 
         doge_btc_mining.track_price, change_from_track.0,
         recent_comparison_price, change_from_recent.0);
    
    // Pick the larger change (by absolute value)
    let (price_change_pct, direction) = if change_from_track.0.abs() > change_from_recent.0.abs() {
        msg!("   🎯 Using change from track_price (larger movement)");
        change_from_track
    } else {
        msg!("   🎯 Using change from 4h ago price (larger movement)");
        change_from_recent
    };
    
    msg!("   📈 Selected price change: {}% (direction: {})", price_change_pct, direction);
    
    // Check if change exceeds 3% threshold
    let old_rate = doge_btc_mining.current_dist_rate;
    let mut rate_changed = false;
    
    if price_change_pct.abs() < PRICE_CHANGE_THRESHOLD as i64 {
        msg!("   ➡️ Price change {}% within ±3% deadband, keeping same distribution rate", price_change_pct);
        // Don't update track_price, keep monitoring
    } else if direction > 0 {
        // Price increased by >3% - increase distribution by 1%
        doge_btc_mining.current_dist_rate = doge_btc_mining.current_dist_rate
            .checked_mul(101)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(100)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        msg!("   📈 Price increased {}%! Increasing distribution rate by 1%", price_change_pct);
        rate_changed = true;
    } else {
        // Price decreased by >3% - decrease distribution by 3%
        doge_btc_mining.current_dist_rate = doge_btc_mining.current_dist_rate
            .checked_mul(97)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(100)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        msg!("   📉 Price decreased {}%! Decreasing distribution rate by 3%", price_change_pct);
        rate_changed = true;
    }
    
    // Update track_price only if rate actually changed
    if rate_changed {
        doge_btc_mining.track_price = new_avg_price;
        msg!("   🎯 Updated track_price to: {}", doge_btc_mining.track_price);
    }
    
    // Calculate amounts for LP addition using UPDATED distribution rate
    // Use accumulated SOL (already includes current swap)
    let total_sol_for_lp = doge_btc_mining.sol_for_pol;
    
    msg!("   🏦 Adding liquidity: {} WSOL (accumulated over 4 hours)", total_sol_for_lp);
    
    // Note: WSOL is already in our sol_token_account from swaps, no need to withdraw from treasury
    
    // Perform actual LP addition and burn
    perform_lp_addition_and_burn(
        &ctx.accounts.raydium_program,
        &ctx.accounts.pool_state,
        &ctx.accounts.authority_pda,
        &ctx.accounts.raydium_authority,
        &ctx.accounts.dbtc_vault,
        &ctx.accounts.sol_vault,
        &ctx.accounts.dbtc_token_account,
        &ctx.accounts.sol_token_account,
        &ctx.accounts.lp_token_account,
        &ctx.accounts.lp_mint,
        &ctx.accounts.dbtc_mint,
        &ctx.accounts.sol_mint,
        &ctx.accounts.token_program_2022,
        &ctx.accounts.token_program,
        total_sol_for_lp,
        doge_btc_mining.vault_auth_bump,
        doge_btc_mining,
        lp_token_amount, // Pass the LP token amount
    )?;
    
    // Check actual WSOL balance after LP addition to see how much was consumed
    let wsol_balance_after_lp = {
        let sol_account_data = ctx.accounts.sol_token_account.try_borrow_data()?;
        let sol_token_data = anchor_spl::token::TokenAccount::try_deserialize(&mut &sol_account_data[..])?;
        sol_token_data.amount
    };
    
    // Calculate how much SOL was actually consumed for LP
    let sol_consumed_for_lp = total_sol_for_lp.saturating_sub(wsol_balance_after_lp);
    
    // Update POL tracking - subtract only the amount actually used
    doge_btc_mining.sol_for_pol = wsol_balance_after_lp; // Keep any leftover WSOL for next cycle
    
    msg!("   💰 SOL consumption: {} total available, {} consumed for LP, {} remaining", 
         total_sol_for_lp, sol_consumed_for_lp, doge_btc_mining.sol_for_pol);
    
    // Clear price history to restart the 4-hour cycle
    doge_btc_mining.price_history.clear();
    
    // Update state
    doge_btc_mining.recent_price = new_avg_price; // Store as recent for next cycle
    doge_btc_mining.last_rate_update = current_time;
    
    msg!("   🔄 Price history cleared - restarting 4-hour accumulation cycle");
    msg!("   🎯 Distribution rate: {} -> {} ({})", 
         old_rate, doge_btc_mining.current_dist_rate,
         if rate_changed { "CHANGED" } else { "unchanged" });
    
    emit!(DistributionRateUpdated {
        old_rate,
        new_rate: doge_btc_mining.current_dist_rate,
        price_change_pct: price_change_pct as i32,
        current_price,
        avg_price_4h: new_avg_price,
        track_price: doge_btc_mining.track_price,
        recent_price: doge_btc_mining.recent_price,
        rate_changed,
        sol_received,
        timestamp: current_time,
    });
    
    Ok(())
}



/// Helper function to perform DOGE_BTC to SOL swap via Raydium CPI
fn perform_dbtc_to_sol_swap<'info>(
    raydium_program: &AccountInfo<'info>,
    pool_state: &AccountInfo<'info>,
    amm_config: &AccountInfo<'info>,
    authority_pda: &AccountInfo<'info>,
    raydium_authority: &AccountInfo<'info>,
    dbtc_vault: &AccountInfo<'info>,
    sol_vault: &AccountInfo<'info>,
    dbtc_token_account: &AccountInfo<'info>,
    sol_token_account: &AccountInfo<'info>,
    dbtc_mint: &AccountInfo<'info>,
    sol_mint: &AccountInfo<'info>,
    observation_state: &AccountInfo<'info>,
    token_program_2022: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    amount_in: u64,
    vault_auth_bump: u8,
) -> Result<u64> {
    use raydium_cp_swap::cpi;
    
    msg!("🔄 Performing real Raydium swap: {} DOGE_BTC for WSOL", amount_in);
    
    // Get WSOL token balance before swap by deserializing account data
    let sol_balance_before = {
        let sol_account_data = sol_token_account.try_borrow_data()?;
        let sol_token_data = anchor_spl::token::TokenAccount::try_deserialize(&mut &sol_account_data[..])?;
        sol_token_data.amount
    }; // Borrow is dropped here
    
    // Create signer seeds for vault authority
    let authority_seeds = &[
        DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref(),
        &[vault_auth_bump],
    ];
    let signer_seeds = &[&authority_seeds[..]];
    
    // Create CPI context for Raydium swap
    let cpi_accounts = cpi::accounts::Swap {
        payer: authority_pda.to_account_info(),         // Our PDA as the payer/signer
        authority: raydium_authority.to_account_info(), // Raydium's pool authority PDA
        amm_config: amm_config.to_account_info(),
        pool_state: pool_state.to_account_info(),
        input_token_account: dbtc_token_account.to_account_info(),  // Our token account (authority = our PDA)
        output_token_account: sol_token_account.to_account_info(),   // Our token account (authority = our PDA)
        input_vault: dbtc_vault.to_account_info(),     // Raydium's DOGE_BTC vault
        output_vault: sol_vault.to_account_info(),      // Raydium's SOL vault  
        input_token_program: token_program_2022.to_account_info(),   // Token-2022 for DOGE_BTC
        output_token_program: token_program.to_account_info(),       // Standard token for SOL
        input_token_mint: dbtc_mint.to_account_info(),
        output_token_mint: sol_mint.to_account_info(),
        observation_state: observation_state.to_account_info(),
    };
    
    let cpi_ctx = CpiContext::new_with_signer(
        raydium_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );
    
    // Accept any amount out since we're just getting current market price
    let min_amount_out = 0;
    
    // Perform the actual swap
    cpi::swap_base_input(cpi_ctx, amount_in, min_amount_out)?;
    
    // Calculate actual WSOL received by checking token account balance again
    let sol_received = {
        let sol_account_data_after = sol_token_account.try_borrow_data()?;
        let sol_token_data_after = anchor_spl::token::TokenAccount::try_deserialize(&mut &sol_account_data_after[..])?;
        let sol_balance_after = sol_token_data_after.amount;
        sol_balance_after.saturating_sub(sol_balance_before)
    }; // Borrow is dropped here
    
    msg!("✅ Swap completed: received {} WSOL tokens", sol_received);
    
    Ok(sol_received)
}



/// Helper function to add liquidity to Raydium pool and burn LP tokens
fn perform_lp_addition_and_burn<'info>(
    raydium_program: &AccountInfo<'info>,
    pool_state: &AccountInfo<'info>,
    authority_pda: &AccountInfo<'info>,
    raydium_authority: &AccountInfo<'info>,
    dbtc_vault: &AccountInfo<'info>,
    sol_vault: &AccountInfo<'info>,
    dbtc_token_account: &AccountInfo<'info>,
    sol_token_account: &AccountInfo<'info>,
    lp_token_account: &AccountInfo<'info>,
    lp_mint: &AccountInfo<'info>,
    dbtc_mint: &AccountInfo<'info>,
    sol_mint: &AccountInfo<'info>,
    token_program_2022: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    sol_amount: u64,
    vault_auth_bump: u8,
    doge_btc_mining: &mut Account<DogeBtcMining>,
    admin_lp_override: u64,
) -> Result<()> {

    
    msg!("🏦 Starting LP addition: {} SOL", sol_amount);
    
    // Create signer seeds for vault authority
    let authority_seeds = &[
        DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref(),
        &[vault_auth_bump],
    ];
    let signer_seeds = &[&authority_seeds[..]];
    
    // Step 1: Get LP token balance before deposit to calculate actual minted amount
    let lp_balance_before = {
        use anchor_spl::token_interface::TokenAccount as TokenAccountInterface;
        let lp_account_data = lp_token_account.try_borrow_data()?;
        let lp_account = TokenAccountInterface::try_deserialize(&mut &lp_account_data[..])?;
        lp_account.amount
    };
    
    msg!("💰 LP token balance before deposit: {}", lp_balance_before);
    
    // Step 2: Use actual Raydium CPI for deposit
    msg!("🏦 Creating CPI context for Raydium deposit");
    
    let cpi_accounts = raydium_cp_swap::cpi::accounts::Deposit {
        owner: authority_pda.to_account_info(),        // Our vault authority as the owner/signer
        authority: raydium_authority.to_account_info(), // Raydium's pool authority PDA
        pool_state: pool_state.to_account_info(),
        owner_lp_token: lp_token_account.to_account_info(), // LP token account (authority = our PDA)
        token_0_account: sol_token_account.to_account_info(),   // Our SOL account (authority = our PDA) - token0 is WSOL
        token_1_account: dbtc_token_account.to_account_info(), // Our DOGE_BTC account (authority = our PDA) - token1 is DOGE_BTC
        token_0_vault: sol_vault.to_account_info(),      // Raydium's SOL vault - token0 vault
        token_1_vault: dbtc_vault.to_account_info(),    // Raydium's DOGE_BTC vault - token1 vault
        token_program: token_program.to_account_info(),  // Standard token program
        token_program_2022: token_program_2022.to_account_info(), // Token-2022 program
        vault_0_mint: sol_mint.to_account_info(),        // SOL mint - token0 mint
        vault_1_mint: dbtc_mint.to_account_info(),      // DOGE_BTC mint - token1 mint
        lp_mint: lp_mint.to_account_info(),              // Raydium's LP mint
    };
    
    let cpi_ctx = CpiContext::new_with_signer(
        raydium_program.to_account_info(),
        cpi_accounts,
        signer_seeds, // Use vault authority signer seeds for all operations
    );
    
    // Read token vault balances directly from the token accounts
    let sol_vault_balance = {
        let account_data = sol_vault.try_borrow_data()?;
        let token_account = anchor_spl::token::TokenAccount::try_deserialize(&mut &account_data[..])?;
        token_account.amount
    };
    
    let dbtc_vault_balance = {
        let account_data = dbtc_vault.try_borrow_data()?;
        let token_account = anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &account_data[..])?;
        token_account.amount
    };
    
    // Read LP supply from pool state (this is what Raydium uses internally)
    let lp_supply = {
        let pool_data = pool_state.try_borrow_data()?;
        // Skip discriminator (8 bytes) and read the lp_supply field directly
        // Based on PoolState struct: lp_supply is at offset after all the Pubkeys and small fields
        // From pool.rs: 10 Pubkeys (320 bytes) + 5 u8s (5 bytes) = 325 bytes from start + discriminator (8) = 333
        let lp_supply_offset = 8 + 10 * 32 + 5; // discriminator + 10 pubkeys + 5 u8 fields
        if pool_data.len() >= lp_supply_offset + 8 {
            u64::from_le_bytes([
                pool_data[lp_supply_offset],
                pool_data[lp_supply_offset + 1],
                pool_data[lp_supply_offset + 2],
                pool_data[lp_supply_offset + 3],
                pool_data[lp_supply_offset + 4],
                pool_data[lp_supply_offset + 5],
                pool_data[lp_supply_offset + 6],
                pool_data[lp_supply_offset + 7],
            ])
        } else {
            0 // Fallback if we can't read the data
        }
    };
    
    msg!("📊 Pool balances - SOL vault: {}, DOGE_BTC vault: {}, LP supply: {}", 
         sol_vault_balance, dbtc_vault_balance, lp_supply);
    
    // Reserve buffer upfront to account for transfer fees and rounding
    // This ensures our calculations are based on what we can actually use
    let sol_buffer = sol_amount / 50; // 2% buffer for transfer fees and rounding
    let available_sol = sol_amount.saturating_sub(sol_buffer);
    
    msg!("🛡️ Reserved {} SOL as buffer, available for LP: {} SOL", sol_buffer, available_sol);
    
        // Calculate LP tokens and adjusted amounts to maximize token usage
    let (estimated_lp_amount, adjusted_sol_amount, adjusted_dbtc_amount) = if admin_lp_override > 0 {
        // Admin override: Calculate required token amounts for the specified LP amount
        let required_sol = if lp_supply > 0 && sol_vault_balance > 0 {
            (admin_lp_override as u128 * sol_vault_balance as u128 / lp_supply as u128) as u64
        } else {
            available_sol // Fallback to available amount (after buffer)
        };
        
        let required_mdoge = if lp_supply > 0 && dbtc_vault_balance > 0 {
            (admin_lp_override as u128 * dbtc_vault_balance as u128 / lp_supply as u128) as u64
        } else {
            0 // No DOGE_BTC needed if pool is empty
        };
        
        // Use available SOL (already has buffer applied)
        let final_sol = required_sol.min(available_sol);
        let final_mdoge = required_mdoge + 100;
        
        msg!("🔧 Admin override LP calculation: {} LP tokens (needs {} SOL, {} DOGE_BTC)", 
             admin_lp_override, final_sol, final_mdoge);
             
        (admin_lp_override, final_sol, final_mdoge)
    } else {
        // Normal automatic calculation using available SOL (after buffer)
        let lp_from_sol = (available_sol as u128 * lp_supply as u128 / sol_vault_balance as u128) as u64;
        let required_mdoge = (lp_from_sol as u128 * dbtc_vault_balance as u128 / lp_supply as u128) as u64;
        msg!("💰 SOL-limited LP calculation: {} LP tokens (needs {} SOL, {} DOGE_BTC)",  
             lp_from_sol, available_sol, required_mdoge);
        (lp_from_sol, available_sol, required_mdoge + 100) // Add small buffer for ceiling rounding
    };
    
    msg!("🎯 Final LP token amount: {} for deposits of {} SOL, {} DOGE_BTC", 
         estimated_lp_amount, adjusted_sol_amount, adjusted_dbtc_amount);
    
    // Add small additional buffer to DOGE_BTC for transfer fees (1% burn tax)
    // SOL already has buffer applied upfront, but DOGE_BTC needs extra for burn tax
    let max_dbtc_with_buffer = adjusted_dbtc_amount.saturating_add(adjusted_dbtc_amount / 50); // +2% buffer for burn tax
    
    msg!("🛡️ Maximum amounts: {} SOL (buffered upfront), {} DOGE_BTC (with burn tax buffer)", 
         adjusted_sol_amount, max_dbtc_with_buffer);
    
    // Perform the actual deposit with calculated LP amount and proper maximums
    // Parameters: (lp_token_amount, maximum_token_0_amount, maximum_token_1_amount)
    // token0 = WSOL, token1 = DOGE_BTC
    // SOL amount is already buffered, DOGE_BTC has burn tax buffer
    raydium_cp_swap::cpi::deposit(cpi_ctx, estimated_lp_amount, adjusted_sol_amount, max_dbtc_with_buffer)?;
    
    // Calculate actual LP tokens minted by checking balance difference
    let lp_balance_after = {
        use anchor_spl::token_interface::TokenAccount as TokenAccountInterface;
        let lp_account_data = lp_token_account.try_borrow_data()?;
        let lp_account = TokenAccountInterface::try_deserialize(&mut &lp_account_data[..])?;
        lp_account.amount
    };
    
    let lp_tokens_minted = lp_balance_after.saturating_sub(lp_balance_before);
    
    msg!("💰 LP token balance after deposit: {}", lp_balance_after);
    msg!("✅ LP tokens minted: {}", lp_tokens_minted);
    
    // Step 3: Burn the LP tokens immediately using SPL token burn
    if lp_tokens_minted > 0 {
        msg!("🔥 Burning {} LP tokens", lp_tokens_minted);
        
        use anchor_spl::token;
        
        // Use vault authority to burn LP tokens (same authority that owns the LP token account)
        let burn_ctx = CpiContext::new_with_signer(
            token_program.to_account_info(),
            token::Burn {
                mint: lp_mint.to_account_info(),
                from: lp_token_account.to_account_info(),
                authority: authority_pda.to_account_info(),
            },
            signer_seeds, // Use vault authority signer seeds (same as deposit)
        );
        
        token::burn(burn_ctx, lp_tokens_minted)?;
        
        // Get actual amounts consumed by checking token account balances before/after
        let sol_consumed = {
            let sol_account_data = sol_token_account.try_borrow_data()?;
            let sol_token_data = anchor_spl::token::TokenAccount::try_deserialize(&mut &sol_account_data[..])?;
            // Calculate how much SOL was actually consumed (we started with sol_amount, now have this much left)
            sol_amount.saturating_sub(sol_token_data.amount)
        };
            
        // Update POL stats with actual consumed amounts
        doge_btc_mining.pol_stats.update_after_lp_operation(
            lp_tokens_minted,
            sol_consumed,
            adjusted_dbtc_amount
        );
        
        msg!("📊 POL Stats Updated:");
        msg!("   Total LP Burnt: {}", doge_btc_mining.pol_stats.total_lp_burnt);
        msg!("   Total SOL Added: {}", doge_btc_mining.pol_stats.total_sol_added);
        msg!("   Total DOGE_BTC Added: {}", doge_btc_mining.pol_stats.total_dbtc_added);
        msg!("   LP Operations: {}", doge_btc_mining.pol_stats.lp_operations_count);
        
        // Emit LP burn tracking event
        emit!(LpTokensBurned {
            lp_tokens_burned: lp_tokens_minted,
            total_lp_burnt: doge_btc_mining.pol_stats.total_lp_burnt,
            dbtc_amount_added: adjusted_dbtc_amount,
            sol_amount_added: sol_consumed,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        // Verify burn was successful by checking final balance
        let lp_balance_final = {
            use anchor_spl::token_interface::TokenAccount as TokenAccountInterface;
            let lp_account_data = lp_token_account.try_borrow_data()?;
            let lp_account = TokenAccountInterface::try_deserialize(&mut &lp_account_data[..])?;
            lp_account.amount
        };
        
        msg!("🔥 LP tokens burned: {} (Total burnt: {})", lp_tokens_minted, doge_btc_mining.pol_stats.total_lp_burnt);
        msg!("💰 Final LP token balance: {} (should equal initial: {})", lp_balance_final, lp_balance_before);
        
        // Ensure all LP tokens were properly burned
        require_eq!(lp_balance_final, lp_balance_before, ErrorCode::IncompleteTokenBurn);
    } else {
        msg!("⚠️ No LP tokens were minted, skipping burn");
    }
    
    // Calculate and store LP token price in SOL terms
    // LP price = (SOL_in_pool + DBTC_in_pool * DBTC_price) / LP_supply
    let sol_vault_balance_final = {
        let account_data = sol_vault.try_borrow_data()?;
        let token_account = anchor_spl::token::TokenAccount::try_deserialize(&mut &account_data[..])?;
        token_account.amount
    };
    
    let dbtc_vault_balance_final = {
        let account_data = dbtc_vault.try_borrow_data()?;
        let token_account = anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &account_data[..])?;
        token_account.amount
    };
    
    let lp_supply_final = {
        let pool_data = pool_state.try_borrow_data()?;
        let lp_supply_offset = 8 + 10 * 32 + 5;
        if pool_data.len() >= lp_supply_offset + 8 {
            u64::from_le_bytes([
                pool_data[lp_supply_offset],
                pool_data[lp_supply_offset + 1],
                pool_data[lp_supply_offset + 2],
                pool_data[lp_supply_offset + 3],
                pool_data[lp_supply_offset + 4],
                pool_data[lp_supply_offset + 5],
                pool_data[lp_supply_offset + 6],
                pool_data[lp_supply_offset + 7],
            ])
        } else {
            0
        }
    };
    
    // Calculate LP token price if we have valid data
    if lp_supply_final > 0 {
        // Get dBTC price from the recent_price (already in 9-decimal precision)
        let dbtc_price = doge_btc_mining.recent_price;
        
        // Calculate total value in pool in SOL terms (9-decimal precision)
        // SOL value = sol_vault_balance (9 decimals)
        // DBTC value in SOL = dbtc_vault_balance (6 decimals) * dbtc_price (9 decimals) / 10^6
        let sol_value = sol_vault_balance_final; // Already in 9-decimal precision (lamports)
        
        let dbtc_value_in_sol = if dbtc_price > 0 {
            // (dbtc_vault * dbtc_price) / 10^6
            // This gives us SOL value with 9-decimal precision
            (dbtc_vault_balance_final as u128)
                .checked_mul(dbtc_price as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(1_000_000) // DBTC has 6 decimals
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .min(u64::MAX as u128) as u64
        } else {
            0
        };
        
        let total_pool_value_sol = sol_value
            .checked_add(dbtc_value_in_sol)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        // LP token price = total_pool_value / lp_supply
        // Result in 9-decimal precision (SOL per LP token)
        doge_btc_mining.lp_token_price_in_sol = (total_pool_value_sol as u128)
            .checked_mul(1_000_000_000) // Scale to 9 decimals
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(lp_supply_final as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .min(u64::MAX as u128) as u64;
        
        let lp_price_actual = doge_btc_mining.lp_token_price_in_sol as f64 / 1_000_000_000.0;
        msg!("💎 LP token price updated: {} (9-decimal precision), Actual: {:.9} SOL per LP token", 
             doge_btc_mining.lp_token_price_in_sol, lp_price_actual);
        msg!("   Pool composition: {} SOL + {} DBTC (worth {} SOL) = {} SOL total value",
             sol_vault_balance_final as f64 / 1_000_000_000.0,
             dbtc_vault_balance_final as f64 / 1_000_000.0,
             dbtc_value_in_sol as f64 / 1_000_000_000.0,
             total_pool_value_sol as f64 / 1_000_000_000.0);
    } else {
        msg!("⚠️ LP supply is 0, cannot calculate LP token price");
    }
    
    msg!("✅ LP addition and burn completed successfully");
    
    Ok(())
}



// ----------------------------------------------------------------------------------------
// ------------ DYNAMIC DISTRIBUTION FUNCTIONS :: ORACLE & RATE UPDATES -----------------
// ----------------------------------------------------------------------------------------
 

/// Calculate price change percentage between old and new price
/// Returns (change_pct, direction) where direction: 1=increase, -1=decrease, 0=same
fn calculate_price_change_pct(old_price: u64, new_price: u64) -> (i64, i64) {
    if old_price == 0 || new_price == 0 {
        return (0, 0);
    }
    
    let old = old_price as i128;
    let new = new_price as i128;
    
    // Calculate percentage change: ((new - old) / old) × 100
    let diff = new - old;
    let change_pct = (diff * 100) / old;
    
    let direction = if new > old {
        1
    } else if new < old {
        -1
    } else {
        0
    };
    
    (change_pct as i64, direction)
}



// ----------------------------------------------------------------------------------------
// ------------ DYNAMIC DISTRIBUTION ACCOUNT STRUCTS ------------------------------------
// ----------------------------------------------------------------------------------------

 

#[derive(Accounts)]
pub struct UpdateMdogeDistPerSlot<'info> {
    #[account(
        mut,
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump = doge_btc_mining.bump,
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,
    
    /// GlobalConfig for admin authority verification
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    /// Authority (optional - only required when lp_token_amount > 0)
    pub authority: Option<Signer<'info>>,
    
    /// CHECK: Raydium CP-Swap program
    pub raydium_program: UncheckedAccount<'info>,
    
    /// CHECK: Raydium pool state
    #[account(mut)]
    pub pool_state: UncheckedAccount<'info>,
    
    /// CHECK: Raydium AMM config
    pub amm_config: UncheckedAccount<'info>,
    
    /// CHECK: Vault authority PDA (our program's authority for token accounts)
    #[account(
        seeds = [DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump,
    )]
    pub authority_pda: UncheckedAccount<'info>,
    
    /// CHECK: Raydium's pool authority PDA (from Raydium program)
    pub raydium_authority: UncheckedAccount<'info>,
    
    /// CHECK: DOGE_BTC vault in Raydium pool
    #[account(mut)]
    pub dbtc_vault: UncheckedAccount<'info>,
    
    /// CHECK: SOL vault in Raydium pool
    #[account(mut)]
    pub sol_vault: UncheckedAccount<'info>,
    
    /// CHECK: DOGE_BTC token account for swapping
    #[account(mut)]
    pub dbtc_token_account: UncheckedAccount<'info>,
    
    /// CHECK: SOL token account for receiving
    #[account(mut)]
    pub sol_token_account: UncheckedAccount<'info>,
    
    /// CHECK: DOGE_BTC mint
    pub dbtc_mint: UncheckedAccount<'info>,
    
    /// CHECK: SOL mint (WSOL)
    pub sol_mint: UncheckedAccount<'info>,
    
    /// CHECK: Raydium observation state
    #[account(mut)]
    pub observation_state: UncheckedAccount<'info>,
    
    /// CHECK: SOL treasury to receive swapped SOL
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump,
    )]
    pub sol_treasury: UncheckedAccount<'info>,
    
    /// CHECK: LP token account for receiving and burning LP tokens (can be any valid token account)
    #[account(mut)]
    pub lp_token_account: UncheckedAccount<'info>,
        
    /// CHECK: LP mint from Raydium pool (must be writable for minting)
    #[account(mut)]
    pub lp_mint: UncheckedAccount<'info>,
    
    /// Token-2022 program for DOGE_BTC
    pub token_program_2022: Program<'info, Token2022>,
    
    /// Standard token program for SOL
    pub token_program: Program<'info, anchor_spl::token::Token>,
}