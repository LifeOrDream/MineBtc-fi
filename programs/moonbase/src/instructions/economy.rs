use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::ErrorCode;

use anchor_spl::token_interface::{
    TokenAccount as TokenAccount2022,
};
use anchor_spl::token_2022::Token2022;          // ← the PROGRAM-ID wrapper (implements Id)

// Import Raydium CP-Swap for CPI calls
use raydium_cp_swap;




/// Update DBTC distribution rate based on price oracle
/// This function can be called by anyone every 30 minutes
/// Distribution rate updated every 4 hours with 3% deadband
pub fn update_dbtc_dist_per_slot_internal(ctx: Context<UpdateMdogeDistPerSlot>, lp_token_amount: u64) -> Result<()> {
    msg!("🌟 === STARTING UPDATE DBTC DISTRIBUTION PER SLOT ===");
    
    let doge_btc_mining: &mut Account<'_, DogeBtcMining> = &mut ctx.accounts.doge_btc_mining;
    let current_time = Clock::get()?.unix_timestamp;
    
    msg!("   📅 Current timestamp: {}", current_time);
    msg!("   ⏰ Last update timestamp: {}", doge_btc_mining.last_rate_update);
    msg!("   🎯 Admin LP override: {}", lp_token_amount);
    
    // SECURITY: Validate that the provided pool_state matches the authorized pool in global_config
    msg!("\n   🔒 Validating Raydium pool authorization...");
    require!(
        ctx.accounts.global_config.raydium_pool_state != Pubkey::default(),
        ErrorCode::InvalidAccount
    );
    msg!("   📍 Authorized pool: {}", ctx.accounts.global_config.raydium_pool_state);
    msg!("   📍 Provided pool: {}", ctx.accounts.pool_state.key());
    require!(
        ctx.accounts.pool_state.key() == ctx.accounts.global_config.raydium_pool_state,
        ErrorCode::InvalidAccount
    );
    msg!("   ✅ Pool validation passed!");
    
    // Check if admin override is being used (when lp_token_amount > 0)
    if lp_token_amount > 0 {
        msg!("\n   🔧 Admin override detected");
        // Verify that the authority is provided and matches the global config
        require!(ctx.accounts.authority.is_some(), ErrorCode::Unauthorized);
        let authority = ctx.accounts.authority.as_ref().unwrap();
        msg!("   👤 Authority: {}", authority.key());
        msg!("   🔐 Expected authority: {}", ctx.accounts.global_config.ext_authority);
        require!(
            ctx.accounts.global_config.ext_authority == authority.key(),
            ErrorCode::Unauthorized
        );
        msg!("   ✅ Admin authority validated");
        msg!("   🎯 Using LP token amount: {}", lp_token_amount);
    } else {
        msg!("\n   🔄 Using automatic LP calculation (no admin override)");
    }
    
    // Check if at least 30 minutes has passed since last update
    msg!("\n   ⏱️ Checking time constraints...");
    let thirty_mins = THIRTY_MINS as i64;
    if current_time < doge_btc_mining.last_rate_update + thirty_mins {
        msg!("   ⏰ Update too early - must wait at least 30 minutes between updates");
        msg!("      Next update allowed: {}", doge_btc_mining.last_rate_update + thirty_mins);
        msg!("      Time remaining: {} seconds", (doge_btc_mining.last_rate_update + thirty_mins - current_time));
        return Ok(());
    }
    msg!("   ✅ Time constraint satisfied ({}s since last update)", 
         current_time - doge_btc_mining.last_rate_update);
    
    msg!("\n🔄 === PROCESSING DISTRIBUTION RATE UPDATE ===");
    msg!("   ⚙️  Current distribution rate: {} DOGE_BTC per slot", doge_btc_mining.current_dist_rate);
    
    // Read buybacks SOL vault balance
    msg!("\n💰 === CHECKING BUYBACKS VAULT ===");
    let buybacks_vault_balance = ctx.accounts.buybacks_sol_vault.lamports();
    let buybacks_account = &mut ctx.accounts.buybacks_account;
    
    msg!("   💳 Buybacks vault: {}", ctx.accounts.buybacks_sol_vault.key());
    msg!("   💰 Raw balance: {} lamports ({} SOL)", 
         buybacks_vault_balance, buybacks_vault_balance as f64 / 1e9);
    
    // Calculate rent-exempt minimum for the buybacks vault
    let rent = Rent::get()?;
    let buybacks_vault_data_len = ctx.accounts.buybacks_sol_vault.data_len();
    let buybacks_vault_rent_exempt = rent.minimum_balance(buybacks_vault_data_len);
    
    msg!("   📏 Vault data length: {} bytes", buybacks_vault_data_len);
    msg!("   💎 Rent-exempt minimum: {} lamports ({} SOL)", 
         buybacks_vault_rent_exempt, buybacks_vault_rent_exempt as f64 / 1e9);
    msg!("   💰 Previously earnmarked POL: {} lamports ({} SOL)", 
         buybacks_account.sol_for_pol, buybacks_account.sol_for_pol as f64 / 1e9);
    
    // Calculate available SOL (subtract rent-exempt minimum and already earnmarked SOL for POL)
    let available_sol = buybacks_vault_balance
        .saturating_sub(buybacks_vault_rent_exempt)
        .saturating_sub(buybacks_account.sol_for_pol);
    
    msg!("   ✅ Available SOL: {} lamports ({} SOL)", 
         available_sol, available_sol as f64 / 1e9);
    msg!("      (balance - rent_exempt - earnmarked_pol)");
    
    // Ensure we have enough SOL available
    if available_sol == 0 {
        msg!("   ⚠️ No SOL available for swaps after accounting for rent-exempt minimum");
        return Ok(());
    }
    
    // Calculate 10% for swap (SOL → DOGE_BTC), 10% for POL earnmarking
    msg!("\n💱 === CALCULATING BUYBACK AND POL AMOUNTS ===");
    let sol_for_swap = available_sol / 10; // 10% for price oracle swap
    let sol_for_pol_earnmark = available_sol / 10; // 10% for POL
    
    msg!("   📊 Price snapshot {}/8: Planning SOL → DOGE_BTC swap", 
         doge_btc_mining.price_history.len() + 1);
    msg!("   💵 SOL for swap: {} lamports ({} SOL) [10% of available]", 
         sol_for_swap, sol_for_swap as f64 / 1e9);
    msg!("   💰 SOL for POL earnmark: {} lamports ({} SOL) [10% of available]", 
         sol_for_pol_earnmark, sol_for_pol_earnmark as f64 / 1e9);
    msg!("   📊 Total SOL to be used: {} lamports ({} SOL)", 
         sol_for_swap + sol_for_pol_earnmark, (sol_for_swap + sol_for_pol_earnmark) as f64 / 1e9);
    
    // Transfer SOL from buybacks vault to sol_token_account for swap
    if sol_for_swap > 0 {
        msg!("\n💸 === TRANSFERRING SOL FOR SWAP ===");
        msg!("   📤 From: Buybacks vault ({})", ctx.accounts.buybacks_sol_vault.key());
        msg!("   📥 To: SOL token account ({})", ctx.accounts.sol_token_account.key());
        msg!("   💵 Amount: {} lamports ({} SOL)", sol_for_swap, sol_for_swap as f64 / 1e9);
        
        msg!("   🔑 Using buybacks vault PDA seeds with bump: {}", ctx.bumps.buybacks_sol_vault);
        
        // Transfer SOL using system program with PDA as signer
        // This ensures proper account balancing
        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.buybacks_sol_vault.to_account_info(),
                    to: ctx.accounts.sol_token_account.to_account_info(),
                },
                &[&[
                    BUYBACKS_SOL_VAULT_SEED.as_ref(),
                    &[ctx.bumps.buybacks_sol_vault],
                ]],
            ),
            sol_for_swap,
        )?;
        msg!("   ✅ SOL transfer completed");
        
        // Then sync the native SOL balance with the token account
        // This is required for wrapped SOL (WSOL) token accounts
        msg!("   🔄 Syncing native SOL balance for WSOL token account...");
        anchor_spl::token::sync_native(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token::SyncNative {
                    account: ctx.accounts.sol_token_account.to_account_info(),
                },
            ),
        )?;
        
        msg!("   ✅ WSOL sync completed");
        msg!("   💰 Successfully transferred {} SOL and synced WSOL account", sol_for_swap as f64 / 1e9);
    }
    
    // Perform swap via Raydium CPI to get current exchange rate (SOL → DOGE_BTC)
    msg!("\n💱 === PERFORMING RAYDIUM SWAP ===");
    let dbtc_received = if sol_for_swap > 0 {
        msg!("   🚀 Calling Raydium swap CPI...");
        let received = perform_sol_to_dbtc_swap(
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
            sol_for_swap,
            doge_btc_mining.vault_auth_bump,
        )?;
        msg!("   ✅ Swap completed: Received {} DOGE_BTC ({} DOGE_BTC)", 
             received, received as f64 / 1e6);
        received
    } else {
        msg!("   ⚠️ No SOL to swap, skipping");
        0
    };
    
    // Calculate current price (SOL per DOGE_BTC) with proper decimal handling
    msg!("\n📊 === CALCULATING NEW PRICE ===");
    msg!("   🧮 Price calculation:");
    msg!("      SOL swapped: {} lamports ({} SOL)", sol_for_swap, sol_for_swap as f64 / 1e9);
    msg!("      DOGE_BTC received: {} units ({} DOGE_BTC)", dbtc_received, dbtc_received as f64 / 1e6);
    
    // sol_for_swap is in WSOL base units (9 decimals), dbtc_received is in DOGE_BTC base units (6 decimals)
    // 
    // Formula: Price = (sol_for_swap / 10^9) / (dbtc_received / 10^6)
    // Simplified: Price = (sol_for_swap * 10^6) / (dbtc_received * 10^9)
    // To store with 9-decimal precision: multiply by 10^9
    // Final: Price = (sol_for_swap * 10^6 * 10^9) / (dbtc_received * 10^9) = (sol_for_swap * 10^6) / dbtc_received
    let current_price = if dbtc_received > 0 {
        // Prevent overflow by checking limits
        // Calculate: (sol_for_swap * 10^9) / dbtc_received
       // This gives us SOL per DOGE_BTC stored with 9-decimal precision
            (sol_for_swap as u128)
                .checked_mul(1_000_000_000) // Scale by 10^9 for full precision
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(dbtc_received as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .min(u64::MAX as u128) as u64
    } else {
        0
    };
    
    // Calculate human-readable price for logging
    // Convert back to actual SOL per DOGE_BTC
    let actual_price = current_price as f64 / 1_000_000_000.0;
    msg!("   ✅ Price calculated:");
    msg!("      Raw price (with precision): {}", current_price);
    msg!("      Actual price: {:.9} SOL per DOGE_BTC", actual_price);
    msg!("      Inverse: {:.6} DOGE_BTC per SOL", 1.0 / actual_price);
    
    // Add current price to history
    let price_entry = PriceEntry {
        timestamp: current_time,
        price: current_price,
    };
    
    // Add price entry to history
    doge_btc_mining.price_history.push(price_entry);
    msg!("   📈 Added price entry to history. Total entries: {}/8", 
         doge_btc_mining.price_history.len());
    
    // Earnmark SOL for POL in buybacks account
    msg!("\n💰 === EARNMARKING SOL FOR POL ===");
    let previous_pol = buybacks_account.sol_for_pol;
    buybacks_account.sol_for_pol = buybacks_account.sol_for_pol.checked_add(sol_for_pol_earnmark).ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("   💵 Earnmarking: {} lamports ({} SOL)", 
         sol_for_pol_earnmark, sol_for_pol_earnmark as f64 / 1e9);
    msg!("   📊 Previous POL balance: {} lamports ({} SOL)", 
         previous_pol, previous_pol as f64 / 1e9);
    msg!("   ✅ New POL balance: {} lamports ({} SOL)", 
         buybacks_account.sol_for_pol, buybacks_account.sol_for_pol as f64 / 1e9);
    
    // Calculate ongoing weighted average (even before 4 hours)
    msg!("\n📊 === CALCULATING WEIGHTED AVERAGE PRICE ===");
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
        
        msg!("   Entry {}: Price={}, Weight={}, Contribution={}", 
             i + 1, entry.price, weight, price_contribution);
    }
    
    let current_weighted_avg = if total_weights > 0 {
        (weighted_sum / total_weights).min(u64::MAX as u128) as u64
    } else {
        current_price
    };
    
    msg!("   📊 Weighted sum: {}", weighted_sum);
    msg!("   📊 Total weights: {}", total_weights);
    msg!("   ✅ Weighted average price: {} ({:.9} SOL per DOGE_BTC)", 
         current_weighted_avg, current_weighted_avg as f64 / 1e9);
    
    // Update recent price with current weighted average
    doge_btc_mining.recent_price = current_weighted_avg;
    
    
    // Update timestamp for next snapshot
    doge_btc_mining.last_rate_update = current_time;
        
    // ----------------------------------------------------
    // Check if 4 hours have passed AND we have 8 price entries
    // Only then check if distribution rate should change
    // ----------------------------------------------------
    msg!("\n⏱️ === CHECKING DISTRIBUTION RATE UPDATE CONDITIONS ===");
    let four_hours = FOUR_HOURS as i64;
    msg!("   🎯 4-hour threshold: {}s", four_hours);
    let time_since_last = doge_btc_mining.price_history.first()
        .map(|e| current_time - e.timestamp)
        .unwrap_or(0);
    msg!("   ⏰ Time since first snapshot: {}s ({}h)", 
         time_since_last, time_since_last as f64 / 3600.0);
    
    if doge_btc_mining.price_history.len() < 8 || time_since_last < four_hours {
        msg!("   ❌ Conditions NOT met for distribution rate update:");
        if doge_btc_mining.price_history.len() < 8 {
            msg!("      Need {} more price snapshots", 8 - doge_btc_mining.price_history.len());
        }
        if time_since_last < four_hours {
            msg!("      Need to wait {} more seconds ({} hours)", 
                 four_hours - time_since_last, 
                 (four_hours - time_since_last) as f64 / 3600.0);
        }
        msg!("   ⏸️ Skipping distribution rate update for now");
        msg!("\n✅ === UPDATE COMPLETE (PRICE SNAPSHOT ONLY) ===");
        return Ok(());
    }
    
    // ----------------------------------------------------
    // 4 hours completed - Check if rate should change
    // ----------------------------------------------------
    msg!("   ✅ 4-hour cycle complete with {} snapshots", doge_btc_mining.price_history.len());
    
    let new_avg_price = current_weighted_avg;
    
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
    // Use earnmarked SOL from buybacks account
    msg!("\n💧 === LIQUIDITY POOL OPERATIONS ===");
    let total_sol_for_lp = buybacks_account.sol_for_pol;
    
    msg!("   💰 Total SOL earnmarked for LP: {} lamports ({} SOL)", 
         total_sol_for_lp, total_sol_for_lp as f64 / 1e9);
    msg!("   📊 Accumulated over {} price snapshots", doge_btc_mining.price_history.len());
    
    // Transfer SOL from buybacks vault to sol_token_account for LP addition
    if total_sol_for_lp > 0 {
        msg!("\n   💸 === TRANSFERRING SOL FOR LP ===");
        msg!("   📤 From: Buybacks vault ({})", ctx.accounts.buybacks_sol_vault.key());
        msg!("   📥 To: SOL token account ({})", ctx.accounts.sol_token_account.key());
        msg!("   💵 Amount: {} lamports ({} SOL)", total_sol_for_lp, total_sol_for_lp as f64 / 1e9);
        
        // Transfer SOL using system program
        // This ensures proper account balancing
        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.buybacks_sol_vault.to_account_info(),
                    to: ctx.accounts.sol_token_account.to_account_info(),
                },
                &[&[
                    BUYBACKS_SOL_VAULT_SEED.as_ref(),
                    &[ctx.bumps.buybacks_sol_vault],
                ]],
            ),
            total_sol_for_lp,
        )?;
        
        // Then sync the native SOL balance with the token account
        // This is required for wrapped SOL (WSOL) token accounts
        anchor_spl::token::sync_native(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token::SyncNative {
                    account: ctx.accounts.sol_token_account.to_account_info(),
                },
            ),
        )?;
        
        msg!("   ✅ Transferred {} SOL from buybacks vault to LP account and synced", total_sol_for_lp as f64 / 1e9);
    } else {
        msg!("   ⚠️ No SOL earnmarked for LP, skipping LP operations");
    }
    
    // Perform actual LP addition and burn
    // The function will calculate DOGE_BTC needed based on SOL amount
    // Bought-back DOGE_BTC in dbtc_token_account will be used first
    // If more DOGE_BTC is needed, it will be transferred from dbtc_token_vault
    if total_sol_for_lp > 0 {
        msg!("\n   🎯 === ADDING LIQUIDITY TO POOL ===");
        msg!("   📊 Using bought-back DOGE_BTC first, then main vault if needed");
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
        Some(&ctx.accounts.dbtc_token_vault.to_account_info()), // Main vault for additional DOGE_BTC
        )?;
        msg!("   ✅ Successfully added liquidity and burned LP tokens");
        
        // Check actual WSOL balance after LP addition to see how much was consumed
        let wsol_balance_after_lp = {
            let sol_account_data = ctx.accounts.sol_token_account.try_borrow_data()?;
            let sol_token_data = anchor_spl::token::TokenAccount::try_deserialize(&mut &sol_account_data[..])?;
            sol_token_data.amount
        };
        msg!("   💰 WSOL balance after LP addition: {}", wsol_balance_after_lp);
        
        // Calculate how much SOL was actually consumed for LP
        let sol_consumed_for_lp = total_sol_for_lp.saturating_sub(wsol_balance_after_lp);
        msg!("   📊 SOL consumed for LP: {} SOL", sol_consumed_for_lp as f64 / 1e9);
        
        // Update buybacks POL tracking - subtract only the amount actually used
        buybacks_account.sol_for_pol = wsol_balance_after_lp; // Keep any leftover SOL for next cycle
        msg!("   🔄 Updated POL tracking: {} SOL remaining for next cycle", wsol_balance_after_lp as f64 / 1e9);
        
        msg!("   💰 LP summary: {} SOL available, {} SOL consumed, {} SOL remaining for POL", 
             total_sol_for_lp, sol_consumed_for_lp, buybacks_account.sol_for_pol);
    }
    
    // Clear price history to restart the 4-hour cycle
    doge_btc_mining.price_history.clear();
    
    msg!("\n🧹 === FINALIZING UPDATE ===");
    msg!("   🔄 Cleared price history for new 4-hour accumulation cycle");
    
    // Update state
    doge_btc_mining.recent_price = new_avg_price; // Store as recent for next cycle
    doge_btc_mining.last_rate_update = current_time;
    msg!("   📝 Updated recent price: {} DOGE_BTC per SOL", new_avg_price);
    msg!("   ⏰ Updated last rate update timestamp: {}", current_time);
    
    msg!("\n✅ === DOGE_BTC DISTRIBUTION UPDATE COMPLETE ===");
    msg!("   🎯 Distribution rate: {} -> {} ({})", 
         old_rate, doge_btc_mining.current_dist_rate,
         if rate_changed { "CHANGED" } else { "unchanged" });
    msg!("   📊 Average price (4h): {} DOGE_BTC per SOL", new_avg_price);
    msg!("   💰 DOGE_BTC received from swap: {}", dbtc_received);
    msg!("   💎 SOL earnmarked for next POL: {} SOL", buybacks_account.sol_for_pol as f64 / 1e9);
    msg!("   ⏱️  Next update available in: ~30 minutes");
    msg!("=== END UPDATE DBTC DISTRIBUTION ===\n");
    
    emit!(DistributionRateUpdated {
        old_rate,
        new_rate: doge_btc_mining.current_dist_rate,
        price_change_pct: price_change_pct as i32,
        current_price,
        avg_price_4h: new_avg_price,
        track_price: doge_btc_mining.track_price,
        recent_price: doge_btc_mining.recent_price,
        rate_changed,
        sol_received: sol_for_swap, // SOL used for swap (was swapped for DOGE_BTC)
        timestamp: current_time,
    });
    
    Ok(())
}



