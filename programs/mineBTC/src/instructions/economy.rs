use crate::errors::ErrorCode;
use anchor_spl::token;

// # Economy Instructions
//
// The economy loop is deliberately separate from staking claims:
// - `distribute_sol_fees_internal` moves accumulated SOL from the treasury into buybacks
//   and the protocol/dev lane.
// - `snapshot_price_internal` records the on-chain market price and earnmarks some SOL for POL.
// - `update_rate_internal` converts the snapshot window into a new `mine_btc_per_round` emission rate.
// - `add_lp_and_burn` (below in this file) consumes the earnmarked SOL together with MineBTC from the
//   emissions vault to deepen POL and burn LP.
//
// Stakers do **not** get paid directly from this file. Instead:
// - the emission rate set here is consumed later by round settlement,
// - round settlement increments faction staking reward indexes,
// - staking claim paths in `stake.rs` realize those indexes into player balances.
//
// That separation is important when debugging economics: if staking APR looks wrong, first check the
// round distribution indexes, then the claim paths, and only then the economy loop inputs here.
//

use crate::events::*;
use crate::instructions::helper;
use crate::state::*;
use anchor_lang::prelude::*;

use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::TokenAccount as TokenAccount2022; // ← the PROGRAM-ID wrapper (implements Id)
use anchor_spl::{
    associated_token::AssociatedToken, // <- gives you AssociatedToken program type
    token::{Token, TokenAccount},      // <- gives you TokenAccount type
};

// Import Raydium CP-Swap for CPI calls
use raydium_cp_swap;
use raydium_cp_swap::states::PoolState as RaydiumPoolState;
use raydium_cp_swap::utils::AccountLoad as RaydiumAccountLoad;

fn gross_up_for_token2022_fee<'info>(
    mint_account_info: &AccountInfo<'info>,
    desired_post_fee_amount: u64,
    epoch: u64,
) -> Result<u64> {
    if desired_post_fee_amount == 0 {
        return Ok(0);
    }

    let mut low = desired_post_fee_amount;
    let mut high = desired_post_fee_amount;

    loop {
        let fee_info = helper::get_token2022_transfer_fee_info(mint_account_info, high, epoch)?;
        if fee_info.post_fee_amount >= desired_post_fee_amount {
            break;
        }
        high = high.checked_mul(2).ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    while low < high {
        let mid = low + (high - low) / 2;
        let fee_info = helper::get_token2022_transfer_fee_info(mint_account_info, mid, epoch)?;
        if fee_info.post_fee_amount >= desired_post_fee_amount {
            high = mid;
        } else {
            low = mid.checked_add(1).ok_or(ErrorCode::ArithmeticOverflow)?;
        }
    }

    Ok(low)
}

pub fn distribute_sol_fees_internal(ctx: Context<DistributeSolFees>) -> Result<()> {
    crate::log_fn!("economy", "distribute_sol_fees_internal");
    let sol_treasury = &ctx.accounts.sol_treasury;
    let global_config = &ctx.accounts.global_config;
    let buybacks_ac = &mut ctx.accounts.buybacks_account;

    msg!("Withdrawing SOL from treasury");
    msg!("SOL Treasury: {}", sol_treasury.key());
    msg!(
        "Treasury balance: {} SOL",
        sol_treasury.lamports() as f64 / 1e9
    );

    let rent_exempt_amount = Rent::get()?.minimum_balance(sol_treasury.data_len());
    let current_balance = sol_treasury.lamports();

    // Calculate available balance (total - rent)
    let reserved_amount = rent_exempt_amount;
    let available_solana = current_balance.saturating_sub(reserved_amount);

    // Check if we have enough available balance
    if available_solana == 0 {
        msg!(
            "⚠️ No SOL balance to withdraw. Available: {} SOL",
            available_solana as f64 / 1e9
        );
        return Ok(());
    }
    msg!(
        "   Total balance: {} SOL, Rent: {} SOL",
        current_balance as f64 / 1e9,
        rent_exempt_amount as f64 / 1e9
    );

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
        buybacks_ac.total_sol_accumulated += sol_for_buybacks;

        msg!(
            "💰 Transferred {} SOL to buybacks vault ({}%)",
            sol_for_buybacks as f64 / 1e9,
            buyback_percentage
        );
    }

    let dev_earnings = available_solana.saturating_sub(sol_for_buybacks);
    if dev_earnings > 0 {
        // Transfer SOL from treasury to treasury WSOL account (wraps it)
        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.sol_treasury.to_account_info(),
                    to: ctx.accounts.treasury_wsol_account.to_account_info(),
                },
                signer_seeds,
            ),
            dev_earnings,
        )?;

        // Sync native account to update WSOL balance
        anchor_spl::token::sync_native(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::SyncNative {
                account: ctx.accounts.treasury_wsol_account.to_account_info(),
            },
            signer_seeds,
        ))?;

        // Transfer WSOL from treasury WSOL account to multisig WSOL account
        anchor_spl::token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token::Transfer {
                    from: ctx.accounts.treasury_wsol_account.to_account_info(),
                    to: ctx.accounts.multisig_wsol_account.to_account_info(),
                    authority: ctx.accounts.sol_treasury.to_account_info(),
                },
                signer_seeds,
            ),
            dev_earnings,
        )?;
        msg!(
            "👨‍💻 Sent {} WSOL to Multisig (Dev Earnings)",
            dev_earnings as f64 / 1e9
        );
    }

    // Emit event
    emit!(SolFeesWithdrawn {
        available_solana,
        buyback_amount: sol_for_buybacks,
        dev_earnings_amount: dev_earnings,
    });

    msg!(
        "Withdrew {} SOL from treasury",
        available_solana as f64 / 1e9
    );
    Ok(())
}

