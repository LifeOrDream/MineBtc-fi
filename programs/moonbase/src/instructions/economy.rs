use crate::errors::ErrorCode;
use crate::events::*;
use crate::state::*;
use anchor_lang::prelude::*;

use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::TokenAccount as TokenAccount2022; // ← the PROGRAM-ID wrapper (implements Id)
use anchor_spl::{
    token::{self, Token, TokenAccount},           // <- gives you TokenAccount type
    associated_token::{self, AssociatedToken},     // <- gives you AssociatedToken program type
};

// Import Raydium CP-Swap for CPI calls
use raydium_cp_swap;



pub fn distribute_sol_fees_internal(ctx: Context<DistributeSolFees>) -> Result<()> {
    let sol_treasury = &ctx.accounts.sol_treasury;
    let global_config = &ctx.accounts.global_config;
    let buybacks_ac = &mut ctx.accounts.buybacks_account;

    msg!("Withdrawing SOL from treasury");
    msg!("SOL Treasury: {}", sol_treasury.key());
    msg!("Treasury balance: {} SOL", sol_treasury.lamports() as f64 / 1e9);

    let rent_exempt_amount = Rent::get()?.minimum_balance(sol_treasury.data_len());
    let current_balance = sol_treasury.lamports();

    // Calculate available balance (total - rent)
    let reserved_amount = rent_exempt_amount;
    let available_solana = current_balance.saturating_sub(reserved_amount);

    // Check if we have enough available balance
    if available_solana == 0 {
        msg!( "⚠️ No SOL balance to withdraw. Available: {} SOL", available_solana as f64 / 1e9);        
        return Ok(());
    }
    msg!( "   Total balance: {} SOL, Rent: {} SOL", current_balance as f64 / 1e9, rent_exempt_amount as f64 / 1e9);

    // Calculate buybacks amount using configurable percentage
    let buyback_percentage = global_config.sol_fee_config.buyback_pct as u64;
    let sol_for_buybacks = available_solana * buyback_percentage / M_HUNDRED;

    // Create signer seeds for sol_treasury
    let treasury_seeds = &[SOL_TREASURY_SEED.as_ref(), &[ctx.bumps.sol_treasury]];
    let signer_seeds = &[&treasury_seeds[..]];

    // Transfer buybacks amount to buybacks SOL vault
    if sol_for_buybacks > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.sol_treasury.to_account_info(),
                    to: ctx.accounts.buybacks_sol_vault.to_account_info(),
                },
                signer_seeds,
            ),
            sol_for_buybacks,
        )?;

        // Update buybacks tracking
        buybacks_ac.total_sol_accumulated = buybacks_ac.total_sol_accumulated + sol_for_buybacks;

        msg!("💰 Transferred {} SOL to buybacks vault ({}%)", sol_for_buybacks as f64 / 1e9,  buyback_percentage);
    }
 
    let dev_earnings = available_solana.saturating_sub(sol_for_buybacks);
    if dev_earnings > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.sol_treasury.to_account_info(),
                    to: ctx.accounts.fee_recipient.to_account_info(),
                },
                signer_seeds,
            ),
            dev_earnings,
        )?;
        msg!("👨‍💻 Sent {} SOL to Dev Earnings", dev_earnings as f64 / 1e9);
    }

    // Emit event
    emit!(SolFeesWithdrawn {
        available_solana: available_solana,
        buyback_amount: sol_for_buybacks,
        dev_earnings_amount: dev_earnings,
    });

    msg!("Withdrew {} SOL from treasury", available_solana as f64 / 1e9);
    Ok(())
}