// /// Helper function to perform DOGE_BTC to SOL swap via Raydium CPI
// fn perform_dbtc_to_sol_swap<'info>(
//     raydium_program: &AccountInfo<'info>,
//     pool_state: &AccountInfo<'info>,
//     amm_config: &AccountInfo<'info>,
//     authority_pda: &AccountInfo<'info>,
//     raydium_authority: &AccountInfo<'info>,
//     dbtc_vault: &AccountInfo<'info>,
//     sol_vault: &AccountInfo<'info>,
//     dbtc_token_account: &AccountInfo<'info>,
//     sol_token_account: &AccountInfo<'info>,
//     dbtc_mint: &AccountInfo<'info>,
//     sol_mint: &AccountInfo<'info>,
//     observation_state: &AccountInfo<'info>,
//     token_program_2022: &AccountInfo<'info>,
//     token_program: &AccountInfo<'info>,
//     amount_in: u64,
//     vault_auth_bump: u8,
// ) -> Result<u64> {
//     use raydium_cp_swap::cpi;
    
//     msg!("🔄 Performing real Raydium swap: {} DOGE_BTC for WSOL", amount_in);
    
//     // Get WSOL token balance before swap by deserializing account data
//     let sol_balance_before = {
//         let sol_account_data = sol_token_account.try_borrow_data()?;
//         let sol_token_data = anchor_spl::token::TokenAccount::try_deserialize(&mut &sol_account_data[..])?;
//         sol_token_data.amount
//     }; // Borrow is dropped here
    