/// INSTRUCTION 1: Take a price snapshot (can be called by anyone every 30 minutes)
/// Performs a small SOL → MINE_BTC swap for price discovery and earnmarks SOL for POL
pub fn snapshot_price_internal(ctx: Context<SnapshotPrice>) -> Result<()> {
    crate::log_fn!("economy", "snapshot_price_internal");
    msg!("🌟 === STARTING PRICE SNAPSHOT ===");

    let mine_btc_mining: &mut Account<'_, MineBtcMining> = &mut ctx.accounts.mine_btc_mining;
    let current_time = Clock::get()?.unix_timestamp;

    // Check if LP operation is pending
    require!(
        !mine_btc_mining.lp_operation_pending,
        ErrorCode::InvalidAccount
    );

    msg!("   📅 Current timestamp: {}", current_time);
    msg!(
        "   ⏰ Last update timestamp: {}",
        mine_btc_mining.last_rate_update
    );

    require!(
        mine_btc_mining.price_history.len() < 8,
        ErrorCode::UpdateDistRateFirst
    );

    // SECURITY: Validate that the provided pool_state matches the authorized pool in global_config
    require!(
        ctx.accounts.global_config.raydium_pool_state != Pubkey::default(),
        ErrorCode::InvalidAccount
    );
    require!(
        ctx.accounts.pool_state.key() == ctx.accounts.global_config.raydium_pool_state,
        ErrorCode::InvalidAccount
    );

    // Check if at least snapshot_interval has passed since last snapshot
    msg!("\n   ⏱️ Checking time constraints...");
    let snapshot_interval = ctx.accounts.global_config.snapshot_interval as i64;
    if current_time < mine_btc_mining.last_rate_update + snapshot_interval {
        msg!(
            "   ⏰ Update too early - must wait at least {} seconds between updates",
            snapshot_interval
        );
        msg!(
            "      Next update allowed: {}",
            mine_btc_mining.last_rate_update + snapshot_interval
        );
        msg!(
            "      Time remaining: {} seconds",
            (mine_btc_mining.last_rate_update + snapshot_interval - current_time)
        );
        return Ok(());
    }

    msg!(
        "   ✅ Time constraint satisfied ({}s since last update, required: {}s)",
        current_time - mine_btc_mining.last_rate_update,
        snapshot_interval
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

    // Calculate 10% for swap (SOL → MINE_BTC), 10% for POL earnmarking
    msg!("\n💱 === CALCULATING BUYBACK AND POL AMOUNTS ===");
    let sol_for_swap = available_sol / 10; // 10% for price oracle swap
    let sol_for_pol_earnmark = available_sol / 10; // 10% for POL

    msg!(
        "   📊 Price snapshot {}/8: Planning SOL → MINE_BTC swap",
        mine_btc_mining.price_history.len() + 1
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

    // Perform swap via Raydium CPI to get current exchange rate (SOL → MINE_BTC)
    msg!("\n💱 === PERFORMING RAYDIUM SWAP ===");
    let minebtc_received = if sol_for_swap > 0 {
        msg!("   🚀 Calling Raydium swap CPI...");
        let received = perform_sol_to_minebtc_swap(
            &ctx.accounts.raydium_program,
            &ctx.accounts.pool_state,
            &ctx.accounts.amm_config,
            &ctx.accounts.authority_pda,
            &ctx.accounts.raydium_authority,
            &ctx.accounts.minebtc_vault,
            &ctx.accounts.sol_vault,
            &ctx.accounts.minebtc_token_account.to_account_info(),
            &ctx.accounts.sol_token_account.to_account_info(),
            &ctx.accounts.minebtc_mint,
            &ctx.accounts.sol_mint,
            &ctx.accounts.observation_state,
            &ctx.accounts.token_program_2022,
            &ctx.accounts.token_program,
            sol_for_swap,
            mine_btc_mining.vault_auth_bump,
        )?;
        msg!(
            "   ✅ Swap completed: Received {} MINE_BTC ({} MINE_BTC)",
            received,
            received as f64 / 1e6
        );
        received
    } else {
        msg!("   ⚠️ No SOL to swap, skipping");
        0
    };

    // Calculate current price (SOL per MINE_BTC) with proper decimal handling
    msg!("\n📊 === CALCULATING NEW PRICE ===");
    msg!("   🧮 Price calculation:");
    msg!(
        "      SOL swapped: {} lamports ({} SOL)",
        sol_for_swap,
        sol_for_swap as f64 / 1e9
    );
    msg!(
        "      MINE_BTC received: {} units ({} MINE_BTC)",
        minebtc_received,
        minebtc_received as f64 / 1e6
    );

    // sol_for_swap is in WSOL base units (9 decimals), minebtc_received is in MINE_BTC base units (6 decimals)
    //
    // Formula: Price = (sol_for_swap / 10^9) / (minebtc_received / 10^6)
    // Simplified: Price = (sol_for_swap * 10^6) / (minebtc_received * 10^9)
    // To store with 9-decimal precision: multiply by 10^9
    // Final: Price = (sol_for_swap * 10^6 * 10^9) / (minebtc_received * 10^9) = (sol_for_swap * 10^6) / minebtc_received
    let current_price = if minebtc_received > 0 {
        // Prevent overflow by checking limits
        // Calculate: (sol_for_swap * 10^9) / minebtc_received
        // This gives us SOL per MINE_BTC stored with 9-decimal precision
        (sol_for_swap as u128)
            .checked_mul(1_000_000_000) // Scale by 10^9 for full precision
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(minebtc_received as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .min(u64::MAX as u128) as u64
    } else {
        0
    };

    // Calculate human-readable price for logging
    // Convert back to actual SOL per MINE_BTC
    let actual_price = current_price as f64 / 1_000_000_000.0;
    msg!("   ✅ Price calculated:");
    msg!("      Raw price (with precision): {}", current_price);
    msg!("      Actual price: {:.9} SOL per MINE_BTC", actual_price);
    msg!("      Inverse: {:.6} MINE_BTC per SOL", 1.0 / actual_price);

    // Add current price to history
    let price_entry = PriceEntry {
        timestamp: current_time,
        price: current_price,
    };

    // Add price entry to history
    mine_btc_mining.price_history.push(price_entry);
    msg!(
        "   📈 Added price entry to history. Total entries: {}/8",
        mine_btc_mining.price_history.len()
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

    for (i, entry) in mine_btc_mining.price_history.iter().enumerate() {
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
        "   ✅ Weighted average price: {} ({:.9} SOL per MINE_BTC)",
        current_weighted_avg,
        current_weighted_avg as f64 / 1e9
    );

    // Update recent price with current weighted average
    mine_btc_mining.recent_price = current_weighted_avg;

    // Update timestamp for next snapshot
    mine_btc_mining.last_rate_update = current_time;

    msg!("\n✅ === PRICE SNAPSHOT COMPLETE ===");
    msg!(
        "   📊 Snapshot {}/8 recorded",
        mine_btc_mining.price_history.len()
    );
    msg!("   💰 MINE_BTC received from swap: {}", minebtc_received);
    msg!(
        "   💎 SOL earnmarked for POL: {} SOL",
        buybacks_account.sol_for_pol as f64 / 1e9
    );
    msg!("   ⏱️  Next snapshot available in: ~30 minutes");

    // Emit price snapshot event for off-chain indexing
    emit!(PriceSnapshotTaken {
        snapshot_number: mine_btc_mining.price_history.len() as u8,
        sol_swapped: sol_for_swap,
        minebtc_received,
        current_price,
        weighted_avg_price: current_weighted_avg,
        sol_earnmarked_for_pol: sol_for_pol_earnmark,
        total_pol_balance: buybacks_account.sol_for_pol,
        price_history_count: mine_btc_mining.price_history.len() as u8,
        timestamp: current_time,
    });

    Ok(())
}

/// INSTRUCTION 2a: Update distribution rate (can be called by anyone after 4 hours)
/// Checks if conditions are met, updates distribution rate, sets flag for LP operation
pub fn update_rate_internal(ctx: Context<UpdateRate>) -> Result<()> {
    crate::log_fn!("economy", "update_rate_internal");
    msg!("🌟 === STARTING RATE UPDATE ===");

    let mine_btc_mining = &mut ctx.accounts.mine_btc_mining;
    let current_time = Clock::get()?.unix_timestamp;

    msg!("   📅 Current timestamp: {}", current_time);
    msg!(
        "   ⚙️  Current distribution rate: {} MINE_BTC per round",
        mine_btc_mining.mine_btc_per_round
    );

    // Check if 4 hours have passed AND we have 8 price entries
    let time_since_last = mine_btc_mining
        .price_history
        .first()
        .map(|e| current_time - e.timestamp)
        .unwrap_or(0);

    if mine_btc_mining.price_history.len() < 8 {
        msg!(
            "   ❌ Conditions NOT met: {} snapshots, {}s elapsed",
            mine_btc_mining.price_history.len(),
            time_since_last
        );
        return Ok(());
    }

    msg!(
        "   ✅ 4-hour cycle complete with {} snapshots",
        mine_btc_mining.price_history.len()
    );

    // Calculate weighted average price
    let mut weighted_sum: u128 = 0;
    let mut total_weights: u128 = 0;
    for (i, entry) in mine_btc_mining.price_history.iter().enumerate() {
        let weight = (i + 1) as u128;
        weighted_sum += (entry.price as u128) * weight;
        total_weights += weight;
    }
    let new_avg_price = (weighted_sum / total_weights) as u64;

    // Calculate price change
    let change_from_track = calculate_price_change_pct(mine_btc_mining.track_price, new_avg_price);
    let recent_comparison_price = mine_btc_mining
        .price_history
        .first()
        .map(|e| e.price)
        .unwrap_or(new_avg_price);
    let change_from_recent = calculate_price_change_pct(recent_comparison_price, new_avg_price);
    let (price_change_pct, direction) = if change_from_track.0.abs() > change_from_recent.0.abs() {
        change_from_track
    } else {
        change_from_recent
    };

    // Update rate if threshold exceeded
    let old_rate = mine_btc_mining.mine_btc_per_round;
    let mut rate_changed = false;
    let price_change_threshold = mine_btc_mining.price_change_threshold as i64;

    if price_change_pct.abs() >= price_change_threshold {
        if direction > 0 {
            let increase_multiplier = 100u64
                .checked_add(mine_btc_mining.emission_increase_pct)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            mine_btc_mining.mine_btc_per_round = mine_btc_mining
                .mine_btc_per_round
                .checked_mul(increase_multiplier)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(100)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!(
                "   📈 Price increased {}%! Rate increased by {}%",
                price_change_pct,
                mine_btc_mining.emission_increase_pct
            );
        } else {
            let decrease_multiplier = 100u64.saturating_sub(mine_btc_mining.emission_decrease_pct);
            mine_btc_mining.mine_btc_per_round = mine_btc_mining
                .mine_btc_per_round
                .checked_mul(decrease_multiplier)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(100)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!(
                "   📉 Price decreased {}%! Rate decreased by {}%",
                price_change_pct,
                mine_btc_mining.emission_decrease_pct
            );
        }
        mine_btc_mining.track_price = new_avg_price;
        rate_changed = true;
    }

    // --- Update dynamic faction-war mining multiplier ---
    let faction_war_config = &mut ctx.accounts.faction_war_config;
    if rate_changed {
        let old_multiplier = faction_war_config.mining_multiplier_bps as u128;
        let new_multiplier = if direction > 0 {
            let increase = old_multiplier
                .checked_mul(faction_war_config.multiplier_increase_bps as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                / 10_000;
            old_multiplier
                .checked_add(increase)
                .ok_or(ErrorCode::ArithmeticOverflow)?
        } else {
            let decrease = old_multiplier
                .checked_mul(faction_war_config.multiplier_decrease_bps as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                / 10_000;
            old_multiplier.saturating_sub(decrease)
        };
        let min_bps = faction_war_config.multiplier_min_bps as u128;
        let max_bps = faction_war_config.multiplier_max_bps as u128;
        faction_war_config.mining_multiplier_bps =
            (new_multiplier.min(max_bps).max(min_bps)) as u16;
        msg!(
            "   🎯 FactionWar multiplier updated: {} bps -> {} bps (direction={})",
            old_multiplier,
            faction_war_config.mining_multiplier_bps,
            if direction > 0 { "up" } else { "down" }
        );
        emit!(FactionWarMultiplierUpdated {
            old_multiplier_bps: old_multiplier as u16,
            new_multiplier_bps: faction_war_config.mining_multiplier_bps,
            direction: if direction > 0 { 1 } else { -1 },
            timestamp: current_time,
        });
    }

    // Set LP operation pending flag and store SOL amount
    mine_btc_mining.lp_operation_pending = true;
    msg!(
        "   🎯 LP operation pending: {}",
        mine_btc_mining.lp_operation_pending
    );

    // Clear price history and update state
    mine_btc_mining.price_history.clear();
    mine_btc_mining.recent_price = new_avg_price;
    mine_btc_mining.last_rate_update = current_time;

    msg!(
        "✅ Rate update complete: {} -> {} ({})",
        old_rate,
        mine_btc_mining.mine_btc_per_round,
        if rate_changed { "CHANGED" } else { "unchanged" }
    );

    emit!(DistributionRateUpdated {
        old_rate,
        new_rate: mine_btc_mining.mine_btc_per_round,
        price_change_pct: price_change_pct as i32,
        current_price: new_avg_price,
        avg_price_4h: new_avg_price,
        track_price: mine_btc_mining.track_price,
        recent_price: mine_btc_mining.recent_price,
        rate_changed,
        new_mining_multiplier: faction_war_config.mining_multiplier_bps,
        timestamp: current_time,
    });

    Ok(())
}

/// INSTRUCTION 2b: Add liquidity and burn LP tokens (called after update_rate_internal)
/// Handles the heavy LP operations separately to avoid stack overflow
pub fn add_lp_and_burn_internal(ctx: Context<AddLpAndBurn>, lp_token_amount: u64) -> Result<()> {
    crate::log_fn!("economy", "add_lp_and_burn_internal");
    msg!("🌟 === STARTING LP ADDITION AND BURN ===");
    let clock = Clock::get()?;

    let mine_btc_mining = &mut ctx.accounts.mine_btc_mining;
    let buybacks_account = &mut ctx.accounts.buybacks_account;

    // SECURITY: Validate pool state
    require!(
        ctx.accounts.global_config.raydium_pool_state != Pubkey::default(),
        ErrorCode::InvalidAccount
    );
    require!(
        ctx.accounts.pool_state.key() == ctx.accounts.global_config.raydium_pool_state,
        ErrorCode::InvalidAccount
    );

    // Check if LP operation is pending
    require!(
        mine_btc_mining.lp_operation_pending,
        ErrorCode::InvalidAccount
    );

    // Admin override check
    if lp_token_amount > 0 {
        require!(ctx.accounts.authority.is_some(), ErrorCode::Unauthorized);
        let authority = ctx.accounts.authority.as_ref().unwrap();
        require!(
            ctx.accounts.global_config.ext_authority == authority.key(),
            ErrorCode::Unauthorized
        );
    }

    let total_sol_for_lp = buybacks_account.sol_for_pol;
    msg!(
        "   💰 SOL ready for LP: {} SOL",
        total_sol_for_lp as f64 / 1e9
    );

    if total_sol_for_lp == 0 {
        mine_btc_mining.lp_operation_pending = false;
        msg!("   ⚠️ No SOL for LP, clearing flag");
        return Ok(());
    }

    // Transfer SOL from buybacks vault to sol_token_account
    msg!(
        "\n   💸 === TRANSFERRING {} SOL FOR LP from buybacks vault to sol_token_account ===",
        total_sol_for_lp as f64 / 1e9
    );
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

    anchor_spl::token::sync_native(CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        anchor_spl::token::SyncNative {
            account: ctx.accounts.sol_token_account.to_account_info(),
        },
    ))?;

    // Read pool state
    let available_minebtc = ctx.accounts.minebtc_token_account.amount;
    let authority_seeds = &[
        MINE_BTC_VAULT_AUTHORITY_SEED.as_ref(),
        &[mine_btc_mining.vault_auth_bump],
    ];
    let signer_seeds = &[&authority_seeds[..]];

    let lp_balance_before = {
        let data = ctx.accounts.lp_token_account.try_borrow_data()?;
        anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &data[..])?.amount
    };

    let sol_vault_balance = {
        let data = ctx.accounts.sol_vault.try_borrow_data()?;
        anchor_spl::token::TokenAccount::try_deserialize(&mut &data[..])?.amount
    };

    let minebtc_vault_balance = {
        let data = ctx.accounts.minebtc_vault.try_borrow_data()?;
        anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &data[..])?.amount
    };

    let lp_supply = {
        let pool_state = RaydiumAccountLoad::<RaydiumPoolState>::load_data_mut(
            ctx.accounts.pool_state.as_ref(),
        )?;
        pool_state.lp_supply
    };

    msg!(
        "   📊 Pool state: SOL={} SOL, MINEBTC={} MINEBTC, LP supply={} LP",
        sol_vault_balance as f64 / 1e9,
        minebtc_vault_balance as f64 / 1e6,
        lp_supply as f64 / 1e9
    );

    // Calculate deposit amounts
    let sol_buffer = total_sol_for_lp / 50;
    let available_sol = total_sol_for_lp - sol_buffer;

    let (estimated_lp_amount, adjusted_sol_amount, adjusted_minebtc_amount) = if lp_token_amount > 0
    {
        let required_sol = if lp_supply > 0 && sol_vault_balance > 0 {
            (lp_token_amount as u128 * sol_vault_balance as u128 / lp_supply as u128) as u64
        } else {
            available_sol
        };
        let required_minebtc_post_fee = if lp_supply > 0 && minebtc_vault_balance > 0 {
            (lp_token_amount as u128 * minebtc_vault_balance as u128 / lp_supply as u128) as u64
        } else {
            0
        };
        let required_minebtc = gross_up_for_token2022_fee(
            &ctx.accounts.minebtc_mint.to_account_info(),
            required_minebtc_post_fee,
            clock.epoch,
        )?;
        (
            lp_token_amount,
            required_sol.min(available_sol),
            required_minebtc,
        )
    } else {
        // Guard against division by zero if pool state is empty
        if sol_vault_balance == 0 || lp_supply == 0 {
            msg!("   ⚠️ Pool vault balance is zero, skipping LP operation");
            mine_btc_mining.lp_operation_pending = false;
            return Ok(());
        }
        let lp_from_sol =
            (available_sol as u128 * lp_supply as u128 / sol_vault_balance as u128) as u64;
        let required_minebtc_post_fee =
            (lp_from_sol as u128 * minebtc_vault_balance as u128 / lp_supply as u128) as u64;
        let required_minebtc = gross_up_for_token2022_fee(
            &ctx.accounts.minebtc_mint.to_account_info(),
            required_minebtc_post_fee,
            clock.epoch,
        )?;
        (lp_from_sol, available_sol, required_minebtc)
    };

    let max_minebtc_with_buffer = adjusted_minebtc_amount + (adjusted_minebtc_amount / 50);
    require!(
        available_minebtc >= max_minebtc_with_buffer,
        ErrorCode::InsufficientTokensInVault
    );
    msg!(
        "   💰 Deposit amounts: SOL={} SOL, MINEBTC={} MINEBTC (max with buffer) for {} LP",
        adjusted_sol_amount as f64 / 1e9,
        max_minebtc_with_buffer as f64 / 1e6,
        estimated_lp_amount as f64 / 1e9
    );

    // If max_minebtc_with_buffer exceeds 5% limit, adjust SOL amount to match 1% dogeBTC limit
    let (final_sol_amount, _final_minebtc_amount, final_max_minebtc_with_buffer) =
        if max_minebtc_with_buffer >= available_minebtc / 100 {
            // 1% of available_minebtc
            msg!("   💰 Max MINEBTC with buffer exceeds 1% of available MINEBTC, adjusting SOL amount to match 1% dogeBTC limit");
            adjust_sol_for_minebtc_limit(
                available_minebtc,
                sol_vault_balance,
                minebtc_vault_balance,
                lp_supply,
                adjusted_sol_amount,
                adjusted_minebtc_amount,
                &ctx.accounts.minebtc_mint.to_account_info(),
                clock.epoch,
            )?
        } else {
            (
                adjusted_sol_amount,
                adjusted_minebtc_amount,
                max_minebtc_with_buffer,
            )
        };

    let final_estimated_lp_amount = if lp_supply > 0 && sol_vault_balance > 0 {
        let lp_from_sol =
            (final_sol_amount as u128 * lp_supply as u128 / sol_vault_balance as u128) as u64;
        lp_from_sol.saturating_sub(lp_from_sol / 100)
    } else {
        0
    };

    msg!(
        "   💰 Final estimated LP amount: {} LP for {} SOL and {} MINEBTC",
        final_estimated_lp_amount as f64 / 1e9,
        final_sol_amount as f64 / 1e9,
        final_max_minebtc_with_buffer as f64 / 1e6
    );

    // Dynamic sorting
    let mine_btc_key = ctx.accounts.minebtc_mint.key();
    let sol_key = ctx.accounts.sol_mint.key();
    let (
        token_0_vault,
        token_1_vault,
        token_0_account,
        token_1_account,
        vault_0_mint,
        vault_1_mint,
        amount_0_max,
        amount_1_max,
    ) = if mine_btc_key < sol_key {
        (
            ctx.accounts.minebtc_vault.to_account_info(),
            ctx.accounts.sol_vault.to_account_info(),
            ctx.accounts.minebtc_token_account.to_account_info(),
            ctx.accounts.sol_token_account.to_account_info(),
            ctx.accounts.minebtc_mint.to_account_info(),
            ctx.accounts.sol_mint.to_account_info(),
            final_max_minebtc_with_buffer,
            final_sol_amount,
        )
    } else {
        (
            ctx.accounts.sol_vault.to_account_info(),
            ctx.accounts.minebtc_vault.to_account_info(),
            ctx.accounts.sol_token_account.to_account_info(),
            ctx.accounts.minebtc_token_account.to_account_info(),
            ctx.accounts.sol_mint.to_account_info(),
            ctx.accounts.minebtc_mint.to_account_info(),
            final_sol_amount,
            final_max_minebtc_with_buffer,
        )
    };

    // Deposit CPI
    raydium_cp_swap::cpi::deposit(
        CpiContext::new_with_signer(
            ctx.accounts.raydium_program.to_account_info(),
            raydium_cp_swap::cpi::accounts::Deposit {
                owner: ctx.accounts.authority_pda.to_account_info(),
                authority: ctx.accounts.raydium_authority.to_account_info(),
                pool_state: ctx.accounts.pool_state.to_account_info(),
                owner_lp_token: ctx.accounts.lp_token_account.to_account_info(),
                token_0_account,
                token_1_account,
                token_0_vault,
                token_1_vault,
                vault_0_mint,
                vault_1_mint,
                token_program: ctx.accounts.token_program.to_account_info(),
                token_program_2022: ctx.accounts.token_program_2022.to_account_info(),
                lp_mint: ctx.accounts.lp_mint.to_account_info(),
            },
            signer_seeds,
        ),
        final_estimated_lp_amount,
        amount_0_max,
        amount_1_max,
    )?;

    // Calculate LP tokens minted
    let lp_balance_after = {
        let data = ctx.accounts.lp_token_account.try_borrow_data()?;
        anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &data[..])?.amount
    };
    msg!("   💰 LP balance after: {}", lp_balance_after);
    msg!("   💰 LP balance before: {}", lp_balance_before);
    let lp_tokens_minted = lp_balance_after.saturating_sub(lp_balance_before);
    msg!("   💰 LP tokens minted: {}", lp_tokens_minted);

    let sol_balance_after = {
        let data = ctx.accounts.sol_token_account.try_borrow_data()?;
        anchor_spl::token::TokenAccount::try_deserialize(&mut &data[..])?.amount
    };
    msg!("   💰 SOL balance after: {}", sol_balance_after);
    msg!("   💰 SOL balance before: {}", total_sol_for_lp);
    let sol_consumed = total_sol_for_lp.saturating_sub(sol_balance_after);
    msg!("   💰 SOL consumed: {}", sol_consumed);

    // ✅ RIGHT: Reloads fresh data from the account info
    let available_minebtc_after = {
        let account_info = ctx.accounts.minebtc_token_account.to_account_info();
        let data = account_info.try_borrow_data()?;
        anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &data[..])?.amount
    };
    msg!("   💰 MINEBTC balance after: {}", available_minebtc_after);
    msg!("   💰 MINEBTC balance before: {}", available_minebtc);
    let minebtc_consumed = available_minebtc.saturating_sub(available_minebtc_after);
    msg!("   💰 MINEBTC consumed: {}", minebtc_consumed);

    msg!(
        "   ✅ LP minted: {}, SOL consumed: {}, MINEBTC consumed: {}",
        lp_tokens_minted,
        sol_consumed,
        minebtc_consumed
    );

    // Burn LP tokens
    if lp_tokens_minted > 0 {
        let lp_token_price = {
            let minebtc_price = mine_btc_mining.recent_price;
            let minebtc_value_in_sol = if minebtc_price > 0 {
                (minebtc_consumed as u128) * (minebtc_price as u128) / 1_000_000_u128
            } else {
                0
            } as u64;
            let total_value_sol = sol_consumed + minebtc_value_in_sol;
            (total_value_sol as u128) * 1_000_000_000_u128 / (lp_tokens_minted as u128)
        };
        if lp_token_price > 0 {
            mine_btc_mining.lp_token_price_in_sol = lp_token_price as u64;
        }
        msg!(
            "   💰 LP token price: {} SOL per LP",
            lp_token_price as f64 / 1e9
        );

        emit!(LiquidityAdded {
            sol_amount: sol_consumed,
            minebtc_amount: minebtc_consumed,
            lp_tokens_minted,
            lp_token_price: lp_token_price as u64,
            timestamp: Clock::get()?.unix_timestamp
        });

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
        mine_btc_mining.pol_stats.update_after_lp_operation(
            lp_tokens_minted,
            sol_consumed,
            minebtc_consumed,
        );

        emit!(LpTokensBurned {
            lp_tokens_burned: lp_tokens_minted,
            total_lp_burnt: mine_btc_mining.pol_stats.total_lp_burnt,
            minebtc_amount_added: minebtc_consumed,
            sol_amount_added: sol_consumed,
            lp_token_price: mine_btc_mining.lp_token_price_in_sol,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    // Return remaining SOL
    let sol_remaining = {
        let data = ctx.accounts.sol_token_account.try_borrow_data()?;
        anchor_spl::token::TokenAccount::try_deserialize(&mut &data[..])?.amount
    };

    if sol_remaining > 0 {
        anchor_spl::token::close_account(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::CloseAccount {
                account: ctx.accounts.sol_token_account.to_account_info(),
                destination: ctx.accounts.buybacks_sol_vault.to_account_info(),
                authority: ctx.accounts.authority_pda.to_account_info(),
            },
            signer_seeds,
        ))?;
    }

    // Update state
    buybacks_account.sol_for_pol = buybacks_account.sol_for_pol.saturating_sub(sol_consumed);
    mine_btc_mining.lp_operation_pending = false;

    msg!("✅ LP addition and burn complete");
    Ok(())
}

// /// Helper function to perform MINE_BTC to SOL swap via Raydium CPI
// fn perform_minebtc_to_sol_swap<'info>(
//     raydium_program: &AccountInfo<'info>,
//     pool_state: &AccountInfo<'info>,
//     amm_config: &AccountInfo<'info>,
//     authority_pda: &AccountInfo<'info>,
//     raydium_authority: &AccountInfo<'info>,
//     minebtc_vault: &AccountInfo<'info>,
//     sol_vault: &AccountInfo<'info>,
//     minebtc_token_account: &AccountInfo<'info>,
//     sol_token_account: &AccountInfo<'info>,
//     minebtc_mint: &AccountInfo<'info>,
//     sol_mint: &AccountInfo<'info>,
//     observation_state: &AccountInfo<'info>,
//     token_program_2022: &AccountInfo<'info>,
//     token_program: &AccountInfo<'info>,
//     amount_in: u64,
//     vault_auth_bump: u8,
// ) -> Result<u64> {
//     use raydium_cp_swap::cpi;

//     msg!("🔄 Performing real Raydium swap: {} MINE_BTC for WSOL", amount_in);

//     // Get WSOL token balance before swap by deserializing account data
//     let sol_balance_before = {
//         let sol_account_data = sol_token_account.try_borrow_data()?;
//         let sol_token_data = anchor_spl::token::TokenAccount::try_deserialize(&mut &sol_account_data[..])?;
//         sol_token_data.amount
//     }; // Borrow is dropped here

//     // Create signer seeds for vault authority
//     let authority_seeds = &[
//         MINE_BTC_VAULT_AUTHORITY_SEED.as_ref(),
//         &[vault_auth_bump],
//     ];
//     let signer_seeds = &[&authority_seeds[..]];

//     // Create CPI context for Raydium swap
//     let cpi_accounts = cpi::accounts::Swap {
//         payer: authority_pda.to_account_info(),         // Our PDA as the payer/signer
//         authority: raydium_authority.to_account_info(), // Raydium's pool authority PDA
//         amm_config: amm_config.to_account_info(),
//         pool_state: pool_state.to_account_info(),
//         input_token_account: minebtc_token_account.to_account_info(),  // Our token account (authority = our PDA)
//         output_token_account: sol_token_account.to_account_info(),   // Our token account (authority = our PDA)
//         input_vault: minebtc_vault.to_account_info(),     // Raydium's MINE_BTC vault
//         output_vault: sol_vault.to_account_info(),      // Raydium's SOL vault
//         input_token_program: token_program_2022.to_account_info(),   // Token-2022 for MINE_BTC
//         output_token_program: token_program.to_account_info(),       // Standard token for SOL
//         input_token_mint: minebtc_mint.to_account_info(),
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

/// Helper function to perform SOL to MINE_BTC swap via Raydium CPI
fn perform_sol_to_minebtc_swap<'info>(
    raydium_program: &AccountInfo<'info>,
    pool_state: &AccountInfo<'info>,
    amm_config: &AccountInfo<'info>,
    authority_pda: &AccountInfo<'info>,
    raydium_authority: &AccountInfo<'info>,
    minebtc_vault: &AccountInfo<'info>,
    sol_vault: &AccountInfo<'info>,
    minebtc_token_account: &AccountInfo<'info>,
    sol_token_account: &AccountInfo<'info>,
    minebtc_mint: &AccountInfo<'info>,
    sol_mint: &AccountInfo<'info>,
    observation_state: &AccountInfo<'info>,
    token_program_2022: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    sol_amount_in: u64,
    vault_auth_bump: u8,
) -> Result<u64> {
    use raydium_cp_swap::cpi;

    msg!("🔄 === STARTING SOL → MINE_BTC SWAP ===");
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

    // Get MINE_BTC token balance before swap by deserializing account data
    msg!("   📊 Reading MINE_BTC token account balance...");
    let minebtc_balance_before = {
        use anchor_spl::token_interface::TokenAccount as TokenAccountInterface;
        let minebtc_account_data = minebtc_token_account.try_borrow_data()?;
        let minebtc_token_data =
            TokenAccountInterface::try_deserialize(&mut &minebtc_account_data[..])?;
        msg!("   ✅ Successfully deserialized MINE_BTC token account");
        minebtc_token_data.amount
    }; // Borrow is dropped here

    msg!(
        "   💰 MINE_BTC balance BEFORE swap: {} ({} MINE_BTC)",
        minebtc_balance_before,
        minebtc_balance_before as f64 / 1e6
    );

    // Create signer seeds for vault authority
    msg!("   🔑 Creating signer seeds for vault authority PDA...");
    let authority_seeds = &[MINE_BTC_VAULT_AUTHORITY_SEED.as_ref(), &[vault_auth_bump]];
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
        output_token_account: minebtc_token_account.to_account_info(), // Our MINE_BTC token account (authority = our PDA)
        input_vault: sol_vault.to_account_info(),                      // Raydium's SOL vault
        output_vault: minebtc_vault.to_account_info(),                 // Raydium's MINE_BTC vault
        input_token_program: token_program.to_account_info(), // Standard token program for SOL
        output_token_program: token_program_2022.to_account_info(), // Token-2022 program for MINE_BTC
        input_token_mint: sol_mint.to_account_info(),
        output_token_mint: minebtc_mint.to_account_info(),
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
        "      Min output: {} MINE_BTC (accepting any amount)",
        min_amount_out
    );
    cpi::swap_base_input(cpi_ctx, sol_amount_in, min_amount_out)?;
    msg!("   ✅ Raydium swap CPI completed successfully");

    // Calculate actual MINE_BTC received by checking token account balance again
    msg!("   📊 Reading MINE_BTC token account balance after swap...");
    let minebtc_received = {
        use anchor_spl::token_interface::TokenAccount as TokenAccountInterface;
        let minebtc_account_data_after = minebtc_token_account.try_borrow_data()?;
        let minebtc_token_data_after =
            TokenAccountInterface::try_deserialize(&mut &minebtc_account_data_after[..])?;
        let minebtc_balance_after = minebtc_token_data_after.amount;
        msg!(
            "   💰 MINE_BTC balance AFTER swap: {} ({} MINE_BTC)",
            minebtc_balance_after,
            minebtc_balance_after as f64 / 1e6
        );
        let received = minebtc_balance_after.saturating_sub(minebtc_balance_before);
        msg!(
            "   📈 MINE_BTC received: {} ({} MINE_BTC)",
            received,
            received as f64 / 1e6
        );
        received
    }; // Borrow is dropped here

    msg!("✅ === SWAP COMPLETED SUCCESSFULLY ===");
    msg!(
        "   Swapped: {} SOL → {} MINE_BTC",
        sol_amount_in as f64 / 1e9,
        minebtc_received as f64 / 1e6
    );
    msg!(
        "   Exchange rate: {} MINE_BTC per SOL",
        (minebtc_received as f64 / 1e6) / (sol_amount_in as f64 / 1e9)
    );

    Ok(minebtc_received)
}

// DELETED: perform_lp_addition_and_burn function - now inlined in update_rate_and_add_lp_internal
// to avoid stack overflow. The logic is directly embedded in the calling function to reduce
// stack depth and memory pressure.

// ----------------------------------------------------------------------------------------
// ------------ DYNAMIC DISTRIBUTION FUNCTIONS :: ORACLE & RATE UPDATES -----------------
// ----------------------------------------------------------------------------------------

/// Adjust SOL amount to respect 5% MINEBTC vault limit
/// When max_minebtc_with_buffer exceeds 5% of available_minebtc, calculates the SOL amount
/// needed to match exactly 5% of available MINEBTC, maintaining pool ratio
/// Accounts for 1% MINEBTC burn tax and adds slippage tolerance
/// Returns (adjusted_sol_amount, adjusted_minebtc_amount, adjusted_max_minebtc_with_buffer)
fn adjust_sol_for_minebtc_limit(
    available_minebtc: u64,
    sol_vault_balance: u64,
    minebtc_vault_balance: u64,
    lp_supply: u64,
    original_sol_amount: u64,
    original_minebtc_amount: u64,
    minebtc_mint: &AccountInfo<'_>,
    epoch: u64,
) -> Result<(u64, u64, u64)> {
    // Calculate maximum MINEBTC we can use (1% of available_minebtc)
    let max_minebtc_allowed = available_minebtc / 100; // 1% of available_minebtc
    msg!(
        "   📊 Max MINEBTC allowed (1% of available_minebtc): {} MINEBTC",
        max_minebtc_allowed as f64 / 1e6
    );

    // Leave room for the 2% buffer on the pre-fee amount.
    let max_base_minebtc = (max_minebtc_allowed as u128 * 50 / 51) as u64;
    msg!(
        "   📊 Max base MINEBTC before transfer fee: {} MINEBTC",
        max_base_minebtc as f64 / 1e6
    );

    // Calculate MINEBTC that will actually reach the pool using the live Token-2022 fee config.
    let minebtc_received_in_pool =
        helper::get_token2022_transfer_fee_info(minebtc_mint, max_base_minebtc, epoch)?
            .post_fee_amount;
    msg!(
        "   📊 MINEBTC that will reach pool (after transfer fee): {} MINEBTC",
        minebtc_received_in_pool as f64 / 1e6
    );

    // Calculate corresponding SOL amount based on pool ratio
    // Use the MINEBTC that actually reaches the pool (after burn) for ratio calculation
    // If pool exists: sol_amount = (minebtc_received_in_pool * sol_vault_balance) / minebtc_vault_balance
    // If pool is empty, use original ratio
    let sol_from_ratio = if lp_supply > 0 && minebtc_vault_balance > 0 && sol_vault_balance > 0 {
        // Use pool ratio based on MINEBTC that reaches pool
        (minebtc_received_in_pool as u128 * sol_vault_balance as u128
            / minebtc_vault_balance as u128) as u64
    } else {
        // Pool is empty or invalid, use original ratio
        if original_minebtc_amount > 0 {
            let original_minebtc_received = helper::get_token2022_transfer_fee_info(
                minebtc_mint,
                original_minebtc_amount,
                epoch,
            )?
            .post_fee_amount;
            (minebtc_received_in_pool as u128 * original_sol_amount as u128
                / original_minebtc_received as u128) as u64
        } else {
            original_sol_amount // Fallback to original if no ratio available
        }
    };

    // Add slippage tolerance (reduce SOL by 3% to account for price movement and slippage)
    // This ensures we don't exceed slippage limits in Raydium
    let adjusted_sol_amount = sol_from_ratio.saturating_sub(sol_from_ratio * 3 / 100);
    msg!(
        "   📊 SOL from ratio (based on MINEBTC after burn): {} SOL",
        sol_from_ratio as f64 / 1e9
    );
    msg!(
        "   📊 SOL with slippage tolerance (3%): {} SOL",
        adjusted_sol_amount as f64 / 1e9
    );

    // Recalculate buffer with adjusted amounts
    let adjusted_minebtc_amount = max_base_minebtc;
    // Account for burn tax in the buffer calculation
    // After burn: adjusted_minebtc_amount * 0.99 will reach the pool
    // So max_minebtc_with_buffer should account for this
    let adjusted_max_minebtc_with_buffer =
        adjusted_minebtc_amount.saturating_add(adjusted_minebtc_amount / 50);

    msg!("   ✅ Adjusted amounts:");
    msg!(
        "      SOL: {} SOL (was {} SOL)",
        adjusted_sol_amount as f64 / 1e9,
        original_sol_amount as f64 / 1e9
    );
    msg!(
        "      MINEBTC: {} MINEBTC (was {} MINEBTC)",
        adjusted_minebtc_amount as f64 / 1e6,
        original_minebtc_amount as f64 / 1e6
    );
    msg!(
        "      Max MINEBTC with buffer: {} MINEBTC (limit: {} MINEBTC)",
        adjusted_max_minebtc_with_buffer as f64 / 1e6,
        max_minebtc_allowed as f64 / 1e6
    );

    Ok((
        adjusted_sol_amount,
        adjusted_minebtc_amount,
        adjusted_max_minebtc_with_buffer,
    ))
}

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
        bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,

    /// Treasury's WSOL token account (authority is treasury PDA)
    /// Initialized automatically if it doesn't exist (payer pays for initialization)
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = wsol_mint,
        associated_token::authority = sol_treasury,
    )]
    pub treasury_wsol_account: Account<'info, TokenAccount>,

    /// Multisig WSOL token account (destination for WSOL transfers)
    /// MUST be owned by global_config.fee_recipient (the multisig address)
    #[account(
        mut,
        constraint = multisig_wsol_account.mint == wsol_mint.key() @ ErrorCode::InvalidMint,
        constraint = multisig_wsol_account.owner == global_config.fee_recipient @ ErrorCode::Unauthorized
    )]
    pub multisig_wsol_account: Account<'info, TokenAccount>,

    /// CHECK: WSOL mint
    pub wsol_mint: UncheckedAccount<'info>,

    /// CHECK: Buybacks SOL vault PDA (System Account)
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

    /// Payer for account initialization (can be anyone calling this keeper function)
    #[account(mut)]
    pub payer: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