/// INSTRUCTION 1: Take a price snapshot (can be called by anyone every 30 minutes)
/// Performs a small SOL → DOGE_BTC swap for price discovery and earnmarks SOL for POL
pub fn snapshot_price_internal(ctx: Context<SnapshotPrice>) -> Result<()> {
    msg!("🌟 === STARTING PRICE SNAPSHOT ===");

    let doge_btc_mining: &mut Account<'_, DogeBtcMining> = &mut ctx.accounts.doge_btc_mining;
    let current_time = Clock::get()?.unix_timestamp;

    msg!("   📅 Current timestamp: {}", current_time);
    msg!(
        "   ⏰ Last update timestamp: {}",
        doge_btc_mining.last_rate_update
    );

    require!( doge_btc_mining.price_history.len() < 8, ErrorCode::UpdateDistRateFirst);

    // SECURITY: Validate that the provided pool_state matches the authorized pool in global_config
    require!(
        ctx.accounts.global_config.raydium_pool_state != Pubkey::default(),
        ErrorCode::InvalidAccount
    );
    require!(
        ctx.accounts.pool_state.key() == ctx.accounts.global_config.raydium_pool_state,
        ErrorCode::InvalidAccount
    );

    // Check if at least 30 minutes has passed since last snapshot
    msg!("\n   ⏱️ Checking time constraints...");
    let thirty_mins = THIRTY_MINS as i64;
    if current_time < doge_btc_mining.last_rate_update + thirty_mins {
        msg!("   ⏰ Update too early - must wait at least 30 minutes between updates");
        msg!(
            "      Next update allowed: {}",
            doge_btc_mining.last_rate_update + thirty_mins
        );
        msg!(
            "      Time remaining: {} seconds",
            (doge_btc_mining.last_rate_update + thirty_mins - current_time)
        );
        return Ok(());
    }

    msg!(
        "   ✅ Time constraint satisfied ({}s since last update)",
        current_time - doge_btc_mining.last_rate_update
    );

    msg!("\n🔄 === PROCESSING DISTRIBUTION RATE UPDATE ===");

    // Read buybacks SOL vault balance
    msg!("\n💰 === CHECKING BUYBACKS VAULT ===");
    let buybacks_vault_balance = ctx.accounts.buybacks_sol_vault.lamports();
    let buybacks_account = &mut ctx.accounts.buybacks_account;

    msg!(
        "   💳 Buybacks vault: {}",
        ctx.accounts.buybacks_sol_vault.key()
    );
    msg!(
        "   💰 Raw balance: {} lamports ({} SOL)",
        buybacks_vault_balance,
        buybacks_vault_balance as f64 / 1e9
    );

    // Calculate rent-exempt minimum for the buybacks vault
    let rent = Rent::get()?;
    let buybacks_vault_data_len = ctx.accounts.buybacks_sol_vault.data_len();
    let buybacks_vault_rent_exempt = rent.minimum_balance(buybacks_vault_data_len);

    msg!(
        "   💎 Rent-exempt minimum: {} lamports ({} SOL)",
        buybacks_vault_rent_exempt,
        buybacks_vault_rent_exempt as f64 / 1e9
    );
    msg!(
        "   💰 Previously earnmarked POL: {} lamports ({} SOL)",
        buybacks_account.sol_for_pol,
        buybacks_account.sol_for_pol as f64 / 1e9
    );

    // Calculate available SOL (subtract rent-exempt minimum and already earnmarked SOL for POL)
    let available_sol = buybacks_vault_balance
        .saturating_sub(buybacks_vault_rent_exempt)
        .saturating_sub(buybacks_account.sol_for_pol);

    msg!(
        "   ✅ Available SOL: {} lamports ({} SOL)",
        available_sol,
        available_sol as f64 / 1e9
    );
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

    msg!(
        "   📊 Price snapshot {}/8: Planning SOL → DOGE_BTC swap",
        doge_btc_mining.price_history.len() + 1
    );
    msg!(
        "   💵 SOL for swap: {} lamports ({} SOL) [10% of available]",
        sol_for_swap,
        sol_for_swap as f64 / 1e9
    );
    msg!(
        "   💰 SOL for POL earnmark: {} lamports ({} SOL) [10% of available]",
        sol_for_pol_earnmark,
        sol_for_pol_earnmark as f64 / 1e9
    );
    msg!(
        "   📊 Total SOL to be used: {} lamports ({} SOL)",
        sol_for_swap + sol_for_pol_earnmark,
        (sol_for_swap + sol_for_pol_earnmark) as f64 / 1e9
    );

    // Transfer SOL from buybacks vault to sol_token_account for swap
    if sol_for_swap > 0 {
        msg!("\n💸 === TRANSFERRING SOL FOR SWAP ===");
        msg!(
            "   📤 From: Buybacks vault ({})",
            ctx.accounts.buybacks_sol_vault.key()
        );
        msg!(
            "   📥 To: SOL token account ({})",
            ctx.accounts.sol_token_account.key()
        );
        msg!(
            "   💵 Amount: {} lamports ({} SOL)",
            sol_for_swap,
            sol_for_swap as f64 / 1e9
        );

        msg!(
            "   🔑 Using buybacks vault PDA seeds with bump: {}",
            ctx.bumps.buybacks_sol_vault
        );

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
        anchor_spl::token::sync_native(CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::SyncNative {
                account: ctx.accounts.sol_token_account.to_account_info(),
            },
        ))?;

        msg!("   ✅ WSOL sync completed");
        msg!(
            "   💰 Successfully transferred {} SOL and synced WSOL account",
            sol_for_swap as f64 / 1e9
        );
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
            &ctx.accounts.dbtc_token_account.to_account_info(),
            &ctx.accounts.sol_token_account.to_account_info(),
            &ctx.accounts.dbtc_mint,
            &ctx.accounts.sol_mint,
            &ctx.accounts.observation_state,
            &ctx.accounts.token_program_2022,
            &ctx.accounts.token_program,
            sol_for_swap,
            doge_btc_mining.vault_auth_bump,
        )?;
        msg!(
            "   ✅ Swap completed: Received {} DOGE_BTC ({} DOGE_BTC)",
            received,
            received as f64 / 1e6
        );
        received
    } else {
        msg!("   ⚠️ No SOL to swap, skipping");
        0
    };

    // Calculate current price (SOL per DOGE_BTC) with proper decimal handling
    msg!("\n📊 === CALCULATING NEW PRICE ===");
    msg!("   🧮 Price calculation:");
    msg!(
        "      SOL swapped: {} lamports ({} SOL)",
        sol_for_swap,
        sol_for_swap as f64 / 1e9
    );
    msg!(
        "      DOGE_BTC received: {} units ({} DOGE_BTC)",
        dbtc_received,
        dbtc_received as f64 / 1e6
    );

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
    msg!(
        "   📈 Added price entry to history. Total entries: {}/8",
        doge_btc_mining.price_history.len()
    );

    // Earnmark SOL for POL in buybacks account
    msg!("\n💰 === EARNMARKING SOL FOR POL ===");
    let previous_pol = buybacks_account.sol_for_pol;
    buybacks_account.sol_for_pol = buybacks_account
        .sol_for_pol
        .checked_add(sol_for_pol_earnmark)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   💵 Earnmarking: {} lamports ({} SOL)",
        sol_for_pol_earnmark,
        sol_for_pol_earnmark as f64 / 1e9
    );
    msg!(
        "   📊 Previous POL balance: {} lamports ({} SOL)",
        previous_pol,
        previous_pol as f64 / 1e9
    );
    msg!(
        "   ✅ New POL balance: {} lamports ({} SOL)",
        buybacks_account.sol_for_pol,
        buybacks_account.sol_for_pol as f64 / 1e9
    );

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

        msg!(
            "   Entry {}: Price={}, Weight={}, Contribution={}",
            i + 1,
            entry.price,
            weight,
            price_contribution
        );
    }

    let current_weighted_avg = if total_weights > 0 {
        (weighted_sum / total_weights).min(u64::MAX as u128) as u64
    } else {
        current_price
    };

    msg!("   📊 Weighted sum: {}", weighted_sum);
    msg!("   📊 Total weights: {}", total_weights);
    msg!(
        "   ✅ Weighted average price: {} ({:.9} SOL per DOGE_BTC)",
        current_weighted_avg,
        current_weighted_avg as f64 / 1e9
    );

    // Update recent price with current weighted average
    doge_btc_mining.recent_price = current_weighted_avg;

    // Update timestamp for next snapshot
    doge_btc_mining.last_rate_update = current_time;

    msg!("\n✅ === PRICE SNAPSHOT COMPLETE ===");
    msg!(
        "   📊 Snapshot {}/8 recorded",
        doge_btc_mining.price_history.len()
    );
    msg!("   💰 DOGE_BTC received from swap: {}", dbtc_received);
    msg!(
        "   💎 SOL earnmarked for POL: {} SOL",
        buybacks_account.sol_for_pol as f64 / 1e9
    );
    msg!("   ⏱️  Next snapshot available in: ~30 minutes");

    // Emit price snapshot event for off-chain indexing
    emit!(PriceSnapshotTaken {
        snapshot_number: doge_btc_mining.price_history.len() as u8,
        sol_swapped: sol_for_swap,
        dbtc_received,
        current_price,
        weighted_avg_price: current_weighted_avg,
        sol_earnmarked_for_pol: sol_for_pol_earnmark,
        total_pol_balance: buybacks_account.sol_for_pol,
        price_history_count: doge_btc_mining.price_history.len() as u8,
        timestamp: current_time,
    });

    Ok(())
}