//     // Create signer seeds for vault authority
//     let authority_seeds = &[
//         DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref(),
//         &[vault_auth_bump],
//     ];
//     let signer_seeds = &[&authority_seeds[..]];
    
//     // Create CPI context for Raydium swap
//     let cpi_accounts = cpi::accounts::Swap {
//         payer: authority_pda.to_account_info(),         // Our PDA as the payer/signer
//         authority: raydium_authority.to_account_info(), // Raydium's pool authority PDA
//         amm_config: amm_config.to_account_info(),
//         pool_state: pool_state.to_account_info(),
//         input_token_account: dbtc_token_account.to_account_info(),  // Our token account (authority = our PDA)
//         output_token_account: sol_token_account.to_account_info(),   // Our token account (authority = our PDA)
//         input_vault: dbtc_vault.to_account_info(),     // Raydium's DOGE_BTC vault
//         output_vault: sol_vault.to_account_info(),      // Raydium's SOL vault  
//         input_token_program: token_program_2022.to_account_info(),   // Token-2022 for DOGE_BTC
//         output_token_program: token_program.to_account_info(),       // Standard token for SOL
//         input_token_mint: dbtc_mint.to_account_info(),
//         output_token_mint: sol_mint.to_account_info(),
//         observation_state: observation_state.to_account_info(),
//     };
    