/// Account struct for taking price snapshots (Instruction 1)
/// Lighter weight - only needs swap-related accounts
#[derive(Accounts)]
pub struct SnapshotPrice<'info> {
    #[account(
        mut,
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump = mine_btc_mining.bump,
    )]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

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
        seeds = [MINE_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump,
    )]
    pub authority_pda: UncheckedAccount<'info>,

    /// CHECK: Raydium's pool authority PDA (from Raydium program)
    pub raydium_authority: UncheckedAccount<'info>,

    /// CHECK: MINE_BTC vault in Raydium pool
    #[account(mut)]
    pub minebtc_vault: UncheckedAccount<'info>,

    /// CHECK: SOL vault in Raydium pool
    #[account(mut)]
    pub sol_vault: UncheckedAccount<'info>,

    /// MINE_BTC token vault (main vault - same as used in initialize_mining)
    #[account(
        mut,
        seeds = [MINE_BTC_VAULT_SEED.as_ref(), mine_btc_mining.key().as_ref()],
        bump,
        constraint = minebtc_token_account.mint == minebtc_mint.key() @ ErrorCode::InvalidMint,
        constraint = minebtc_token_account.owner == authority_pda.key() @ ErrorCode::Unauthorized,
    )]
    pub minebtc_token_account: InterfaceAccount<'info, TokenAccount2022>,

    // === The WSOL ATA, created automatically if missing
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = sol_mint,
        associated_token::authority = authority_pda,
        associated_token::token_program = token_program
    )]
    pub sol_token_account: Account<'info, TokenAccount>, // <- concrete TokenAccount type

    pub associated_token_program: Program<'info, AssociatedToken>,

    /// CHECK: MINE_BTC mint
    #[account(mut)]
    pub minebtc_mint: UncheckedAccount<'info>,

    /// CHECK: SOL mint (WSOL)
    #[account(mut)]
    pub sol_mint: UncheckedAccount<'info>,

    /// CHECK: Raydium observation state
    #[account(mut)]
    pub observation_state: UncheckedAccount<'info>,

    /// Token-2022 program for MINE_BTC
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