/// INSTRUCTION 2: Update distribution rate and add liquidity (can be called by anyone after 4 hours)
/// Checks if conditions are met, updates distribution rate, and adds liquidity to pool
pub fn update_rate_and_add_lp_internal(
    ctx: Context<UpdateRateAndAddLp>,
    lp_token_amount: u64,
) -> Result<()> {
    msg!("🌟 === STARTING RATE UPDATE AND LP ADDITION ===");

    let doge_btc_mining = &mut ctx.accounts.doge_btc_mining;
    let buybacks_account = &mut ctx.accounts.buybacks_account;
    let current_time = Clock::get()?.unix_timestamp;

    msg!("   📅 Current timestamp: {}", current_time);
    msg!(
        "   ⚙️  Current distribution rate: {} DOGE_BTC per slot",
        doge_btc_mining.current_dist_rate
    );
    msg!("   🎯 Admin LP override: {}", lp_token_amount);

    // SECURITY: Validate that the provided pool_state matches the authorized pool in global_config
    msg!("\n   🔒 Validating Raydium pool authorization...");
    require!(
        ctx.accounts.global_config.raydium_pool_state != Pubkey::default(),
        ErrorCode::InvalidAccount
    );
    require!(
        ctx.accounts.pool_state.key() == ctx.accounts.global_config.raydium_pool_state,
        ErrorCode::InvalidAccount
    );
    msg!("   ✅ Pool validation passed!");

    // Check if admin override is being used (when lp_token_amount > 0)
    if lp_token_amount > 0 {
        msg!("\n   🔧 Admin override detected");
        require!(ctx.accounts.authority.is_some(), ErrorCode::Unauthorized);
        let authority = ctx.accounts.authority.as_ref().unwrap();
        require!(
            ctx.accounts.global_config.ext_authority == authority.key(),
            ErrorCode::Unauthorized
        );
        msg!("   ✅ Admin authority validated");
        msg!("   🎯 Using LP token amount: {}", lp_token_amount);
    } else {
        msg!("\n   🔄 Using automatic LP calculation (no admin override)");
    }

    // ----------------------------------------------------
    // Check if 4 hours have passed AND we have 8 price entries
    // Only then check if distribution rate should change
    // ----------------------------------------------------
    msg!("\n⏱️ === CHECKING DISTRIBUTION RATE UPDATE CONDITIONS ===");
    let four_hours = FOUR_HOURS as i64;
    msg!("   🎯 4-hour threshold: {}s", four_hours);
    let time_since_last = doge_btc_mining
        .price_history
        .first()
        .map(|e| current_time - e.timestamp)
        .unwrap_or(0);
    msg!(
        "   ⏰ Time since first snapshot: {}s ({}h)",
        time_since_last,
        time_since_last as f64 / 3600.0
    );

    if doge_btc_mining.price_history.len() < 8 || time_since_last < four_hours {
        msg!("   ❌ Conditions NOT met for distribution rate update:");
        if doge_btc_mining.price_history.len() < 8 {
            msg!(
                "      Need {} more price snapshots",
                8 - doge_btc_mining.price_history.len()
            );
        }
        if time_since_last < four_hours {
            msg!(
                "      Need to wait {} more seconds ({} hours)",
                four_hours - time_since_last,
                (four_hours - time_since_last) as f64 / 3600.0
            );
        }
        msg!("   ⏸️ Conditions not met - call snapshot_price instead");
        return Ok(());
    }

    // ----------------------------------------------------
    // 4 hours completed - Check if rate should change
    // ----------------------------------------------------
    msg!(
        "   ✅ 4-hour cycle complete with {} snapshots",
        doge_btc_mining.price_history.len()
    );

    // Recalculate weighted average from price history
    msg!("\n📊 === RECALCULATING WEIGHTED AVERAGE PRICE ===");
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

        msg!(
            "   Entry {}: Price={}, Weight={}, Contribution={}",
            i + 1,
            entry.price,
            weight,
            price_contribution
        );
    }

    let new_avg_price = if total_weights > 0 {
        (weighted_sum / total_weights).min(u64::MAX as u128) as u64
    } else {
        doge_btc_mining.recent_price
    };

    msg!("   📊 Weighted sum: {}", weighted_sum);
    msg!("   📊 Total weights: {}", total_weights);
    msg!(
        "   ✅ Weighted average price: {} ({:.9} SOL per DOGE_BTC)",
        new_avg_price,
        new_avg_price as f64 / 1e9
    );

    // Calculate price change percentage from BOTH recent and track prices
    // Use the LARGER change to determine if we should update
    let change_from_track = calculate_price_change_pct(doge_btc_mining.track_price, new_avg_price);

    // For recent_price, use the oldest entry in history (4 hours ago)
    let recent_comparison_price = doge_btc_mining
        .price_history
        .first()
        .map(|e| e.price)
        .unwrap_or(new_avg_price);
    let change_from_recent = calculate_price_change_pct(recent_comparison_price, new_avg_price);

    msg!(
        "   📊 Price changes: from track_price ({}): {}%, from 4h ago ({}): {}%",
        doge_btc_mining.track_price,
        change_from_track.0,
        recent_comparison_price,
        change_from_recent.0
    );

    // Pick the larger change (by absolute value)
    let (price_change_pct, direction) = if change_from_track.0.abs() > change_from_recent.0.abs() {
        msg!("   🎯 Using change from track_price (larger movement)");
        change_from_track
    } else {
        msg!("   🎯 Using change from 4h ago price (larger movement)");
        change_from_recent
    };

    msg!(
        "   📈 Selected price change: {}% (direction: {})",
        price_change_pct,
        direction
    );

    // Check if change exceeds 3% threshold
    let old_rate = doge_btc_mining.current_dist_rate;
    let mut rate_changed = false;

    if price_change_pct.abs() < PRICE_CHANGE_THRESHOLD as i64 {
        msg!(
            "   ➡️ Price change {}% within ±3% deadband, keeping same distribution rate",
            price_change_pct
        );
        // Don't update track_price, keep monitoring
    } else if direction > 0 {
        // Price increased by >3% - increase distribution by 1%
        doge_btc_mining.current_dist_rate = doge_btc_mining
            .current_dist_rate
            .checked_mul(101)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(100)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        msg!(
            "   📈 Price increased {}%! Increasing distribution rate by 1%",
            price_change_pct
        );
        rate_changed = true;
    } else {
        // Price decreased by >3% - decrease distribution by 3%
        doge_btc_mining.current_dist_rate = doge_btc_mining
            .current_dist_rate
            .checked_mul(97)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(100)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        msg!(
            "   📉 Price decreased {}%! Decreasing distribution rate by 3%",
            price_change_pct
        );
        rate_changed = true;
    }

    // Update track_price only if rate actually changed
    if rate_changed {
        doge_btc_mining.track_price = new_avg_price;
        msg!(
            "   🎯 Updated track_price to: {}",
            doge_btc_mining.track_price
        );
    }

    // Calculate amounts for LP addition using UPDATED distribution rate
    // Use earnmarked SOL from buybacks account
    msg!("\n💧 === LIQUIDITY POOL OPERATIONS ===");
    let total_sol_for_lp = buybacks_account.sol_for_pol;

    msg!(
        "   💰 Total SOL earnmarked for LP: {} lamports ({} SOL)",
        total_sol_for_lp,
        total_sol_for_lp as f64 / 1e9
    );
    msg!(
        "   📊 Accumulated over {} price snapshots",
        doge_btc_mining.price_history.len()
    );

    // Transfer SOL from buybacks vault to sol_token_account for LP addition
    if total_sol_for_lp > 0 {
        msg!("\n   💸 === TRANSFERRING SOL FOR LP ===");
        msg!(
            "   📤 From: Buybacks vault ({})",
            ctx.accounts.buybacks_sol_vault.key()
        );
        msg!(
            "   📥 To: SOL token account ({})",
            ctx.accounts.sol_token_account.key()
        );
        msg!(
            "   💵 Amount: {} lamports ({} SOL)",
            total_sol_for_lp,
            total_sol_for_lp as f64 / 1e9
        );

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
        anchor_spl::token::sync_native(CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::SyncNative {
                account: ctx.accounts.sol_token_account.to_account_info(),
            },
        ))?;

        msg!(
            "   ✅ Transferred {} SOL from buybacks vault to SOL token account and synced",
            total_sol_for_lp as f64 / 1e9
        );
    } else {
        msg!("   ⚠️ No SOL earnmarked for LP, skipping LP operations");
    }

    // Perform actual LP addition and burn (INLINED to avoid stack overflow)
    // DOGE_BTC will be taken from the main token vault (dbtc_token_account)
    let mut lp_tokens_minted = 0u64;
    let sol_consumed: u64;
    let dbtc_consumed: u64;
    
    if total_sol_for_lp > 0 {
        msg!("\n   🎯 === ADDING LIQUIDITY TO POOL ===");
        
        // Check available DOGE_BTC in main vault
        let available_dbtc = ctx.accounts.dbtc_token_account.amount;
        msg!("   💰 Available DOGE_BTC: {} DBTC", available_dbtc as f64 / 1e6);

        // Create signer seeds for vault authority
        let authority_seeds = &[DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref(), &[doge_btc_mining.vault_auth_bump]];
        let signer_seeds = &[&authority_seeds[..]];
        
        // Read pool state once
        let lp_balance_before = {
            let lp_account_data = ctx.accounts.lp_token_account.try_borrow_data()?;
            let lp_account = anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &lp_account_data[..])?;
            lp_account.amount
        };
        
        let sol_vault_balance = {
            let account_data = ctx.accounts.sol_vault.try_borrow_data()?;
            let token_account = anchor_spl::token::TokenAccount::try_deserialize(&mut &account_data[..])?;
            token_account.amount
        };

        let dbtc_vault_balance = {
            let account_data = ctx.accounts.dbtc_vault.try_borrow_data()?;
            let token_account = anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &account_data[..])?;
            token_account.amount
        };
        
        let lp_supply = {
            let data = ctx.accounts.lp_mint.try_borrow_data()?;
            let mint = anchor_spl::token::Mint::try_deserialize(&mut &data[..])?;
            mint.supply
        };
        
        msg!("   📊 Pool state: SOL={} SOL, DBTC={} DBTC, LP supply={} LP", 
             sol_vault_balance as f64 / 1e9, dbtc_vault_balance as f64 / 1e6, lp_supply as f64 / 1e6);
        
        // Calculate deposit amounts
        let sol_buffer = total_sol_for_lp / 50;
        let available_sol = total_sol_for_lp.saturating_sub(sol_buffer);
        
        let (estimated_lp_amount, adjusted_sol_amount, adjusted_dbtc_amount) = if lp_token_amount > 0 {
            let required_sol = if lp_supply > 0 && sol_vault_balance > 0 {
                (lp_token_amount as u128 * sol_vault_balance as u128 / lp_supply as u128) as u64
            } else {
                available_sol
            };
            let required_mdoge = if lp_supply > 0 && dbtc_vault_balance > 0 {
                (lp_token_amount as u128 * dbtc_vault_balance as u128 / lp_supply as u128) as u64
            } else {
                0
            };
            (lp_token_amount, required_sol.min(available_sol), required_mdoge + 100)
        } else {
            let lp_from_sol = (available_sol as u128 * lp_supply as u128 / sol_vault_balance as u128) as u64;
            let required_mdoge = (lp_from_sol as u128 * dbtc_vault_balance as u128 / lp_supply as u128) as u64;
            (lp_from_sol, available_sol, required_mdoge + 100)
        };
        
        let max_dbtc_with_buffer = adjusted_dbtc_amount.saturating_add(adjusted_dbtc_amount / 50);
        msg!("   💰 Deposit amounts: SOL={} SOL, DBTC={} DBTC (max with buffer)", 
             adjusted_sol_amount as f64 / 1e9, max_dbtc_with_buffer as f64 / 1e6);
        
        // Validate: DOGE_BTC needed must be less than 5% of vault balance
        require!(available_dbtc >= max_dbtc_with_buffer, ErrorCode::InsufficientTokensInVault);
        require!(max_dbtc_with_buffer < available_dbtc / 20, ErrorCode::MaxLimitError);
        
        // Execute Raydium deposit CPI
        raydium_cp_swap::cpi::deposit(
            CpiContext::new_with_signer(
                ctx.accounts.raydium_program.to_account_info(),
                raydium_cp_swap::cpi::accounts::Deposit {
                    owner: ctx.accounts.authority_pda.to_account_info(),
                    authority: ctx.accounts.raydium_authority.to_account_info(),
                    pool_state: ctx.accounts.pool_state.to_account_info(),
                    owner_lp_token: ctx.accounts.lp_token_account.to_account_info(),
                    token_0_account: ctx.accounts.sol_token_account.to_account_info(),
                    token_1_account: ctx.accounts.dbtc_token_account.to_account_info(),
                    token_0_vault: ctx.accounts.sol_vault.to_account_info(),
                    token_1_vault: ctx.accounts.dbtc_vault.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                    token_program_2022: ctx.accounts.token_program_2022.to_account_info(),
                    vault_0_mint: ctx.accounts.sol_mint.to_account_info(),
                    vault_1_mint: ctx.accounts.dbtc_mint.to_account_info(),
                    lp_mint: ctx.accounts.lp_mint.to_account_info(),
                },
                signer_seeds,
            ),
            estimated_lp_amount,
            adjusted_sol_amount,
            max_dbtc_with_buffer,
        )?;
        
        // Calculate LP tokens minted
        let lp_balance_after = {
            let lp_account_data = ctx.accounts.lp_token_account.try_borrow_data()?;
            let lp_account = anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &lp_account_data[..])?;
            lp_account.amount
        };
        lp_tokens_minted = lp_balance_after.saturating_sub(lp_balance_before);
        msg!("   ✅ LP tokens minted: {} LP", lp_tokens_minted as f64 / 1e6);

        // Calculate actual amounts consumed from deposit
        let sol_balance_after_deposit = {
            let sol_account_data = ctx.accounts.sol_token_account.try_borrow_data()?;
            let sol_token_data = anchor_spl::token::TokenAccount::try_deserialize(&mut &sol_account_data[..])?;
            sol_token_data.amount
        };
        sol_consumed = total_sol_for_lp.saturating_sub(sol_balance_after_deposit);
        
        // Read actual DOGE_BTC consumed (difference in vault balance)
        let dbtc_vault_balance_after = {
            let account_data = ctx.accounts.dbtc_vault.try_borrow_data()?;
            let token_account = anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &account_data[..])?;
            token_account.amount
        };
        dbtc_consumed = dbtc_vault_balance_after.saturating_sub(dbtc_vault_balance);
        
        msg!("   💰 Actual consumption: SOL={} SOL, DBTC={} DBTC", 
             sol_consumed as f64 / 1e9, dbtc_consumed as f64 / 1e6);

        // Emit LiquidityAdded event (before burn)
        if lp_tokens_minted > 0 {
            // Calculate LP token price: (sol_deposited + dbtc_deposited * price) / lp_tokens_minted
            let lp_token_price = if lp_tokens_minted > 0 {
                let dbtc_price = doge_btc_mining.recent_price; // 9 decimals SOL per DBTC
                let dbtc_value_in_sol = if dbtc_price > 0 {
                    (dbtc_consumed as u128)
                        .checked_mul(dbtc_price as u128)
                        .ok_or(ErrorCode::ArithmeticOverflow)?
                        .checked_div(1_000_000) // DBTC has 6 decimals
                        .ok_or(ErrorCode::ArithmeticOverflow)?
                        .min(u64::MAX as u128) as u64
                } else {
                    0
                };
                let total_value_sol = sol_consumed.checked_add(dbtc_value_in_sol).ok_or(ErrorCode::ArithmeticOverflow)?;
                (total_value_sol as u128)
                    .checked_mul(1_000_000_000)
                    .ok_or(ErrorCode::ArithmeticOverflow)?
                    .checked_div(lp_tokens_minted as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?
                    .min(u64::MAX as u128) as u64
            } else {
                0
            };

            if lp_token_price > 0 {
                doge_btc_mining.lp_token_price_in_sol = lp_token_price;
            }
            
            emit!(LiquidityAdded {
                sol_amount: sol_consumed,
                dbtc_amount: dbtc_consumed,
                lp_tokens_minted,
                lp_token_price,
                timestamp: Clock::get()?.unix_timestamp,
            });
        }

        // Burn LP tokens
        if lp_tokens_minted > 0 {
            use anchor_spl::token;
            token::burn(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    token::Burn {
                        mint: ctx.accounts.lp_mint.to_account_info(),
                        from: ctx.accounts.lp_token_account.to_account_info(),
                        authority: ctx.accounts.authority_pda.to_account_info(),
                    },
                    signer_seeds,
                ),
                lp_tokens_minted,
            )?;
            
            // Update POL stats
            doge_btc_mining.pol_stats.update_after_lp_operation(lp_tokens_minted, sol_consumed, dbtc_consumed);
            
            msg!("   💰 LP token price: {} SOL per LP", doge_btc_mining.lp_token_price_in_sol as f64 / 1e9);
            
            // Read final vault balances for event
            let sol_vault_balance_final = {
                let data = ctx.accounts.sol_vault.try_borrow_data()?;
                let ta = anchor_spl::token::TokenAccount::try_deserialize(&mut &data[..])?;
                ta.amount
            };
            let dbtc_vault_balance_final = {
                let data = ctx.accounts.dbtc_vault.try_borrow_data()?;
                let ta = anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &data[..])?;
                ta.amount
            };
            let lp_supply_after_burn = {
                let data = ctx.accounts.lp_mint.try_borrow_data()?;
                let mint = anchor_spl::token::Mint::try_deserialize(&mut &data[..])?;
                mint.supply
            };
            
            emit!(LpTokensBurned {
                lp_tokens_burned: lp_tokens_minted,
                total_lp_burnt: doge_btc_mining.pol_stats.total_lp_burnt,
                dbtc_amount_added: dbtc_consumed,
                sol_amount_added: sol_consumed,
                sol_vault_balance: sol_vault_balance_final,
                dbtc_vault_balance: dbtc_vault_balance_final,
                lp_supply: lp_supply_after_burn,
                lp_token_price: doge_btc_mining.lp_token_price_in_sol,
                timestamp: Clock::get()?.unix_timestamp,
            });
        }
        
        // Send remaining SOL back to buybacks vault
        let sol_remaining = {
            let sol_account_data = ctx.accounts.sol_token_account.try_borrow_data()?;
            let sol_token_data = anchor_spl::token::TokenAccount::try_deserialize(&mut &sol_account_data[..])?;
            sol_token_data.amount
        };
        
        if sol_remaining > 0 {
            msg!("   💸 Returning {} SOL to buybacks vault", sol_remaining as f64 / 1e9);
            
            // Close WSOL account - this unwraps SOL and sends it to destination
            // The account will be recreated with init_if_needed next time
            anchor_spl::token::close_account(CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token::CloseAccount {
                    account: ctx.accounts.sol_token_account.to_account_info(),
                    destination: ctx.accounts.buybacks_sol_vault.to_account_info(),
                    authority: ctx.accounts.authority_pda.to_account_info(),
                },
                signer_seeds,
            ))?;
            
            msg!("   ✅ Closed WSOL account and returned {} SOL to buybacks vault", sol_remaining as f64 / 1e9);
        }
        
        // Update sol_for_pol: subtract consumed SOL (remaining SOL was already returned)
        buybacks_account.sol_for_pol = buybacks_account.sol_for_pol.saturating_sub(sol_consumed);
        msg!("   ✅ Updated sol_for_pol: {} SOL (consumed {} SOL)", 
             buybacks_account.sol_for_pol as f64 / 1e9, sol_consumed as f64 / 1e9);
        
        msg!("   ✅ LP addition and burn completed successfully");
    }

    // Clear price history to restart the 4-hour cycle
    let price_history_count = doge_btc_mining.price_history.len() as u8;
    doge_btc_mining.price_history.clear();

    msg!("\n🧹 === FINALIZING UPDATE ===");
    msg!("   🔄 Cleared price history for new 4-hour accumulation cycle");

    // Update state
    doge_btc_mining.recent_price = new_avg_price; // Store as recent for next cycle
    doge_btc_mining.last_rate_update = current_time;
    msg!(
        "   📝 Updated recent price: {} DOGE_BTC per SOL",
        new_avg_price
    );
    msg!("   ⏰ Updated last rate update timestamp: {}", current_time);

    msg!("\n✅ === RATE UPDATE AND LP ADDITION COMPLETE ===");
    msg!(
        "   🎯 Distribution rate: {} -> {} ({})",
        old_rate,
        doge_btc_mining.current_dist_rate,
        if rate_changed { "CHANGED" } else { "unchanged" }
    );
    msg!(
        "   📊 Average price (4h): {} DOGE_BTC per SOL",
        new_avg_price
    );
    msg!(
        "   💎 SOL remaining for POL: {} SOL",
        buybacks_account.sol_for_pol as f64 / 1e9
    );

    // Calculate SOL used for POL (total before minus remaining after)
    let sol_for_pol_used = total_sol_for_lp.saturating_sub(buybacks_account.sol_for_pol);

    emit!(DistributionRateUpdated {
        old_rate,
        new_rate: doge_btc_mining.current_dist_rate,
        price_change_pct: price_change_pct as i32,
        current_price: new_avg_price,
        avg_price_4h: new_avg_price,
        track_price: doge_btc_mining.track_price,
        recent_price: doge_btc_mining.recent_price,
        rate_changed,
        sol_received: 0, // No swap in this instruction
        price_history_count,
        sol_for_pol_used,
        sol_for_pol_remaining: buybacks_account.sol_for_pol,
        lp_tokens_burned: lp_tokens_minted,
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
    msg!(
        "   💵 Input amount: {} lamports ({} SOL)",
        sol_amount_in,
        sol_amount_in as f64 / 1e9
    );
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

    msg!(
        "   💰 DOGE_BTC balance BEFORE swap: {} ({} DOGE_BTC)",
        dbtc_balance_before,
        dbtc_balance_before as f64 / 1e6
    );

    // Create signer seeds for vault authority
    msg!("   🔑 Creating signer seeds for vault authority PDA...");
    let authority_seeds = &[DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref(), &[vault_auth_bump]];
    let signer_seeds = &[&authority_seeds[..]];
    msg!("   ✅ Signer seeds created with bump: {}", vault_auth_bump);

    // Create CPI context for Raydium swap
    msg!("   📦 Setting up CPI accounts for Raydium swap...");
    let cpi_accounts = cpi::accounts::Swap {
        payer: authority_pda.to_account_info(), // Our PDA as the payer/signer
        authority: raydium_authority.to_account_info(), // Raydium's pool authority PDA
        amm_config: amm_config.to_account_info(),
        pool_state: pool_state.to_account_info(),
        input_token_account: sol_token_account.to_account_info(), // Our SOL token account (authority = our PDA)
        output_token_account: dbtc_token_account.to_account_info(), // Our DOGE_BTC token account (authority = our PDA)
        input_vault: sol_vault.to_account_info(),                   // Raydium's SOL vault
        output_vault: dbtc_vault.to_account_info(),                 // Raydium's DOGE_BTC vault
        input_token_program: token_program.to_account_info(), // Standard token program for SOL
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
    msg!(
        "   🎯 Min amount out set to: {} (accepting any amount for price discovery)",
        min_amount_out
    );

    // Perform the actual swap
    msg!("   🚀 Executing Raydium swap CPI...");
    msg!("      Input: {} SOL", sol_amount_in as f64 / 1e9);
    msg!(
        "      Min output: {} DOGE_BTC (accepting any amount)",
        min_amount_out
    );
    cpi::swap_base_input(cpi_ctx, sol_amount_in, min_amount_out)?;
    msg!("   ✅ Raydium swap CPI completed successfully");

    // Calculate actual DOGE_BTC received by checking token account balance again
    msg!("   📊 Reading DOGE_BTC token account balance after swap...");
    let dbtc_received = {
        use anchor_spl::token_interface::TokenAccount as TokenAccountInterface;
        let dbtc_account_data_after = dbtc_token_account.try_borrow_data()?;
        let dbtc_token_data_after =
            TokenAccountInterface::try_deserialize(&mut &dbtc_account_data_after[..])?;
        let dbtc_balance_after = dbtc_token_data_after.amount;
        msg!(
            "   💰 DOGE_BTC balance AFTER swap: {} ({} DOGE_BTC)",
            dbtc_balance_after,
            dbtc_balance_after as f64 / 1e6
        );
        let received = dbtc_balance_after.saturating_sub(dbtc_balance_before);
        msg!(
            "   📈 DOGE_BTC received: {} ({} DOGE_BTC)",
            received,
            received as f64 / 1e6
        );
        received
    }; // Borrow is dropped here

    msg!("✅ === SWAP COMPLETED SUCCESSFULLY ===");
    msg!(
        "   Swapped: {} SOL → {} DOGE_BTC",
        sol_amount_in as f64 / 1e9,
        dbtc_received as f64 / 1e6
    );
    msg!(
        "   Exchange rate: {} DOGE_BTC per SOL",
        (dbtc_received as f64 / 1e6) / (sol_amount_in as f64 / 1e9)
    );

    Ok(dbtc_received)
}

// DELETED: perform_lp_addition_and_burn function - now inlined in update_rate_and_add_lp_internal
// to avoid stack overflow. The logic is directly embedded in the calling function to reduce
// stack depth and memory pressure.

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
pub struct DistributeSolFees<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: SOL treasury PDA (System Account)
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump  // Let Anchor find the correct bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,

    /// CHECK: Buybacks SOL vault PDA (System Account)
    #[account(
        mut,
        seeds = [BUYBACKS_SOL_VAULT_SEED.as_ref()],
        bump
    )]
    pub buybacks_sol_vault: UncheckedAccount<'info>,

    /// CHECK: Creation fee recipient account (receives dev earnings)
    #[account(
        mut,
        address = global_config.fee_recipient @ ErrorCode::InvalidAccount
    )]
    pub fee_recipient: UncheckedAccount<'info>,

    /// Buybacks tracking account (required)
    #[account(
        mut,
        seeds = [BUYBACKS_SEED.as_ref()],
        bump,
    )]
    pub buybacks_account: Account<'info, BuybacksAccount>,

    pub system_program: Program<'info, System>,
}
 
 