//     let cpi_ctx = CpiContext::new_with_signer(
//         raydium_program.to_account_info(),
//         cpi_accounts,
//         signer_seeds,
//     );
    
//     // Accept any amount out since we're just getting current market price
//     let min_amount_out = 0;
    
//     // Perform the actual swap
//     cpi::swap_base_input(cpi_ctx, amount_in, min_amount_out)?;
    
//     // Calculate actual WSOL received by checking token account balance again
//     let sol_received = {
//         let sol_account_data_after = sol_token_account.try_borrow_data()?;
//         let sol_token_data_after = anchor_spl::token::TokenAccount::try_deserialize(&mut &sol_account_data_after[..])?;
//         let sol_balance_after = sol_token_data_after.amount;
//         sol_balance_after.saturating_sub(sol_balance_before)
//     }; // Borrow is dropped here
    
//     msg!("✅ Swap completed: received {} WSOL tokens", sol_received);
    
//     Ok(sol_received)
// }

/// Helper function to perform SOL to DOGE_BTC swap via Raydium CPI
fn perform_sol_to_dbtc_swap<'info>(
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
    sol_amount_in: u64,
    vault_auth_bump: u8,
) -> Result<u64> {
    use raydium_cp_swap::cpi;
    
    msg!("🔄 === STARTING SOL → DOGE_BTC SWAP ===");
    msg!("   💵 Input amount: {} lamports ({} SOL)", sol_amount_in, sol_amount_in as f64 / 1e9);
    msg!("   🏦 Raydium program: {}", raydium_program.key());
    msg!("   🏊 Pool state: {}", pool_state.key());
    msg!("   ⚙️  AMM config: {}", amm_config.key());
    msg!("   🔐 Our authority PDA: {}", authority_pda.key());
    msg!("   🔐 Raydium authority: {}", raydium_authority.key());
    
    // Get DOGE_BTC token balance before swap by deserializing account data
    msg!("   📊 Reading DOGE_BTC token account balance...");
    let dbtc_balance_before = {
        use anchor_spl::token_interface::TokenAccount as TokenAccountInterface;
        let dbtc_account_data = dbtc_token_account.try_borrow_data()?;
        let dbtc_token_data = TokenAccountInterface::try_deserialize(&mut &dbtc_account_data[..])?;
        msg!("   ✅ Successfully deserialized DOGE_BTC token account");
        dbtc_token_data.amount
    }; // Borrow is dropped here
    
    msg!("   💰 DOGE_BTC balance BEFORE swap: {} ({} DOGE_BTC)", 
         dbtc_balance_before, dbtc_balance_before as f64 / 1e6);
    
    // Create signer seeds for vault authority
    msg!("   🔑 Creating signer seeds for vault authority PDA...");
    let authority_seeds = &[
        DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref(),
        &[vault_auth_bump],
    ];
    let signer_seeds = &[&authority_seeds[..]];
    msg!("   ✅ Signer seeds created with bump: {}", vault_auth_bump);
    
    // Create CPI context for Raydium swap
    msg!("   📦 Setting up CPI accounts for Raydium swap...");
    let cpi_accounts = cpi::accounts::Swap {
        payer: authority_pda.to_account_info(),         // Our PDA as the payer/signer
        authority: raydium_authority.to_account_info(), // Raydium's pool authority PDA
        amm_config: amm_config.to_account_info(),
        pool_state: pool_state.to_account_info(),
        input_token_account: sol_token_account.to_account_info(),   // Our SOL token account (authority = our PDA)
        output_token_account: dbtc_token_account.to_account_info(),  // Our DOGE_BTC token account (authority = our PDA)
        input_vault: sol_vault.to_account_info(),      // Raydium's SOL vault
        output_vault: dbtc_vault.to_account_info(),     // Raydium's DOGE_BTC vault  
        input_token_program: token_program.to_account_info(),       // Standard token program for SOL
        output_token_program: token_program_2022.to_account_info(), // Token-2022 program for DOGE_BTC
        input_token_mint: sol_mint.to_account_info(),
        output_token_mint: dbtc_mint.to_account_info(),
        observation_state: observation_state.to_account_info(),
    };
    msg!("   ✅ CPI accounts configured");
    
    let cpi_ctx = CpiContext::new_with_signer(
        raydium_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );
    
    // Accept any amount out since we're getting current market price
    let min_amount_out = 0;
    msg!("   🎯 Min amount out set to: {} (accepting any amount for price discovery)", min_amount_out);
    
    // Perform the actual swap
    msg!("   🚀 Executing Raydium swap CPI...");
    msg!("      Input: {} SOL", sol_amount_in as f64 / 1e9);
    msg!("      Min output: {} DOGE_BTC (accepting any amount)", min_amount_out);
    cpi::swap_base_input(cpi_ctx, sol_amount_in, min_amount_out)?;
    msg!("   ✅ Raydium swap CPI completed successfully");
    
    // Calculate actual DOGE_BTC received by checking token account balance again
    msg!("   📊 Reading DOGE_BTC token account balance after swap...");
    let dbtc_received = {
        use anchor_spl::token_interface::TokenAccount as TokenAccountInterface;
        let dbtc_account_data_after = dbtc_token_account.try_borrow_data()?;
        let dbtc_token_data_after = TokenAccountInterface::try_deserialize(&mut &dbtc_account_data_after[..])?;
        let dbtc_balance_after = dbtc_token_data_after.amount;
        msg!("   💰 DOGE_BTC balance AFTER swap: {} ({} DOGE_BTC)", 
             dbtc_balance_after, dbtc_balance_after as f64 / 1e6);
        let received = dbtc_balance_after.saturating_sub(dbtc_balance_before);
        msg!("   📈 DOGE_BTC received: {} ({} DOGE_BTC)", 
             received, received as f64 / 1e6);
        received
    }; // Borrow is dropped here
    
    msg!("✅ === SWAP COMPLETED SUCCESSFULLY ===");
    msg!("   Swapped: {} SOL → {} DOGE_BTC", 
         sol_amount_in as f64 / 1e9, dbtc_received as f64 / 1e6);
    msg!("   Exchange rate: {} DOGE_BTC per SOL", 
         (dbtc_received as f64 / 1e6) / (sol_amount_in as f64 / 1e9));
    
    Ok(dbtc_received)
}