/// Account struct for updating rate (Instruction 2a) - Lightweight
#[derive(Accounts)]
pub struct UpdateRate<'info> {
    #[account(
        mut,
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump = mine_btc_mining.bump,
    )]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

    #[account(mut, seeds = [FACTION_WAR_CONFIG_SEED], bump = faction_war_config.bump)]
    pub faction_war_config: Account<'info, FactionWarConfig>,
}

/// Account struct for LP addition and burn (Instruction 2b) - Heavier weight
#[derive(Accounts)]
pub struct AddLpAndBurn<'info> {
    #[account(mut, seeds = [MINE_BTC_MINING_SEED.as_ref()], bump = mine_btc_mining.bump)]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

    #[account(seeds = [GLOBAL_CONFIG_SEED.as_ref()], bump = global_config.bump)]
    pub global_config: Account<'info, GlobalConfig>,

    /// Authority (optional - only required when lp_token_amount > 0)
    pub authority: Option<Signer<'info>>,

    /// CHECK: Raydium CP-Swap program
    pub raydium_program: UncheckedAccount<'info>,

    /// CHECK: Raydium pool state
    #[account(mut)]
    pub pool_state: UncheckedAccount<'info>,

    /// CHECK: Vault authority PDA
    #[account(seeds = [MINE_BTC_VAULT_AUTHORITY_SEED.as_ref()], bump)]
    pub authority_pda: UncheckedAccount<'info>,

    /// CHECK: Raydium's pool authority PDA
    pub raydium_authority: UncheckedAccount<'info>,

    /// CHECK: MINE_BTC vault in Raydium pool
    #[account(mut)]
    pub minebtc_vault: UncheckedAccount<'info>,

    /// CHECK: SOL vault in Raydium pool
    #[account(mut)]
    pub sol_vault: UncheckedAccount<'info>,

    /// MINE_BTC token vault
    #[account(
        mut,
        seeds = [MINE_BTC_VAULT_SEED.as_ref(), mine_btc_mining.key().as_ref()],
        bump,
        constraint = minebtc_token_account.mint == minebtc_mint.key() @ ErrorCode::InvalidMint,
        constraint = minebtc_token_account.owner == authority_pda.key() @ ErrorCode::Unauthorized,
    )]
    pub minebtc_token_account: InterfaceAccount<'info, TokenAccount2022>,

    /// CHECK: SOL token account for LP addition
    #[account(mut)]
    pub sol_token_account: UncheckedAccount<'info>,

    /// CHECK: MINE_BTC mint
    #[account(mut)]
    pub minebtc_mint: UncheckedAccount<'info>,

    /// CHECK: SOL mint (WSOL)
    #[account(mut)]
    pub sol_mint: UncheckedAccount<'info>,

    /// CHECK: LP token account
    #[account(mut)]
    pub lp_token_account: UncheckedAccount<'info>,

    /// CHECK: LP mint from Raydium pool
    #[account(mut)]
    pub lp_mint: UncheckedAccount<'info>,

    pub token_program_2022: Program<'info, Token2022>,
    pub token_program: Program<'info, anchor_spl::token::Token>,

    /// CHECK: Buybacks SOL vault PDA
    #[account(mut, seeds = [BUYBACKS_SOL_VAULT_SEED.as_ref()], bump)]
    pub buybacks_sol_vault: UncheckedAccount<'info>,

    #[account(mut, seeds = [BUYBACKS_SEED.as_ref()], bump)]
    pub buybacks_account: Account<'info, BuybacksAccount>,

    pub system_program: Program<'info, System>,
}
