use crate::errors::ErrorCode;
use anchor_spl::token;

// # Economy Instructions
//
// The economy loop is deliberately separate from staking claims:
// - `distribute_sol_fees_internal` moves accumulated SOL from the treasury into buybacks
//   and the protocol/dev lane.
// - `snapshot_price_internal` records the on-chain market price and earnmarks some SOL for POL.
// - `update_rate_internal` converts the snapshot window into a new `dbtc_per_round` emission rate.
// - `add_lp_and_burn` (below in this file) consumes the earnmarked SOL together with degenBTC from the
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

/// Canonical wrapped-SOL mint pubkey. Used as an `address = …` constraint
/// anywhere this file accepts a WSOL mint as an `UncheckedAccount`, so a
/// caller can't substitute an attacker-controlled mint that happens to slot
/// into the same ATA-derivation / CPI shape.
pub const WSOL_MINT_PUBKEY: Pubkey =
    anchor_lang::solana_program::pubkey!("So11111111111111111111111111111111111111112");

fn u64_mul_div(a: u64, b: u64, c: u64) -> Result<u64> {
    u64::try_from(helper::mul_div(a, b, c)?).map_err(|_| ErrorCode::ArithmeticOverflow.into())
}

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

    // Calculate splits using configurable percentages.
    let buyback_percentage = global_config.sol_fee_config.buyback_pct as u64;
    let nft_mm_percentage = global_config.sol_fee_config.nft_market_making_pct as u64;
    require!(
        buyback_percentage + nft_mm_percentage <= M_HUNDRED,
        ErrorCode::InvalidParameters
    );
    let sol_for_buybacks = u64_mul_div(available_solana, buyback_percentage, M_HUNDRED)?;
    let sol_for_nft_mm = u64_mul_div(available_solana, nft_mm_percentage, M_HUNDRED)?;

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
        buybacks_ac.total_sol_accumulated = buybacks_ac
            .total_sol_accumulated
            .checked_add(sol_for_buybacks)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        msg!(
            "💰 Transferred {} SOL to buybacks vault ({}%)",
            sol_for_buybacks as f64 / 1e9,
            buyback_percentage
        );
    }

    // Transfer NFT market-making amount to inventory_sweep_vault.
    if sol_for_nft_mm > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.sol_treasury.to_account_info(),
                    to: ctx.accounts.inventory_sweep_vault.to_account_info(),
                },
                signer_seeds,
            ),
            sol_for_nft_mm,
        )?;
        emit!(NftMarketMakingFunded {
            sol_amount: sol_for_nft_mm,
            timestamp: Clock::get()?.unix_timestamp,
        });
        msg!(
            "🎨 Transferred {} SOL to inventory_sweep_vault for NFT market making ({}%)",
            sol_for_nft_mm as f64 / 1e9,
            nft_mm_percentage
        );
    }

    let dev_earnings = available_solana
        .checked_sub(sol_for_buybacks)
        .and_then(|r| r.checked_sub(sol_for_nft_mm))
        .ok_or(ErrorCode::ArithmeticOverflow)?;
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

    let dbtc_mining: &mut Account<'_, DegenBtcMining> = &mut ctx.accounts.dbtc_mining;
    let current_time = Clock::get()?.unix_timestamp;

    // Check if LP operation is pending
    require!(
        !dbtc_mining.lp_operation_pending,
        ErrorCode::InvalidAccount
    );

    msg!("   📅 Current timestamp: {}", current_time);
    msg!(
        "   ⏰ Last update timestamp: {}",
        dbtc_mining.last_rate_update
    );

    require!(
        dbtc_mining.price_history.len() < 8,
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
    if current_time < dbtc_mining.last_rate_update + snapshot_interval {
        msg!(
            "   ⏰ Update too early - must wait at least {} seconds between updates",
            snapshot_interval
        );
        msg!(
            "      Next update allowed: {}",
            dbtc_mining.last_rate_update + snapshot_interval
        );
        msg!(
            "      Time remaining: {} seconds",
            (dbtc_mining.last_rate_update + snapshot_interval - current_time)
        );
        return Ok(());
    }

    msg!(
        "   ✅ Time constraint satisfied ({}s since last update, required: {}s)",
        current_time - dbtc_mining.last_rate_update,
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
        dbtc_mining.price_history.len() + 1
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
            &ctx.accounts.degenbtc_mint,
            &ctx.accounts.sol_mint,
            &ctx.accounts.observation_state,
            &ctx.accounts.token_program_2022,
            &ctx.accounts.token_program,
            sol_for_swap,
            dbtc_mining.vault_auth_bump,
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
        dbtc_received,
        dbtc_received as f64 / 1e6
    );

    // sol_for_swap is in WSOL base units (9 decimals), dbtc_received is in MINE_BTC base units (6 decimals)
    //
    // Formula: Price = (sol_for_swap / 10^9) / (dbtc_received / 10^6)
    // Simplified: Price = (sol_for_swap * 10^6) / (dbtc_received * 10^9)
    // Store as lamports per whole MINE_BTC token:
    // Final: Price = (sol_for_swap * 10^6) / dbtc_received
    //
    // Hard-reject zero-output swaps. Raydium's `swap_base_input` accepts
    // `min_amount_out = 0`, so an attacker who can move the pool around the
    // CPI (e.g. sandwich) could drive `dbtc_received` to 0, push a 0-price
    // entry into `price_history`, and corrupt the weighted-average → emission
    // rate. The whole oracle pipeline is meaningless without a non-zero
    // price, so we revert the snapshot instead of recording a sentinel.
    require!(dbtc_received > 0, ErrorCode::InvalidAmount);
    let raw_price = (sol_for_swap as u128)
        .checked_mul(DBTC_BASE_UNITS as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(dbtc_received as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let current_price = u64::try_from(raw_price).map_err(|_| ErrorCode::ArithmeticOverflow)?;
    require!(current_price > 0, ErrorCode::InvalidAmount);

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
    dbtc_mining.price_history.push(price_entry);
    msg!(
        "   📈 Added price entry to history. Total entries: {}/8",
        dbtc_mining.price_history.len()
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

    for (i, entry) in dbtc_mining.price_history.iter().enumerate() {
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
        u64::try_from(weighted_sum / total_weights).map_err(|_| ErrorCode::ArithmeticOverflow)?
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
    dbtc_mining.recent_price = current_weighted_avg;

    // Update timestamp for next snapshot
    dbtc_mining.last_rate_update = current_time;

    msg!("\n✅ === PRICE SNAPSHOT COMPLETE ===");
    msg!(
        "   📊 Snapshot {}/8 recorded",
        dbtc_mining.price_history.len()
    );
    msg!("   💰 MINE_BTC received from swap: {}", dbtc_received);
    msg!(
        "   💎 SOL earnmarked for POL: {} SOL",
        buybacks_account.sol_for_pol as f64 / 1e9
    );
    msg!("   ⏱️  Next snapshot available in: ~30 minutes");

    // Emit price snapshot event for off-chain indexing
    emit!(PriceSnapshotTaken {
        snapshot_number: dbtc_mining.price_history.len() as u8,
        sol_swapped: sol_for_swap,
        dbtc_received,
        current_price,
        weighted_avg_price: current_weighted_avg,
        sol_earnmarked_for_pol: sol_for_pol_earnmark,
        total_pol_balance: buybacks_account.sol_for_pol,
        price_history_count: dbtc_mining.price_history.len() as u8,
        timestamp: current_time,
    });

    Ok(())
}

/// INSTRUCTION 2a: Update distribution rate (can be called by anyone after 4 hours)
/// Checks if conditions are met, updates distribution rate, sets flag for LP operation
pub fn update_rate_internal(ctx: Context<UpdateRate>) -> Result<()> {
    crate::log_fn!("economy", "update_rate_internal");
    msg!("🌟 === STARTING RATE UPDATE ===");

    let dbtc_mining = &mut ctx.accounts.dbtc_mining;
    let current_time = Clock::get()?.unix_timestamp;

    msg!("   📅 Current timestamp: {}", current_time);
    msg!(
        "   ⚙️  Current distribution rate: {} MINE_BTC per round",
        dbtc_mining.dbtc_per_round
    );

    // Check if 4 hours have passed AND we have 8 price entries
    let time_since_last = dbtc_mining
        .price_history
        .first()
        .map(|e| current_time - e.timestamp)
        .unwrap_or(0);

    if dbtc_mining.price_history.len() < 8 {
        msg!(
            "   ❌ Conditions NOT met: {} snapshots, {}s elapsed",
            dbtc_mining.price_history.len(),
            time_since_last
        );
        return Ok(());
    }

    msg!(
        "   ✅ 4-hour cycle complete with {} snapshots",
        dbtc_mining.price_history.len()
    );

    // Calculate weighted average price
    let mut weighted_sum: u128 = 0;
    let mut total_weights: u128 = 0;
    for (i, entry) in dbtc_mining.price_history.iter().enumerate() {
        let weight = (i + 1) as u128;
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
    let new_avg_price =
        u64::try_from(weighted_sum / total_weights).map_err(|_| ErrorCode::ArithmeticOverflow)?;

    // Calculate price change
    let change_from_track = calculate_price_change_pct(dbtc_mining.track_price, new_avg_price);
    let recent_comparison_price = dbtc_mining
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
    let old_rate = dbtc_mining.dbtc_per_round;
    let mut rate_changed = false;
    let price_change_threshold = dbtc_mining.price_change_threshold as i64;

    if price_change_pct.abs() >= price_change_threshold {
        if direction > 0 {
            let increase_multiplier = 100u64
                .checked_add(dbtc_mining.emission_increase_pct)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            dbtc_mining.dbtc_per_round = dbtc_mining
                .dbtc_per_round
                .checked_mul(increase_multiplier)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(100)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!(
                "   📈 Price increased {}%! Rate increased by {}%",
                price_change_pct,
                dbtc_mining.emission_increase_pct
            );
        } else {
            let decrease_multiplier = 100u64
                .checked_sub(dbtc_mining.emission_decrease_pct)
                .ok_or(ErrorCode::InvalidParameters)?;
            dbtc_mining.dbtc_per_round = dbtc_mining
                .dbtc_per_round
                .checked_mul(decrease_multiplier)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(100)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!(
                "   📉 Price decreased {}%! Rate decreased by {}%",
                price_change_pct,
                dbtc_mining.emission_decrease_pct
            );
        }
        dbtc_mining.track_price = new_avg_price;
        rate_changed = true;
    }

    // --- Update dynamic faction-war mining multiplier ---
    let war_config = &mut ctx.accounts.war_config;
    if rate_changed {
        let old_multiplier = war_config.mining_multiplier_bps as u128;
        let new_multiplier = if direction > 0 {
            let increase = old_multiplier
                .checked_mul(war_config.multiplier_increase_bps as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                / 10_000;
            old_multiplier
                .checked_add(increase)
                .ok_or(ErrorCode::ArithmeticOverflow)?
        } else {
            let decrease = old_multiplier
                .checked_mul(war_config.multiplier_decrease_bps as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                / 10_000;
            old_multiplier
                .checked_sub(decrease)
                .unwrap_or(MIN_FACTION_WAR_MINING_MULTIPLIER_BPS as u128)
        };
        let min_bps = (war_config.multiplier_min_bps).clamp(
            MIN_FACTION_WAR_MINING_MULTIPLIER_BPS,
            MAX_FACTION_WAR_MINING_MULTIPLIER_BPS,
        ) as u128;
        let max_bps = (war_config.multiplier_max_bps).clamp(
            MIN_FACTION_WAR_MINING_MULTIPLIER_BPS,
            MAX_FACTION_WAR_MINING_MULTIPLIER_BPS,
        ) as u128;
        require!(min_bps <= max_bps, ErrorCode::InvalidParameters);
        war_config.mining_multiplier_bps =
            (new_multiplier.min(max_bps).max(min_bps)) as u16;
        msg!(
            "   🎯 FactionWar multiplier updated: {} bps -> {} bps (direction={})",
            old_multiplier,
            war_config.mining_multiplier_bps,
            if direction > 0 { "up" } else { "down" }
        );
        emit!(FactionWarMultiplierUpdated {
            old_multiplier_bps: old_multiplier as u16,
            new_multiplier_bps: war_config.mining_multiplier_bps,
            direction: if direction > 0 { 1 } else { -1 },
            timestamp: current_time,
        });
    }

    // Set LP operation pending flag and store SOL amount
    dbtc_mining.lp_operation_pending = true;
    msg!(
        "   🎯 LP operation pending: {}",
        dbtc_mining.lp_operation_pending
    );

    // Clear price history and update state
    dbtc_mining.price_history.clear();
    dbtc_mining.recent_price = new_avg_price;
    dbtc_mining.last_rate_update = current_time;

    msg!(
        "✅ Rate update complete: {} -> {} ({})",
        old_rate,
        dbtc_mining.dbtc_per_round,
        if rate_changed { "CHANGED" } else { "unchanged" }
    );

    emit!(DistributionRateUpdated {
        old_rate,
        new_rate: dbtc_mining.dbtc_per_round,
        price_change_pct: price_change_pct as i32,
        current_price: new_avg_price,
        avg_price_4h: new_avg_price,
        track_price: dbtc_mining.track_price,
        recent_price: dbtc_mining.recent_price,
        rate_changed,
        new_mining_multiplier: war_config.mining_multiplier_bps,
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

    let dbtc_mining = &mut ctx.accounts.dbtc_mining;
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
        dbtc_mining.lp_operation_pending,
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
        dbtc_mining.lp_operation_pending = false;
        msg!("   ⚠️ No SOL for LP, clearing flag");
        return Ok(());
    }

    // === Pool-state read happens BEFORE any SOL movement ===
    //
    // Order matters here for a P0 reason: an earlier version of this ix did
    // the buybacks-vault → `sol_token_account` transfer first, then read the
    // pool's `sol_vault` / `lp_supply`, then early-returned `Ok(())` if either
    // was zero. With the old unchecked `sol_token_account`, that ordering let
    // a caller drain `sol_for_pol` straight into an attacker-owned WSOL
    // account by also passing a zero-balance `sol_vault`. We've since
    // anchored `sol_token_account` to the canonical `authority_pda`-owned
    // WSOL ATA (see Accounts struct) so the drain is no longer possible
    // even on the early-return path — but we also reorder the work here so
    // the early-return fires *before* any SOL leaves the buybacks vault.
    // Belt and suspenders: either change alone closes the bug, both together
    // make accidental reintroduction harder.
    //
    // We also pin `sol_vault` / `dbtc_vault` / `lp_mint` to the addresses
    // recorded in the canonical pool state so a forged token account with
    // matching mint can't slip through subsequent reads.
    let available_dbtc = ctx.accounts.dbtc_token_account.amount;
    let authority_seeds = &[
        DEGEN_BTC_VAULT_AUTHORITY_SEED.as_ref(),
        &[dbtc_mining.vault_auth_bump],
    ];
    let signer_seeds = &[&authority_seeds[..]];

    let (lp_supply, pool_token_0_vault, pool_token_1_vault, pool_token_0_mint, pool_lp_mint) = {
        let pool_state = RaydiumAccountLoad::<RaydiumPoolState>::load_data_mut(
            ctx.accounts.pool_state.as_ref(),
        )?;
        (
            pool_state.lp_supply,
            pool_state.token_0_vault,
            pool_state.token_1_vault,
            pool_state.token_0_mint,
            pool_state.lp_mint,
        )
    };

    // Match `sol_vault` / `dbtc_vault` against the pool's recorded vaults.
    // Token-0 is whichever of (sol_mint, dbtc_mint) sorts lower — same
    // ordering as the deposit CPI below.
    let sol_is_token_0 = pool_token_0_mint == ctx.accounts.sol_mint.key();
    let (expected_sol_vault, expected_dbtc_vault) = if sol_is_token_0 {
        (pool_token_0_vault, pool_token_1_vault)
    } else {
        (pool_token_1_vault, pool_token_0_vault)
    };
    require!(
        ctx.accounts.sol_vault.key() == expected_sol_vault,
        ErrorCode::InvalidAccount
    );
    require!(
        ctx.accounts.dbtc_vault.key() == expected_dbtc_vault,
        ErrorCode::InvalidAccount
    );
    require!(
        ctx.accounts.lp_mint.key() == pool_lp_mint,
        ErrorCode::InvalidAccount
    );

    let sol_vault_balance = {
        let data = ctx.accounts.sol_vault.try_borrow_data()?;
        anchor_spl::token::TokenAccount::try_deserialize(&mut &data[..])?.amount
    };

    let dbtc_vault_balance = {
        let data = ctx.accounts.dbtc_vault.try_borrow_data()?;
        anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &data[..])?.amount
    };

    // Early-return for empty/uninit pool BEFORE moving any SOL out of the
    // buybacks vault. `lp_token_amount == 0` is the permissionless path; the
    // admin override path with `lp_token_amount > 0` falls through and lets
    // Raydium's deposit CPI surface the bad-pool error.
    if lp_token_amount == 0 && (sol_vault_balance == 0 || lp_supply == 0) {
        msg!("   ⚠️ Pool vault balance is zero, skipping LP operation");
        dbtc_mining.lp_operation_pending = false;
        return Ok(());
    }

    // Transfer SOL from buybacks vault to sol_token_account (the canonical
    // `authority_pda`-owned WSOL ATA enforced by the Accounts struct).
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

    let lp_balance_before = {
        let data = ctx.accounts.lp_token_account.try_borrow_data()?;
        anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &data[..])?.amount
    };

    msg!(
        "   📊 Pool state: SOL={} SOL, degenBTC={} degenBTC, LP supply={} LP",
        sol_vault_balance as f64 / 1e9,
        dbtc_vault_balance as f64 / 1e6,
        lp_supply as f64 / 1e9
    );

    // Calculate deposit amounts
    let sol_buffer = total_sol_for_lp / 50;
    let available_sol = total_sol_for_lp - sol_buffer;

    let (estimated_lp_amount, adjusted_sol_amount, adjusted_dbtc_amount) = if lp_token_amount > 0
    {
        let required_sol = if lp_supply > 0 && sol_vault_balance > 0 {
            u64_mul_div(lp_token_amount, sol_vault_balance, lp_supply)?
        } else {
            available_sol
        };
        let required_dbtc_post_fee = if lp_supply > 0 && dbtc_vault_balance > 0 {
            u64_mul_div(lp_token_amount, dbtc_vault_balance, lp_supply)?
        } else {
            0
        };
        let required_dbtc = gross_up_for_token2022_fee(
            &ctx.accounts.degenbtc_mint.to_account_info(),
            required_dbtc_post_fee,
            clock.epoch,
        )?;
        (
            lp_token_amount,
            required_sol.min(available_sol),
            required_dbtc,
        )
    } else {
        // The (sol_vault_balance == 0 || lp_supply == 0) early-return is
        // hoisted above — see the pool-state read block. Reaching this branch
        // means both are non-zero, so the divisions below are safe.
        let lp_from_sol = u64_mul_div(available_sol, lp_supply, sol_vault_balance)?;
        let required_dbtc_post_fee = u64_mul_div(lp_from_sol, dbtc_vault_balance, lp_supply)?;
        let required_dbtc = gross_up_for_token2022_fee(
            &ctx.accounts.degenbtc_mint.to_account_info(),
            required_dbtc_post_fee,
            clock.epoch,
        )?;
        (lp_from_sol, available_sol, required_dbtc)
    };

    let max_dbtc_with_buffer = adjusted_dbtc_amount
        .checked_add(adjusted_dbtc_amount / 50)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    require!(
        available_dbtc >= max_dbtc_with_buffer,
        ErrorCode::InsufficientTokensInVault
    );
    msg!(
        "   💰 Deposit amounts: SOL={} SOL, degenBTC={} degenBTC (max with buffer) for {} LP",
        adjusted_sol_amount as f64 / 1e9,
        max_dbtc_with_buffer as f64 / 1e6,
        estimated_lp_amount as f64 / 1e9
    );

    // If max_dbtc_with_buffer exceeds 5% limit, adjust SOL amount to match 1% degenBTC limit
    let (final_sol_amount, _final_dbtc_amount, final_max_dbtc_with_buffer) =
        if max_dbtc_with_buffer >= available_dbtc / 100 {
            // 1% of available_dbtc
            msg!("   💰 Max degenBTC with buffer exceeds 1% of available degenBTC, adjusting SOL amount to match 1% degenBTC limit");
            adjust_sol_for_dbtc_limit(
                available_dbtc,
                sol_vault_balance,
                dbtc_vault_balance,
                lp_supply,
                adjusted_sol_amount,
                adjusted_dbtc_amount,
                &ctx.accounts.degenbtc_mint.to_account_info(),
                clock.epoch,
            )?
        } else {
            (
                adjusted_sol_amount,
                adjusted_dbtc_amount,
                max_dbtc_with_buffer,
            )
        };

    let final_estimated_lp_amount = if lp_supply > 0 && sol_vault_balance > 0 {
        let lp_from_sol = u64_mul_div(final_sol_amount, lp_supply, sol_vault_balance)?;
        lp_from_sol.saturating_sub(lp_from_sol / 100)
    } else {
        0
    };

    msg!(
        "   💰 Final estimated LP amount: {} LP for {} SOL and {} degenBTC",
        final_estimated_lp_amount as f64 / 1e9,
        final_sol_amount as f64 / 1e9,
        final_max_dbtc_with_buffer as f64 / 1e6
    );

    // Dynamic sorting
    let mine_btc_key = ctx.accounts.degenbtc_mint.key();
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
            ctx.accounts.dbtc_vault.to_account_info(),
            ctx.accounts.sol_vault.to_account_info(),
            ctx.accounts.dbtc_token_account.to_account_info(),
            ctx.accounts.sol_token_account.to_account_info(),
            ctx.accounts.degenbtc_mint.to_account_info(),
            ctx.accounts.sol_mint.to_account_info(),
            final_max_dbtc_with_buffer,
            final_sol_amount,
        )
    } else {
        (
            ctx.accounts.sol_vault.to_account_info(),
            ctx.accounts.dbtc_vault.to_account_info(),
            ctx.accounts.sol_token_account.to_account_info(),
            ctx.accounts.dbtc_token_account.to_account_info(),
            ctx.accounts.sol_mint.to_account_info(),
            ctx.accounts.degenbtc_mint.to_account_info(),
            final_sol_amount,
            final_max_dbtc_with_buffer,
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
    let lp_tokens_minted = lp_balance_after
        .checked_sub(lp_balance_before)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("   💰 LP tokens minted: {}", lp_tokens_minted);

    // Re-deserialize directly via Account's underlying info — the typed
    // `Account<TokenAccount>` was loaded at ix entry and won't reflect the
    // post-CPI balance change without an explicit reload.
    let sol_balance_after = {
        let info = ctx.accounts.sol_token_account.to_account_info();
        let data = info.try_borrow_data()?;
        anchor_spl::token::TokenAccount::try_deserialize(&mut &data[..])?.amount
    };
    msg!("   💰 SOL balance after: {}", sol_balance_after);
    msg!("   💰 SOL balance before: {}", total_sol_for_lp);
    let sol_consumed = total_sol_for_lp
        .checked_sub(sol_balance_after)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("   💰 SOL consumed: {}", sol_consumed);

    // ✅ RIGHT: Reloads fresh data from the account info
    let available_dbtc_after = {
        let account_info = ctx.accounts.dbtc_token_account.to_account_info();
        let data = account_info.try_borrow_data()?;
        anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &data[..])?.amount
    };
    msg!("   💰 degenBTC balance after: {}", available_dbtc_after);
    msg!("   💰 degenBTC balance before: {}", available_dbtc);
    let dbtc_consumed = available_dbtc
        .checked_sub(available_dbtc_after)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("   💰 degenBTC consumed: {}", dbtc_consumed);

    msg!(
        "   ✅ LP minted: {}, SOL consumed: {}, degenBTC consumed: {}",
        lp_tokens_minted,
        sol_consumed,
        dbtc_consumed
    );

    // Burn LP tokens
    if lp_tokens_minted > 0 {
        let lp_token_price = {
            let dbtc_price = dbtc_mining.recent_price;
            let dbtc_value_in_sol = if dbtc_price > 0 {
                helper::mul_div(dbtc_consumed, dbtc_price, 1_000_000)?
            } else {
                0
            };
            let total_value_sol = sol_consumed
                .checked_add(
                    u64::try_from(dbtc_value_in_sol)
                        .map_err(|_| ErrorCode::ArithmeticOverflow)?,
                )
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            helper::mul_div(total_value_sol, 1_000_000_000, lp_tokens_minted)?
        };
        if lp_token_price > 0 {
            dbtc_mining.lp_token_price_in_sol =
                u64::try_from(lp_token_price).map_err(|_| ErrorCode::ArithmeticOverflow)?;
        }
        msg!(
            "   💰 LP token price: {} SOL per LP",
            lp_token_price as f64 / 1e9
        );

        emit!(LiquidityAdded {
            sol_amount: sol_consumed,
            dbtc_amount: dbtc_consumed,
            lp_tokens_minted,
            lp_token_price: u64::try_from(lp_token_price)
                .map_err(|_| ErrorCode::ArithmeticOverflow)?,
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
        dbtc_mining
            .pol_stats
            .update_after_lp_operation(lp_tokens_minted);

        // If this LP op pushed us past the cycle's settle threshold, snapshot
        // the current round_id. That round becomes the last round of this
        // cycle: no new rounds can begin (start_round will block on this),
        // and settle_war runs once that round's data has folded into
        // FactionWarState. Captured only once per cycle.
        let war_config = &mut ctx.accounts.war_config;
        let current_round_id = ctx.accounts.global_game_state.current_round_id;
        // Capture the cycle's final round_id when the LP-burn threshold is
        // crossed. Once non-zero, this blocks new rounds (start_round) and
        // gates settle_war (which requires the boundary round to be folded).
        // Guard: only capture when at least one round has been started. If
        // the threshold somehow crosses before any round (e.g. lp ops fire
        // very early), we wait — leaving cycle_end_round_id at 0 lets rounds
        // continue starting, and the next LP burn after a round has played
        // captures correctly.
        if war_config.cycle_end_round_id == 0
            && current_round_id > 0
            && dbtc_mining.pol_stats.lp_operations_count
                >= war_config.settle_at_lp_op_count
        {
            war_config.cycle_end_round_id = current_round_id;
            msg!(
                "🪖 [add_lp_and_burn] cycle settle threshold crossed (lp_ops={} >= {}); cycle_end_round_id={}",
                dbtc_mining.pol_stats.lp_operations_count,
                war_config.settle_at_lp_op_count,
                current_round_id
            );
            emit!(CycleEndRoundSnapshotted {
                war_id: war_config.current_war_id,
                cycle_end_round_id: current_round_id,
                lp_operations_count: dbtc_mining.pol_stats.lp_operations_count,
                timestamp: Clock::get()?.unix_timestamp,
            });
        }

        emit!(LpTokensBurned {
            lp_tokens_burned: lp_tokens_minted,
            total_lp_burnt: dbtc_mining.pol_stats.total_lp_burnt,
            dbtc_amount_added: dbtc_consumed,
            sol_amount_added: sol_consumed,
            lp_token_price: dbtc_mining.lp_token_price_in_sol,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    // Return remaining SOL — re-read fresh data, same reason as
    // `sol_balance_after` above.
    let sol_remaining = {
        let info = ctx.accounts.sol_token_account.to_account_info();
        let data = info.try_borrow_data()?;
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
    buybacks_account.sol_for_pol = buybacks_account
        .sol_for_pol
        .checked_sub(sol_consumed)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    dbtc_mining.lp_operation_pending = false;

    msg!("✅ LP addition and burn complete");
    Ok(())
}

// /// Helper function to perform MINE_BTC to SOL swap via Raydium CPI
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
//     degenbtc_mint: &AccountInfo<'info>,
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
//         DEGEN_BTC_VAULT_AUTHORITY_SEED.as_ref(),
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
//         input_vault: dbtc_vault.to_account_info(),     // Raydium's MINE_BTC vault
//         output_vault: sol_vault.to_account_info(),      // Raydium's SOL vault
//         input_token_program: token_program_2022.to_account_info(),   // Token-2022 for MINE_BTC
//         output_token_program: token_program.to_account_info(),       // Standard token for SOL
//         input_token_mint: degenbtc_mint.to_account_info(),
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
    degenbtc_mint: &AccountInfo<'info>,
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
    let dbtc_balance_before = {
        use anchor_spl::token_interface::TokenAccount as TokenAccountInterface;
        let dbtc_account_data = dbtc_token_account.try_borrow_data()?;
        let dbtc_token_data =
            TokenAccountInterface::try_deserialize(&mut &dbtc_account_data[..])?;
        msg!("   ✅ Successfully deserialized MINE_BTC token account");
        dbtc_token_data.amount
    }; // Borrow is dropped here

    msg!(
        "   💰 MINE_BTC balance BEFORE swap: {} ({} MINE_BTC)",
        dbtc_balance_before,
        dbtc_balance_before as f64 / 1e6
    );

    // Create signer seeds for vault authority
    msg!("   🔑 Creating signer seeds for vault authority PDA...");
    let authority_seeds = &[DEGEN_BTC_VAULT_AUTHORITY_SEED.as_ref(), &[vault_auth_bump]];
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
        output_token_account: dbtc_token_account.to_account_info(), // Our MINE_BTC token account (authority = our PDA)
        input_vault: sol_vault.to_account_info(),                      // Raydium's SOL vault
        output_vault: dbtc_vault.to_account_info(),                 // Raydium's MINE_BTC vault
        input_token_program: token_program.to_account_info(), // Standard token program for SOL
        output_token_program: token_program_2022.to_account_info(), // Token-2022 program for MINE_BTC
        input_token_mint: sol_mint.to_account_info(),
        output_token_mint: degenbtc_mint.to_account_info(),
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
    let dbtc_received = {
        use anchor_spl::token_interface::TokenAccount as TokenAccountInterface;
        let dbtc_account_data_after = dbtc_token_account.try_borrow_data()?;
        let dbtc_token_data_after =
            TokenAccountInterface::try_deserialize(&mut &dbtc_account_data_after[..])?;
        let dbtc_balance_after = dbtc_token_data_after.amount;
        msg!(
            "   💰 MINE_BTC balance AFTER swap: {} ({} MINE_BTC)",
            dbtc_balance_after,
            dbtc_balance_after as f64 / 1e6
        );
        let received = dbtc_balance_after.saturating_sub(dbtc_balance_before);
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
        dbtc_received as f64 / 1e6
    );
    msg!(
        "   Exchange rate: {} MINE_BTC per SOL",
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

/// Adjust SOL amount to respect 5% degenBTC vault limit
/// When max_dbtc_with_buffer exceeds 5% of available_dbtc, calculates the SOL amount
/// needed to match exactly 5% of available degenBTC, maintaining pool ratio
/// Accounts for 1% degenBTC burn tax and adds slippage tolerance
/// Returns (adjusted_sol_amount, adjusted_dbtc_amount, adjusted_max_dbtc_with_buffer)
fn adjust_sol_for_dbtc_limit(
    available_dbtc: u64,
    sol_vault_balance: u64,
    dbtc_vault_balance: u64,
    lp_supply: u64,
    original_sol_amount: u64,
    original_dbtc_amount: u64,
    degenbtc_mint: &AccountInfo<'_>,
    epoch: u64,
) -> Result<(u64, u64, u64)> {
    // Calculate maximum degenBTC we can use (1% of available_dbtc)
    let max_dbtc_allowed = available_dbtc / 100; // 1% of available_dbtc
    msg!(
        "   📊 Max degenBTC allowed (1% of available_dbtc): {} degenBTC",
        max_dbtc_allowed as f64 / 1e6
    );

    // Leave room for the 2% buffer on the pre-fee amount.
    let max_base_dbtc = u64_mul_div(max_dbtc_allowed, 50, 51)?;
    msg!(
        "   📊 Max base degenBTC before transfer fee: {} degenBTC",
        max_base_dbtc as f64 / 1e6
    );

    // Calculate degenBTC that will actually reach the pool using the live Token-2022 fee config.
    let dbtc_received_in_pool =
        helper::get_token2022_transfer_fee_info(degenbtc_mint, max_base_dbtc, epoch)?
            .post_fee_amount;
    msg!(
        "   📊 degenBTC that will reach pool (after transfer fee): {} degenBTC",
        dbtc_received_in_pool as f64 / 1e6
    );

    // Calculate corresponding SOL amount based on pool ratio
    // Use the degenBTC that actually reaches the pool (after burn) for ratio calculation
    // If pool exists: sol_amount = (dbtc_received_in_pool * sol_vault_balance) / dbtc_vault_balance
    // If pool is empty, use original ratio
    let sol_from_ratio = if lp_supply > 0 && dbtc_vault_balance > 0 && sol_vault_balance > 0 {
        // Use pool ratio based on degenBTC that reaches pool
        u64_mul_div(
            dbtc_received_in_pool,
            sol_vault_balance,
            dbtc_vault_balance,
        )?
    } else {
        // Pool is empty or invalid, use original ratio
        if original_dbtc_amount > 0 {
            let original_dbtc_received = helper::get_token2022_transfer_fee_info(
                degenbtc_mint,
                original_dbtc_amount,
                epoch,
            )?
            .post_fee_amount;
            require!(original_dbtc_received > 0, ErrorCode::InvalidAmount);
            u64_mul_div(
                dbtc_received_in_pool,
                original_sol_amount,
                original_dbtc_received,
            )?
        } else {
            original_sol_amount // Fallback to original if no ratio available
        }
    };

    // Add slippage tolerance (reduce SOL by 3% to account for price movement and slippage)
    // This ensures we don't exceed slippage limits in Raydium
    let slippage_buffer = u64_mul_div(sol_from_ratio, 3, M_HUNDRED)?;
    let adjusted_sol_amount = sol_from_ratio
        .checked_sub(slippage_buffer)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   📊 SOL from ratio (based on degenBTC after burn): {} SOL",
        sol_from_ratio as f64 / 1e9
    );
    msg!(
        "   📊 SOL with slippage tolerance (3%): {} SOL",
        adjusted_sol_amount as f64 / 1e9
    );

    // Recalculate buffer with adjusted amounts
    let adjusted_dbtc_amount = max_base_dbtc;
    // Account for burn tax in the buffer calculation
    // After burn: adjusted_dbtc_amount * 0.99 will reach the pool
    // So max_dbtc_with_buffer should account for this
    let adjusted_max_dbtc_with_buffer = adjusted_dbtc_amount
        .checked_add(adjusted_dbtc_amount / 50)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    msg!("   ✅ Adjusted amounts:");
    msg!(
        "      SOL: {} SOL (was {} SOL)",
        adjusted_sol_amount as f64 / 1e9,
        original_sol_amount as f64 / 1e9
    );
    msg!(
        "      degenBTC: {} degenBTC (was {} degenBTC)",
        adjusted_dbtc_amount as f64 / 1e6,
        original_dbtc_amount as f64 / 1e6
    );
    msg!(
        "      Max degenBTC with buffer: {} degenBTC (limit: {} degenBTC)",
        adjusted_max_dbtc_with_buffer as f64 / 1e6,
        max_dbtc_allowed as f64 / 1e6
    );

    Ok((
        adjusted_sol_amount,
        adjusted_dbtc_amount,
        adjusted_max_dbtc_with_buffer,
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

    /// CHECK: WSOL mint. Address-constrained to the canonical WSOL pubkey so a
    /// caller can't pass a fake mint, drive `treasury_wsol_account` ATA-init at
    /// a different mint and silently divert dev-earnings into an attacker ATA.
    /// (sync_native already would catch this on the SPL side, but constraining
    /// here fails earlier with a clearer error and removes the foot-gun.)
    #[account(address = WSOL_MINT_PUBKEY @ ErrorCode::InvalidMint)]
    pub wsol_mint: UncheckedAccount<'info>,

    /// CHECK: Buybacks SOL vault PDA (System Account)
    #[account(
        mut,
        seeds = [BUYBACKS_SOL_VAULT_SEED.as_ref()],
        bump
    )]
    pub buybacks_sol_vault: UncheckedAccount<'info>,

    /// CHECK: NFT market-making SOL vault PDA — receives the
    /// `nft_market_making_pct` slice of distributed SOL fees.
    #[account(
        mut,
        seeds = [INVENTORY_SWEEP_VAULT_SEED.as_ref()],
        bump,
    )]
    pub inventory_sweep_vault: UncheckedAccount<'info>,

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
        bump = dbtc_mining.bump,
    )]
    pub dbtc_mining: Account<'info, DegenBtcMining>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Raydium CP-Swap program. Address-constrained to
    /// `raydium_cp_swap::ID` so a malicious program can't receive our CPI
    /// (which signs with the `authority_pda` PDA) and drain program-owned
    /// token accounts.
    #[account(address = raydium_cp_swap::ID @ ErrorCode::InvalidAccount)]
    pub raydium_program: UncheckedAccount<'info>,

    /// CHECK: Raydium pool state
    #[account(mut)]
    pub pool_state: UncheckedAccount<'info>,

    /// CHECK: Raydium AMM config
    pub amm_config: UncheckedAccount<'info>,

    /// CHECK: Vault authority PDA (our program's authority for token accounts)
    #[account(
        seeds = [DEGEN_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump,
    )]
    pub authority_pda: UncheckedAccount<'info>,

    /// CHECK: Raydium's pool authority PDA (from Raydium program)
    pub raydium_authority: UncheckedAccount<'info>,

    /// CHECK: MINE_BTC vault in Raydium pool
    #[account(mut)]
    pub dbtc_vault: UncheckedAccount<'info>,

    /// CHECK: SOL vault in Raydium pool
    #[account(mut)]
    pub sol_vault: UncheckedAccount<'info>,

    /// MINE_BTC token vault (main vault - same as used in initialize_mining)
    #[account(
        mut,
        seeds = [DEGEN_BTC_VAULT_SEED.as_ref(), dbtc_mining.key().as_ref()],
        bump,
        constraint = dbtc_token_account.mint == degenbtc_mint.key() @ ErrorCode::InvalidMint,
        constraint = dbtc_token_account.owner == authority_pda.key() @ ErrorCode::Unauthorized,
    )]
    pub dbtc_token_account: InterfaceAccount<'info, TokenAccount2022>,

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
    pub degenbtc_mint: UncheckedAccount<'info>,

    /// CHECK: SOL mint (WSOL). Address-constrained to the canonical WSOL pubkey
    /// — defense-in-depth: Raydium's pool already validates mints against its
    /// vaults, but pinning the address here removes the chance of confusion if
    /// the gating `raydium_pool_state` ever changes.
    #[account(mut, address = WSOL_MINT_PUBKEY @ ErrorCode::InvalidMint)]
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
        bump = dbtc_mining.bump,
    )]
    pub dbtc_mining: Account<'info, DegenBtcMining>,

    #[account(mut, seeds = [FACTION_WAR_CONFIG_SEED], bump = war_config.bump)]
    pub war_config: Account<'info, FactionWarConfig>,
}

/// Account struct for LP addition and burn (Instruction 2b) - Heavier weight
#[derive(Accounts)]
pub struct AddLpAndBurn<'info> {
    #[account(mut, seeds = [MINE_BTC_MINING_SEED.as_ref()], bump = dbtc_mining.bump)]
    pub dbtc_mining: Account<'info, DegenBtcMining>,

    #[account(seeds = [GLOBAL_CONFIG_SEED.as_ref()], bump = global_config.bump)]
    pub global_config: Account<'info, GlobalConfig>,

    /// Read-only: provides `current_round_id` so we can snapshot the cycle's
    /// final round when the LP op crosses the war's settle threshold.
    #[account(seeds = [GLOBAL_GAME_STATE_SEED.as_ref()], bump = global_game_state.bump)]
    pub global_game_state: Account<'info, GlobalGameSate>,

    /// Mut: this ix writes `cycle_end_round_id` once the lp threshold crosses.
    #[account(mut, seeds = [FACTION_WAR_CONFIG_SEED.as_ref()], bump = war_config.bump)]
    pub war_config: Account<'info, FactionWarConfig>,

    /// Authority (optional - only required when lp_token_amount > 0)
    pub authority: Option<Signer<'info>>,

    /// CHECK: Raydium CP-Swap program. Address-constrained to
    /// `raydium_cp_swap::ID` so a malicious program can't receive our CPI
    /// (which signs with the `authority_pda` PDA) and drain program-owned
    /// token accounts via authority_pda's signer privilege.
    #[account(address = raydium_cp_swap::ID @ ErrorCode::InvalidAccount)]
    pub raydium_program: UncheckedAccount<'info>,

    /// CHECK: Raydium pool state
    #[account(mut)]
    pub pool_state: UncheckedAccount<'info>,

    /// CHECK: Vault authority PDA
    #[account(seeds = [DEGEN_BTC_VAULT_AUTHORITY_SEED.as_ref()], bump)]
    pub authority_pda: UncheckedAccount<'info>,

    /// CHECK: Raydium's pool authority PDA
    pub raydium_authority: UncheckedAccount<'info>,

    /// CHECK: MINE_BTC vault in Raydium pool
    #[account(mut)]
    pub dbtc_vault: UncheckedAccount<'info>,

    /// CHECK: SOL vault in Raydium pool
    #[account(mut)]
    pub sol_vault: UncheckedAccount<'info>,

    /// MINE_BTC token vault
    #[account(
        mut,
        seeds = [DEGEN_BTC_VAULT_SEED.as_ref(), dbtc_mining.key().as_ref()],
        bump,
        constraint = dbtc_token_account.mint == degenbtc_mint.key() @ ErrorCode::InvalidMint,
        constraint = dbtc_token_account.owner == authority_pda.key() @ ErrorCode::Unauthorized,
    )]
    pub dbtc_token_account: InterfaceAccount<'info, TokenAccount2022>,

    /// SOL token account for LP addition. **Must be the canonical WSOL ATA
    /// owned by `authority_pda`** — without this binding, a caller could pass
    /// an attacker-owned WSOL account and siphon the earnmarked
    /// `buybacks_account.sol_for_pol` through the early-return path (e.g. by
    /// also passing a zero-balance `sol_vault`). With the constraint, any
    /// SOL transferred here is held under `authority_pda`'s signer authority
    /// and is recovered to `buybacks_sol_vault` by the trailing close at the
    /// end of this ix. SnapshotPrice init-if-needs this exact ATA, so it will
    /// exist by the time the first LP burn fires.
    #[account(
        mut,
        token::mint = sol_mint,
        token::authority = authority_pda,
    )]
    pub sol_token_account: Account<'info, TokenAccount>,

    /// CHECK: MINE_BTC mint
    #[account(mut)]
    pub degenbtc_mint: UncheckedAccount<'info>,

    /// CHECK: SOL mint (WSOL). Address-constrained to the canonical WSOL pubkey
    /// — same rationale as `SnapshotPrice::sol_mint`.
    #[account(mut, address = WSOL_MINT_PUBKEY @ ErrorCode::InvalidMint)]
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