/// Helper function to add liquidity to Raydium pool and burn LP tokens
/// buyback_dbtc_vault: Optional main DOGE_BTC vault/custody account to transfer from if bought-back DOGE_BTC is insufficient
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
    buyback_dbtc_vault: Option<&AccountInfo<'info>>, // Optional main vault to transfer from if needed
) -> Result<()> {
    msg!("🏦 === STARTING LP ADDITION AND BURN ===");
    msg!("   💵 SOL amount for LP: {} lamports ({} SOL)", sol_amount, sol_amount as f64 / 1e9);
    msg!("   🏦 Raydium program: {}", raydium_program.key());
    msg!("   🏊 Pool state: {}", pool_state.key());
    msg!("   🔐 Authority PDA: {}", authority_pda.key());
    msg!("   🎯 Admin LP override: {}", admin_lp_override);
    msg!("   💰 Main DOGE_BTC vault available: {}", buyback_dbtc_vault.is_some());
    
    // Check available bought-back DOGE_BTC in dbtc_token_account
    msg!("   📊 Checking bought-back DOGE_BTC balance...");
    let bought_back_dbtc = {
        use anchor_spl::token_interface::TokenAccount as TokenAccountInterface;
        let dbtc_account_data = dbtc_token_account.try_borrow_data()?;
        let dbtc_account = TokenAccountInterface::try_deserialize(&mut &dbtc_account_data[..])?;
        dbtc_account.amount
    };
    
    msg!("   💰 Available bought-back DOGE_BTC: {} ({} DOGE_BTC)", 
         bought_back_dbtc, bought_back_dbtc as f64 / 1e6);
    
    // Create signer seeds for vault authority
    msg!("   🔑 Creating signer seeds for vault authority PDA...");
    let authority_seeds = &[
        DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref(),
        &[vault_auth_bump],
    ];
    let signer_seeds = &[&authority_seeds[..]];
    msg!("   ✅ Signer seeds created with bump: {}", vault_auth_bump);
    
    // Step 1: Get LP token balance before deposit to calculate actual minted amount
    msg!("   📊 Reading LP token balance before deposit...");
    let lp_balance_before = {
        use anchor_spl::token_interface::TokenAccount as TokenAccountInterface;
        let lp_account_data = lp_token_account.try_borrow_data()?;
        let lp_account = TokenAccountInterface::try_deserialize(&mut &lp_account_data[..])?;
        lp_account.amount
    };
    
    msg!("   💰 LP token balance before deposit: {}", lp_balance_before);
    
    // Step 2: Use actual Raydium CPI for deposit
    msg!("\n   🏦 Setting up Raydium deposit CPI...");
    msg!("   📦 Creating CPI accounts for deposit...");
    
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
    msg!("   ✅ CPI context created");
    
    // Read token vault balances directly from the token accounts
    msg!("\n   📊 Reading pool vault balances...");
    let sol_vault_balance = {
        let account_data = sol_vault.try_borrow_data()?;
        let token_account = anchor_spl::token::TokenAccount::try_deserialize(&mut &account_data[..])?;
        msg!("   💰 SOL vault balance: {} ({} SOL)", 
             token_account.amount, token_account.amount as f64 / 1e9);
        token_account.amount
    };
    
    let dbtc_vault_balance = {
        let account_data = dbtc_vault.try_borrow_data()?;
        let token_account = anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &account_data[..])?;
        msg!("   💰 DOGE_BTC vault balance: {} ({} DOGE_BTC)", 
             token_account.amount, token_account.amount as f64 / 1e6);
        token_account.amount
    };
    
    // Read LP supply from pool state (this is what Raydium uses internally)
    msg!("   📊 Reading LP supply from pool state...");
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
    msg!("   💰 LP supply from pool state: {}", lp_supply);
    msg!("\n   📊 Pool summary:");
    msg!("      SOL vault: {} ({} SOL)", sol_vault_balance, sol_vault_balance as f64 / 1e9);
    msg!("      DOGE_BTC vault: {} ({} DOGE_BTC)", dbtc_vault_balance, dbtc_vault_balance as f64 / 1e6);
    msg!("      LP supply: {}", lp_supply);
    msg!("      Pool ratio: {} DOGE_BTC per SOL", 
         (dbtc_vault_balance as f64 / 1e6) / (sol_vault_balance as f64 / 1e9));
    
    // Reserve buffer upfront to account for transfer fees and rounding
    // This ensures our calculations are based on what we can actually use
    msg!("\n   🛡️ Calculating buffer for safe deposit...");
    let sol_buffer = sol_amount / 50; // 2% buffer for transfer fees and rounding
    let available_sol = sol_amount.saturating_sub(sol_buffer);
    
    msg!("      Reserved buffer: {} SOL (2%)", sol_buffer as f64 / 1e9);
    msg!("      Available for LP: {} SOL", available_sol as f64 / 1e9);
    
    // Calculate LP tokens and adjusted amounts to maximize token usage
    msg!("\n   🧮 Calculating optimal deposit amounts...");
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
    
    // Check if we need to transfer additional DOGE_BTC from main vault
    if max_dbtc_with_buffer > bought_back_dbtc {
        let dbtc_needed = max_dbtc_with_buffer.saturating_sub(bought_back_dbtc);
        msg!("⚠️ Need {} more DOGE_BTC (have {} bought-back, need {} total)", 
             dbtc_needed, bought_back_dbtc, max_dbtc_with_buffer);
        
        // Transfer from main vault if available
        if let Some(main_vault) = buyback_dbtc_vault {
            msg!("📥 Transferring {} DOGE_BTC from main vault to dbtc_token_account", dbtc_needed);
            
            use anchor_spl::token_interface::{self as token_if, Mint as MintInterface};
            
            // Get mint decimals
            let mint_decimals = {
                let mint_data = dbtc_mint.try_borrow_data()?;
                let mint = MintInterface::try_deserialize(&mut &mint_data[..])?;
                mint.decimals
            };
            
            let transfer_ctx = CpiContext::new_with_signer(
                token_program_2022.to_account_info(),
                token_if::TransferChecked {
                    from: main_vault.to_account_info(),
                    mint: dbtc_mint.to_account_info(),
                    to: dbtc_token_account.to_account_info(),
                    authority: authority_pda.to_account_info(),
                },
                signer_seeds,
            );
            
            token_if::transfer_checked(
                transfer_ctx,
                dbtc_needed,
                mint_decimals,
            )?;
            
            msg!("✅ Transferred {} DOGE_BTC from main vault", dbtc_needed);
        } else {
            msg!("⚠️ No main vault provided - may fail if bought-back DOGE_BTC is insufficient");
            // Continue anyway - Raydium will fail if insufficient funds
        }
    } else {
        msg!("✅ Sufficient bought-back DOGE_BTC available ({} >= {})", bought_back_dbtc, max_dbtc_with_buffer);
    }
    
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
    #[account(mut)]
    pub dbtc_mint: UncheckedAccount<'info>,
    
    /// CHECK: SOL mint (WSOL)
    #[account(mut)]
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
    
    /// CHECK: Buybacks SOL vault PDA (System Account - source of SOL for swaps and POL)
    #[account(
        mut,
        seeds = [BUYBACKS_SOL_VAULT_SEED.as_ref()],
        bump
    )]
    pub buybacks_sol_vault: UncheckedAccount<'info>,
    
    /// Buybacks tracking account (required)
    #[account(
        mut,
        seeds = [BUYBACKS_SEED.as_ref()],
        bump,
    )]
    pub buybacks_account: Account<'info, BuybacksAccount>,
    
    /// CHECK: Main DOGE_BTC token vault (for transferring additional DOGE_BTC if bought-back amount is insufficient)
    #[account(
        mut,
        seeds = [DOGE_BTC_VAULT_SEED.as_ref(), doge_btc_mining.key().as_ref()],
        bump,
    )]
    pub dbtc_token_vault: InterfaceAccount<'info, TokenAccount2022>,
    
    /// System program (required for sync_native)
    pub system_program: Program<'info, System>,
}