/// Account struct for taking price snapshots (Instruction 1)
/// Lighter weight - only needs swap-related accounts
#[derive(Accounts)]
pub struct SnapshotPrice<'info> {
    #[account(
        mut,
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump = doge_btc_mining.bump,
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,
    
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
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
    
    /// DOGE_BTC token vault (main vault - same as used in initialize_mining)
    #[account(
        mut,
        seeds = [DOGE_BTC_VAULT_SEED.as_ref(), doge_btc_mining.key().as_ref()],
        bump,
        constraint = dbtc_token_account.mint == dbtc_mint.key() @ ErrorCode::InvalidMint,
        constraint = dbtc_token_account.owner == authority_pda.key() @ ErrorCode::Unauthorized,
    )]
    pub dbtc_token_account: InterfaceAccount<'info, TokenAccount2022>,
    
    // === The WSOL ATA, created automatically if missing
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = sol_mint,
        associated_token::authority = authority_pda,
        associated_token::token_program = token_program,  
    )]
    pub sol_token_account: Account<'info, TokenAccount>,   // <- concrete TokenAccount type

    pub associated_token_program: Program<'info, AssociatedToken>,  
     
    /// CHECK: DOGE_BTC mint
    #[account(mut)]
    pub dbtc_mint: UncheckedAccount<'info>,
    
    /// CHECK: SOL mint (WSOL)
    #[account(mut)]
    pub sol_mint: UncheckedAccount<'info>,
    
    /// CHECK: Raydium observation state
    #[account(mut)]
    pub observation_state: UncheckedAccount<'info>,
    
    /// Token-2022 program for DOGE_BTC
    pub token_program_2022: Program<'info, Token2022>,
    
    /// Standard token program for SOL
    pub token_program: Program<'info, anchor_spl::token::Token>,
    
    /// CHECK: Buybacks SOL vault PDA (System Account - source of SOL for swaps)
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
    
    /// System program (required for SOL transfers)
    pub system_program: Program<'info, System>,

    #[account(mut)]
    pub authority: Signer<'info>,    
}

/// Account struct for updating rate and adding LP (Instruction 2)
/// Heavier weight - needs LP-related accounts and main vault
#[derive(Accounts)]
pub struct UpdateRateAndAddLp<'info> {
    #[account(
        mut,
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump = doge_btc_mining.bump,
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,
    
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
    
    /// DOGE_BTC token vault (main vault - same as used in initialize_mining)
    #[account(
        mut,
        seeds = [DOGE_BTC_VAULT_SEED.as_ref(), doge_btc_mining.key().as_ref()],
        bump,
        constraint = dbtc_token_account.mint == dbtc_mint.key() @ ErrorCode::InvalidMint,
        constraint = dbtc_token_account.owner == authority_pda.key() @ ErrorCode::Unauthorized,
    )]
    pub dbtc_token_account: InterfaceAccount<'info, TokenAccount2022>,
    
    /// CHECK: SOL token account for LP addition
    #[account(mut)]
    pub sol_token_account: UncheckedAccount<'info>,
    
    /// CHECK: DOGE_BTC mint
    #[account(mut)]
    pub dbtc_mint: UncheckedAccount<'info>,
    
    /// CHECK: SOL mint (WSOL)
    #[account(mut)]
    pub sol_mint: UncheckedAccount<'info>,
    
    /// CHECK: LP token account for receiving and burning LP tokens
    #[account(mut)]
    pub lp_token_account: UncheckedAccount<'info>,
        
    /// CHECK: LP mint from Raydium pool (must be writable for minting)
    #[account(mut)]
    pub lp_mint: UncheckedAccount<'info>,
    
    /// Token-2022 program for DOGE_BTC
    pub token_program_2022: Program<'info, Token2022>,
    
    /// Standard token program for SOL
    pub token_program: Program<'info, anchor_spl::token::Token>,
    
    /// CHECK: Buybacks SOL vault PDA (System Account - source of SOL for LP)
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
    
    /// System program (required for SOL transfers)
    pub system_program: Program<'info, System>,
}
