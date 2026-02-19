use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022::extension::{
    transfer_fee::TransferFeeConfig, BaseStateWithExtensions, StateWithExtensions,
};
use anchor_spl::token_2022::{self, Burn, TransferChecked};
use anchor_spl::token_interface::{
    harvest_withheld_tokens_to_mint, withdraw_withheld_tokens_from_mint,
    HarvestWithheldTokensToMint, WithdrawWithheldTokensFromMint,
};
use anchor_spl::token_interface::{Mint, TokenAccount as TokenAccount2022};

// # Tax and Distribution Instructions
//
// This module implements the deflationary tax system for MineBTC using Token-2022 transfer fees.
//
// ## Tax Mechanics
//
// All MineBTC transfers incur a 1% tax, which is:
// - **Burned**: Reducing total supply and increasing scarcity.
// - **NFT Floor Sweep**: Allocated for NFT buyback and floor support.
// - **Faction Treasury**: Distributed to factions based on performance rankings.
//
// ## Distribution Rounds
//
// Every day, a distribution round calculates faction rankings based on total hashpower:
// 1. Factions are ranked by hashpower (highest to lowest).
// 2. Rewards are distributed using a tiered model (top factions earn more).
// 3. Each faction claims rewards, which are distributed to their stakers.
//
// ## Key Functions
//
// - `crank_harvest_fees`: Harvests transfer fees from user accounts to the mint.
// - `crank_distribute_tax`: Withdraws and distributes taxes to vaults.
// - `start_distribution_round`: Initiates a new distribution cycle (1-day cooldown).
// - `cal_faction_positions`: Ranks one faction by hashpower.
// - `cal_faction_rewards`: Computes rewards for all factions.
// - `claim_faction_treasury_rewards`: Claims rewards for a faction's stakers.
//

use crate::errors::ErrorCode;
use crate::events::*;
use crate::instructions::helper;
use crate::state::*;

// ========================================================================================
// ============================= ADMIN FUNCTIONS ==========================================
// ========================================================================================

/// Initialize TaxConfig account and create vault token accounts
/// Callable only by global config authority
pub fn internal_initialize_tax_config(
    ctx: Context<InitializeTaxConfig>,
    nft_floor_sweep_pct: u8,
    faction_treasury_pct: u8,
    burn_pct: u8,
    nft_floor_sweep_whitelisted_address: Pubkey,
) -> Result<()> {
    msg!("🔧 [initialize_tax_config] Initializing tax system");

    require!(
        (nft_floor_sweep_pct as u64) + (faction_treasury_pct as u64) + (burn_pct as u64)
            <= M_HUNDRED,
        ErrorCode::InvalidAmount
    );

    let tax_config = &mut ctx.accounts.tax_config;
    let clock = Clock::get()?;

    // Initialize TaxConfig
    tax_config.bump = ctx.bumps.tax_config;
    tax_config.nft_floor_sweep_pct = nft_floor_sweep_pct;
    tax_config.faction_treasury_pct = faction_treasury_pct;
    tax_config.burn_pct = burn_pct;
    tax_config.total_burnt = 0;
    tax_config.round_active = false;
    tax_config.start_timestamp = 0;
    tax_config.end_timestamp = clock.unix_timestamp;
    tax_config.leaderboard_factions_count = 0;
    tax_config.rewards_calculated = false;
    tax_config.factions_claimed_count = 0;

    // Initialize vectors
    tax_config.leaderboard_faction_ids = Vec::new();
    tax_config.leaderboard_hashpower = Vec::new();
    tax_config.faction_rewards = Vec::new();
    tax_config.faction_claimed = Vec::new();

    // Store PDA addresses
    tax_config.withdraw_withheld_authority = ctx.accounts.withdraw_withheld_authority.key();
    tax_config.faction_treasury_vault = ctx.accounts.faction_treasury_vault.key();
    tax_config.nft_floor_sweep_vault = ctx.accounts.nft_floor_sweep_vault.key();
    tax_config.nft_sale_sol_vault = ctx.accounts.nft_sale_sol_vault.key();
    tax_config.nft_floor_sweep_whitelisted_address = nft_floor_sweep_whitelisted_address;

    msg!("   ✅ TaxConfig initialized");
    msg!("   NFT Floor Sweep: {}%", nft_floor_sweep_pct);
    msg!("   Faction Treasury: {}%", faction_treasury_pct);
    msg!("   Burn: {}%", burn_pct);
    let vault_pct = M_HUNDRED as u8 - nft_floor_sweep_pct - faction_treasury_pct - burn_pct;
    msg!("   Back to Vault: {}%", vault_pct);
    msg!(
        "   Withdraw Authority: {}",
        tax_config.withdraw_withheld_authority
    );
    msg!(
        "   Faction Treasury Vault: {}",
        tax_config.faction_treasury_vault
    );
    msg!(
        "   NFT Floor Sweep Vault: {}",
        tax_config.nft_floor_sweep_vault
    );
    msg!(
        "   NFT Floor Sweep Whitelisted Address: {}",
        nft_floor_sweep_whitelisted_address
    );

    Ok(())
}

/// Update tax distribution percentages
/// Callable only by global config authority
pub fn internal_update_tax_config(
    ctx: Context<UpdateTaxConfig>,
    nft_floor_sweep_pct: u8,
    faction_treasury_pct: u8,
    burn_pct: u8,
) -> Result<()> {
    msg!("🔧 [update_tax_config] Updating tax distribution percentages");

    require!(
        (nft_floor_sweep_pct as u64) + (faction_treasury_pct as u64) + (burn_pct as u64)
            <= M_HUNDRED as u64,
        ErrorCode::InvalidAmount
    );

    let tax_config = &mut ctx.accounts.tax_config;
    tax_config.nft_floor_sweep_pct = nft_floor_sweep_pct;
    tax_config.faction_treasury_pct = faction_treasury_pct;
    tax_config.burn_pct = burn_pct;

    msg!("   ✅ TaxConfig updated");
    msg!(
        "   NFT Floor Sweep: {}%, Faction Treasury: {}%, Burn: {}%",
        nft_floor_sweep_pct,
        faction_treasury_pct,
        burn_pct
    );
    let vault_pct = M_HUNDRED as u8 - nft_floor_sweep_pct - faction_treasury_pct - burn_pct;
    msg!("   Back to Vault: {}%", vault_pct);

    Ok(())
}

/// Update NFT floor sweep whitelisted address
/// Callable only by global config authority
pub fn internal_update_nft_floor_sweep_whitelist(
    ctx: Context<UpdateNftFloorSweepWhitelist>,
    new_whitelisted_address: Pubkey,
) -> Result<()> {
    msg!("🔧 [update_nft_floor_sweep_whitelist] Updating whitelisted address");

    let tax_config = &mut ctx.accounts.tax_config;
    tax_config.nft_floor_sweep_whitelisted_address = new_whitelisted_address;

    msg!(
        "   ✅ Whitelisted address updated: {}",
        new_whitelisted_address
    );

    Ok(())
}

/// Withdraw MineBtc from NFT floor sweep vault
/// Callable only by the whitelisted address
/// The whitelisted address will use this MineBtc to swap for SOL off-chain,
/// buy NFTs, re-list them at 1.2x, and transfer SOL proceeds to SOL treasury
pub fn internal_withdraw_nft_floor_sweep_funds(
    ctx: Context<WithdrawNftFloorSweepFunds>,
    amount: u64,
) -> Result<()> {
    msg!(
        "💰 [withdraw_nft_floor_sweep_funds] Withdrawing {} MineBtc",
        amount
    );

    let tax_config = &ctx.accounts.tax_config;
    let whitelisted_address = &ctx.accounts.whitelisted_address;

    // Verify caller is the whitelisted address
    require!(
        tax_config.nft_floor_sweep_whitelisted_address == whitelisted_address.key(),
        ErrorCode::Unauthorized
    );

    // Verify vault has sufficient balance
    let vault_balance = ctx.accounts.nft_floor_sweep_vault.amount;
    require!(vault_balance >= amount, ErrorCode::InsufficientFunds);

    // Transfer MineBtc from vault to whitelisted address's token account
    let withdraw_authority_bump = ctx.bumps.withdraw_withheld_authority;
    let withdraw_authority_seeds = &[
        WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref(),
        &[withdraw_authority_bump],
    ];
    let withdraw_authority_signer = &[&withdraw_authority_seeds[..]];

    token_2022::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program_2022.to_account_info(),
            TransferChecked {
                from: ctx.accounts.nft_floor_sweep_vault.to_account_info(),
                mint: ctx.accounts.minebtc_mint.to_account_info(),
                to: ctx.accounts.whitelisted_token_account.to_account_info(),
                authority: ctx.accounts.withdraw_withheld_authority.to_account_info(),
            },
            withdraw_authority_signer,
        ),
        amount,
        ctx.accounts.minebtc_mint.decimals,
    )?;

    msg!(
        "   ✅ Transferred {} MineBtc to whitelisted address {}",
        amount,
        whitelisted_address.key()
    );

    emit!(NftFloorSweepFundsWithdrawn {
        whitelisted_address: whitelisted_address.key(),
        amount,
        nft_floor_sweep_vault: ctx.accounts.nft_floor_sweep_vault.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

// ========================================================================================
// ============================= TAX HARVESTING & DISTRIBUTION ===========================
// ========================================================================================

/// STEP 1: "Harvest" fees from user accounts to the mint
///
/// This is the "stupid crank." A keeper bot must find all token accounts
/// with withheld fees (off-chain) and pass them into this function
/// in batches using `ctx.remaining_accounts`.
///
/// This instruction "sucks" the fees from user accounts and deposits
/// them into the minebtc_mint's own "withheld_amount" field.
///
/// Callable by anyone - designed to be called many times in batches
pub fn internal_crank_harvest_fees<'info>(
    ctx: Context<'_, '_, '_, 'info, CrankHarvestFees<'info>>,
) -> Result<()> {
    msg!("🌱 [crank_harvest_fees] Harvesting withheld fees to mint...");

    // Get all the token accounts that were passed in by the keeper bot
    msg!(
        "   Harvesting from {} accounts...",
        ctx.remaining_accounts.len()
    );

    if ctx.remaining_accounts.is_empty() {
        msg!("   No source accounts provided. Exiting.");
        return Ok(());
    }

    // Get all the token accounts that were passed in by the keeper bot
    // This is already a slice: &[AccountInfo<'info>]
    let source_accounts = ctx.remaining_accounts;
    msg!("   Harvesting from {} accounts...", source_accounts.len());

    // Call `harvest_withheld_tokens_to_mint`
    // This CPI will pull the fees from all `remaining_accounts`
    // and aggregate them into `minebtc_mint.withheld_amount`.
    harvest_withheld_tokens_to_mint(
        CpiContext::new(
            ctx.accounts.token_program_2022.to_account_info(),
            HarvestWithheldTokensToMint {
                mint: ctx.accounts.minebtc_mint.to_account_info(),
                token_program_id: ctx.accounts.token_program_2022.to_account_info(),
            },
        ),
        source_accounts.to_vec(),
    )?;

    msg!("   ✅ Harvest complete. Fees moved to mint.");
    Ok(())
}

/// STEP 2: "Withdraw & Distribute" tax from the MINT
///
/// This function should be called *after* `crank_harvest_fees` has
/// been run. It withdraws the *total* accumulated tax from the
/// mint account and distributes it according to TaxConfig percentages.
///
/// Callable by anyone - program-controlled withdraw authority
pub fn internal_crank_distribute_tax(ctx: Context<CrankDistributeTax>) -> Result<()> {
    msg!("💰 [crank_distribute_tax] Withdrawing *total* tax from mint");

    // 1. Get the total amount of tax sitting on the mint account
    // We must reload to get the most up-to-date data after harvesting
    ctx.accounts.minebtc_mint.reload()?;

    // Read mint data and get TransferFeeConfig extension
    let mint_account_info = ctx.accounts.minebtc_mint.to_account_info();
    let mint_data = mint_account_info.try_borrow_data()?;
    let mint = StateWithExtensions::<anchor_spl::token_2022::spl_token_2022::state::Mint>::unpack(
        &mint_data,
    )?;
    let transfer_fee_config = <StateWithExtensions<
        anchor_spl::token_2022::spl_token_2022::state::Mint,
    > as BaseStateWithExtensions<
        anchor_spl::token_2022::spl_token_2022::state::Mint,
    >>::get_extension::<TransferFeeConfig>(&mint)?;
    let withheld_amount = u64::from(transfer_fee_config.withheld_amount);

    if withheld_amount == 0 {
        msg!("   ❌ No withheld tokens on mint to withdraw. Run harvest crank first.");
        return Ok(());
    }

    msg!(
        "   Mint has {} tokens to withdraw",
        (withheld_amount as f64) / 1e6
    );

    // 2. Withdraw ALL tokens from Mint -> Authority's Temp Vault
    let withdraw_authority_bump = ctx.bumps.withdraw_withheld_authority;
    let withdraw_authority_seeds = &[
        WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref(),
        &[withdraw_authority_bump],
    ];
    let withdraw_authority_signer = &[&withdraw_authority_seeds[..]];

    withdraw_withheld_tokens_from_mint(CpiContext::new_with_signer(
        ctx.accounts.token_program_2022.to_account_info(),
        WithdrawWithheldTokensFromMint {
            destination: ctx
                .accounts
                .withdraw_authority_token_account
                .to_account_info(),
            authority: ctx.accounts.withdraw_withheld_authority.to_account_info(),
            mint: ctx.accounts.minebtc_mint.to_account_info(),
            token_program_id: ctx.accounts.token_program_2022.to_account_info(),
        },
        withdraw_authority_signer,
    ))?;

    // 3. Calculate distribution amounts based on TaxConfig percentages
    let tax_config = &ctx.accounts.tax_config;
    let nft_floor_sweep_amount =
        helper::mul_div(withheld_amount, tax_config.nft_floor_sweep_pct as u64, 100)? as u64;
    let faction_treasury_amount =
        helper::mul_div(withheld_amount, tax_config.faction_treasury_pct as u64, 100)? as u64;
    let burn_amount = helper::mul_div(withheld_amount, tax_config.burn_pct as u64, 100)? as u64;
    // Remainder goes back to the minebtc vault
    let vault_return = withheld_amount
        .saturating_sub(nft_floor_sweep_amount)
        .saturating_sub(faction_treasury_amount)
        .saturating_sub(burn_amount);

    msg!("   Splitting {} tokens:", (withheld_amount as f64) / 1e6);
    msg!(
        "   - NFT Floor Sweep: {} ({}%)",
        (nft_floor_sweep_amount as f64) / 1e6,
        tax_config.nft_floor_sweep_pct
    );
    msg!(
        "   - Faction Treasury: {} ({}%)",
        (faction_treasury_amount as f64) / 1e6,
        tax_config.faction_treasury_pct
    );
    msg!(
        "   - Burn: {} ({}%)",
        (burn_amount as f64) / 1e6,
        tax_config.burn_pct
    );
    msg!("   - Back to Vault: {}", (vault_return as f64) / 1e6);

    // 4. Distribute the funds (all signed by the same PDA)

    // Transfer to NFT floor sweep
    if nft_floor_sweep_amount > 0 {
        token_2022::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program_2022.to_account_info(),
                TransferChecked {
                    from: ctx
                        .accounts
                        .withdraw_authority_token_account
                        .to_account_info(),
                    mint: ctx.accounts.minebtc_mint.to_account_info(),
                    to: ctx.accounts.nft_floor_sweep_vault.to_account_info(),
                    authority: ctx.accounts.withdraw_withheld_authority.to_account_info(),
                },
                withdraw_authority_signer,
            ),
            nft_floor_sweep_amount,
            ctx.accounts.minebtc_mint.decimals,
        )?;
        msg!(
            "   ✅ Transferred {} tokens to NFT floor sweep vault",
            (nft_floor_sweep_amount as f64) / 1e6
        );
    }

    // Transfer to faction treasury
    if faction_treasury_amount > 0 {
        token_2022::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program_2022.to_account_info(),
                TransferChecked {
                    from: ctx
                        .accounts
                        .withdraw_authority_token_account
                        .to_account_info(),
                    mint: ctx.accounts.minebtc_mint.to_account_info(),
                    to: ctx.accounts.faction_treasury_vault.to_account_info(),
                    authority: ctx.accounts.withdraw_withheld_authority.to_account_info(),
                },
                withdraw_authority_signer,
            ),
            faction_treasury_amount,
            ctx.accounts.minebtc_mint.decimals,
        )?;
        msg!(
            "   ✅ Transferred {} tokens to Faction treasury vault",
            (faction_treasury_amount as f64) / 1e6
        );
    }

    // Burn
    if burn_amount > 0 {
        token_2022::burn(
            CpiContext::new_with_signer(
                ctx.accounts.token_program_2022.to_account_info(),
                Burn {
                    mint: ctx.accounts.minebtc_mint.to_account_info(),
                    from: ctx
                        .accounts
                        .withdraw_authority_token_account
                        .to_account_info(),
                    authority: ctx.accounts.withdraw_withheld_authority.to_account_info(),
                },
                withdraw_authority_signer,
            ),
            burn_amount,
        )?;

        let tax_config_mut = &mut ctx.accounts.tax_config;
        tax_config_mut.total_burnt = tax_config_mut.total_burnt.saturating_add(burn_amount);
        msg!(
            "   ✅ Burnt {} tokens (Total burnt: {})",
            (burn_amount as f64) / 1e6,
            (tax_config_mut.total_burnt as f64) / 1e6
        );
    }

    // Transfer remainder back to minebtc vault
    if vault_return > 0 {
        token_2022::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program_2022.to_account_info(),
                TransferChecked {
                    from: ctx
                        .accounts
                        .withdraw_authority_token_account
                        .to_account_info(),
                    mint: ctx.accounts.minebtc_mint.to_account_info(),
                    to: ctx.accounts.minebtc_token_vault.to_account_info(),
                    authority: ctx.accounts.withdraw_withheld_authority.to_account_info(),
                },
                withdraw_authority_signer,
            ),
            vault_return,
            ctx.accounts.minebtc_mint.decimals,
        )?;
        msg!(
            "   ✅ Returned {} tokens to minebtc vault",
            (vault_return as f64) / 1e6
        );
    }

    let total_burnt = ctx.accounts.tax_config.total_burnt;
    emit!(TaxDistributed {
        total_tax_amount: withheld_amount,
        nft_floor_sweep_amount,
        faction_treasury_amount,
        burn_amount,
        total_burnt,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!("✅ [crank_distribute_tax] Tax distribution complete");
    Ok(())
}

// ========================================================================================
// ============================= DISTRIBUTION ROUND MANAGEMENT ===========================
// ========================================================================================

/// Start a new distribution round (callable by anyone after 1-day cooldown)
pub fn internal_start_distribution_round(ctx: Context<StartDistributionRound>) -> Result<()> {
    msg!("🎯 [start_distribution_round] Starting new distribution round");

    let tax_config = &mut ctx.accounts.tax_config;
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;

    // Check if 1 day has passed since last distribution round ended
    require!(!tax_config.round_active, ErrorCode::InvalidState);

    require!(
        current_time >= tax_config.end_timestamp + TaxConfig::DISTRIBUTION_COOLDOWN_SECONDS,
        ErrorCode::InvalidState
    );

    // Check faction treasury has funds
    let faction_treasury_balance = ctx.accounts.faction_treasury_vault.amount;
    require!(faction_treasury_balance > 0, ErrorCode::InvalidAmount);

    msg!(
        "   Faction treasury balance: {} tokens",
        faction_treasury_balance
    );

    // Reset distribution round state
    tax_config.round_active = true;
    tax_config.start_timestamp = current_time;
    tax_config.leaderboard_factions_count = 0;
    tax_config.rewards_calculated = false;
    tax_config.factions_claimed_count = 0;

    // Clear leaderboard and rewards
    tax_config.leaderboard_faction_ids.clear();
    tax_config.leaderboard_hashpower.clear();
    tax_config.faction_rewards.clear();
    tax_config.faction_claimed.clear();

    // Initialize vectors with capacity
    tax_config.leaderboard_faction_ids.resize(MAX_FACTIONS, 0);
    tax_config.leaderboard_hashpower.resize(MAX_FACTIONS, 0);
    tax_config.faction_rewards.resize(MAX_FACTIONS, 0);
    tax_config.faction_claimed.resize(MAX_FACTIONS, false);

    msg!("✅ [start_distribution_round] Distribution round started");
    msg!("   Round start timestamp: {}", current_time);

    emit!(DistributionRoundStarted {
        tax_config: ctx.accounts.tax_config.key(),
        faction_treasury_balance,
        start_timestamp: current_time,
        timestamp: current_time,
    });

    Ok(())
}

/// Calculate and store leaderboard position for one faction
/// Must be called 12 times (once per faction) to build complete leaderboard
pub fn internal_cal_faction_positions(ctx: Context<CalculateFactionLeaderboard>) -> Result<()> {
    msg!("📊 [cal_faction_positions] Calculating leaderboard position");

    // Store values before mutable borrow (for event emission)
    let tax_config_key = ctx.accounts.tax_config.key();
    let faction_state_key = ctx.accounts.faction_state.key();

    let tax_config = &mut ctx.accounts.tax_config;
    let faction_state = &ctx.accounts.faction_state;

    require!(tax_config.round_active, ErrorCode::InvalidState);

    require!(
        tax_config.leaderboard_factions_count < MAX_FACTIONS as u8,
        ErrorCode::InvalidState
    );

    // Calculate total hashpower for this faction (minebtc + lp)
    let total_hashpower = faction_state
        .total_dogebtc_hashpower
        .checked_add(faction_state.total_lp_hashpower)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    msg!("   Faction ID: {}", faction_state.faction_id);
    msg!(
        "   Total hashpower: {} (minebtc: {}, lp: {})",
        total_hashpower,
        faction_state.total_dogebtc_hashpower,
        faction_state.total_lp_hashpower
    );

    // Find insertion position (maintain descending order by hashpower)
    let mut insert_index = tax_config.leaderboard_factions_count as usize;
    for i in 0..tax_config.leaderboard_factions_count as usize {
        if total_hashpower > tax_config.leaderboard_hashpower[i] {
            insert_index = i;
            break;
        }
    }

    // Shift existing entries down
    for i in (insert_index..tax_config.leaderboard_factions_count as usize).rev() {
        if i + 1 < MAX_FACTIONS {
            tax_config.leaderboard_faction_ids[i + 1] = tax_config.leaderboard_faction_ids[i];
            tax_config.leaderboard_hashpower[i + 1] = tax_config.leaderboard_hashpower[i];
        }
    }

    // Insert new entry
    tax_config.leaderboard_faction_ids[insert_index] = faction_state.faction_id;
    tax_config.leaderboard_hashpower[insert_index] = total_hashpower;
    tax_config.leaderboard_factions_count += 1;

    msg!("   Rank: {} (0 = highest)", insert_index);
    msg!(
        "   Leaderboard count: {}/12",
        tax_config.leaderboard_factions_count
    );

    // Store values before emitting event
    let faction_id = faction_state.faction_id;
    let dogebtc_hashpower = faction_state.total_dogebtc_hashpower;
    let lp_hashpower = faction_state.total_lp_hashpower;
    let leaderboard_count = tax_config.leaderboard_factions_count;

    emit!(FactionLeaderboardPositionCalculated {
        tax_config: tax_config_key,
        faction_id,
        faction_state: faction_state_key,
        total_hashpower,
        dogebtc_hashpower,
        lp_hashpower,
        rank: insert_index as u8,
        leaderboard_count,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Calculate rewards for all factions based on leaderboard
/// Can only be called after all 12 factions are on leaderboard
pub fn internal_cal_faction_rewards(ctx: Context<CalculateFactionRewards>) -> Result<()> {
    msg!("💰 [cal_faction_rewards] Calculating faction rewards");

    // Store values before mutable borrow (for event emission)
    let tax_config_key = ctx.accounts.tax_config.key();

    let tax_config = &mut ctx.accounts.tax_config;

    require!(tax_config.round_active, ErrorCode::InvalidState);

    require!(
        tax_config.leaderboard_factions_count == MAX_FACTIONS as u8,
        ErrorCode::InvalidState
    );

    require!(!tax_config.rewards_calculated, ErrorCode::InvalidState);

    // Get total faction treasury balance
    let total_treasury = ctx.accounts.faction_treasury_vault.amount;
    require!(total_treasury > 0, ErrorCode::InvalidAmount);

    msg!("   Total treasury: {} tokens", total_treasury);

    // Calculate rewards based on rank:
    // Rank 0 (1st): 25%
    // Rank 1 (2nd): 15%
    // Rank 2 (3rd): 10%
    // Ranks 3-11: Randomly select one to get remaining 50%

    let first_place_reward = total_treasury * 25 / M_HUNDRED;
    let second_place_reward = total_treasury * 15 / M_HUNDRED;
    let third_place_reward = total_treasury * 10 / M_HUNDRED;
    let remaining_amount =
        total_treasury - first_place_reward - second_place_reward - third_place_reward;

    // Randomly select one faction from ranks 3-11 to get remaining 50%
    // Use current timestamp as seed for pseudo-random selection
    let clock = Clock::get()?;
    let random_seed = clock.unix_timestamp as u64;
    let random_index = 3 + (random_seed % 9) as usize; // Random index between 3-11

    msg!("   Reward distribution:");
    msg!(
        "     Rank 0 ({}): {} tokens (25%)",
        tax_config.leaderboard_faction_ids[0],
        first_place_reward
    );
    msg!(
        "     Rank 1 ({}): {} tokens (15%)",
        tax_config.leaderboard_faction_ids[1],
        second_place_reward
    );
    msg!(
        "     Rank 2 ({}): {} tokens (10%)",
        tax_config.leaderboard_faction_ids[2],
        third_place_reward
    );
    msg!(
        "     Rank {} ({}): {} tokens (50% - randomly selected)",
        random_index,
        tax_config.leaderboard_faction_ids[random_index],
        remaining_amount
    );

    // Set rewards
    tax_config.faction_rewards[0] = first_place_reward;
    tax_config.faction_rewards[1] = second_place_reward;
    tax_config.faction_rewards[2] = third_place_reward;
    tax_config.faction_rewards[random_index] = remaining_amount;

    // All other ranks get 0
    for i in 0..MAX_FACTIONS {
        if i != 0 && i != 1 && i != 2 && i != random_index {
            tax_config.faction_rewards[i] = 0;
        }
    }

    tax_config.rewards_calculated = true;

    // Store values before emitting event
    let first_place_faction_id = tax_config.leaderboard_faction_ids[0];
    let second_place_faction_id = tax_config.leaderboard_faction_ids[1];
    let third_place_faction_id = tax_config.leaderboard_faction_ids[2];
    let random_winner_faction_id = tax_config.leaderboard_faction_ids[random_index];

    emit!(FactionRewardsCalculated {
        tax_config: tax_config_key,
        total_treasury,
        first_place_faction_id,
        first_place_reward: first_place_reward,
        second_place_faction_id,
        second_place_reward: second_place_reward,
        third_place_faction_id,
        third_place_reward: third_place_reward,
        random_winner_faction_id,
        random_winner_reward: remaining_amount,
        timestamp: clock.unix_timestamp,
    });

    msg!("✅ [cal_faction_rewards] Rewards calculated");

    Ok(())
}

/// Claim treasury rewards for one faction
/// Adds rewards to staking reward indexes (50% each to minebtc and lp stakers)
pub fn internal_claim_faction_treasury_rewards(
    ctx: Context<ClaimFactionTreasuryRewards>,
) -> Result<()> {
    msg!("🎁 [claim_faction_treasury_rewards] Claiming treasury rewards");

    let tax_config = &mut ctx.accounts.tax_config;
    let faction_state = &mut ctx.accounts.faction_state;

    require!(tax_config.round_active, ErrorCode::InvalidState);

    require!(tax_config.rewards_calculated, ErrorCode::InvalidState);

    // Find faction's rank in leaderboard
    let mut faction_rank: Option<usize> = None;
    for i in 0..tax_config.leaderboard_factions_count as usize {
        if tax_config.leaderboard_faction_ids[i] == faction_state.faction_id {
            faction_rank = Some(i);
            break;
        }
    }

    let rank = faction_rank.ok_or(ErrorCode::InvalidFactionId)?;
    let reward_amount = tax_config.faction_rewards[rank];

    require!(reward_amount > 0, ErrorCode::InvalidAmount);
    require!(
        !tax_config.faction_claimed[faction_state.faction_id as usize],
        ErrorCode::InvalidState
    );

    msg!("   Faction ID: {}", faction_state.faction_id);
    msg!("   Rank: {}", rank);
    msg!("   Reward amount: {} tokens", reward_amount);

    // Split reward 50/50 between minebtc and lp stakers
    let minebtc_reward = reward_amount / 2;
    let lp_reward = reward_amount - minebtc_reward; // Handle odd amounts

    msg!(
        "   Split: {} to minebtc stakers, {} to lp stakers",
        minebtc_reward,
        lp_reward
    );

    // Transfer tokens from treasury vault to emission vault
    // faction_treasury_vault authority is withdraw_withheld_authority (set at init),
    // so we must sign with that PDA
    let withdraw_authority_bump = ctx.bumps.withdraw_withheld_authority;
    let withdraw_authority_seeds = &[
        WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref(),
        &[withdraw_authority_bump],
    ];
    let withdraw_authority_signer = &[&withdraw_authority_seeds[..]];

    token_2022::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program_2022.to_account_info(),
            token_2022::TransferChecked {
                from: ctx.accounts.faction_treasury_vault.to_account_info(),
                mint: ctx.accounts.minebtc_mint.to_account_info(),
                to: ctx.accounts.minebtc_emission_vault.to_account_info(),
                authority: ctx
                    .accounts
                    .withdraw_withheld_authority
                    .to_account_info(),
            },
            withdraw_authority_signer,
        ),
        reward_amount,
        ctx.accounts.minebtc_mint.decimals,
    )?;

    // Update reward indexes for minebtc stakers
    if minebtc_reward > 0 && faction_state.total_dogebtc_hashpower > 0 {
        let index_increase = helper::mul_div(
            minebtc_reward,
            INDEX_PRECISION,
            faction_state.total_dogebtc_hashpower,
        )?;
        faction_state.dogebtc_dogebtc_reward_index =
            faction_state.dogebtc_dogebtc_reward_index + index_increase;
    }

    // Update reward indexes for lp stakers
    if lp_reward > 0 && faction_state.total_lp_hashpower > 0 {
        let index_increase =
            helper::mul_div(lp_reward, INDEX_PRECISION, faction_state.total_lp_hashpower)?;
        faction_state.lp_dogebtc_reward_index =
            faction_state.lp_dogebtc_reward_index + index_increase;
    }

    // Mark faction as claimed
    tax_config.faction_claimed[faction_state.faction_id as usize] = true;
    tax_config.factions_claimed_count += 1;

    // Clear reward for this faction
    tax_config.faction_rewards[rank] = 0;

    msg!("✅ [claim_faction_treasury_rewards] Rewards claimed and distributed");
    msg!(
        "   Updated dogebtc_dogebtc_reward_index: {}",
        faction_state.dogebtc_dogebtc_reward_index
    );
    msg!(
        "   Updated lp_dogebtc_reward_index: {}",
        faction_state.lp_dogebtc_reward_index
    );

    emit!(FactionTreasuryRewardsClaimed {
        tax_config: ctx.accounts.tax_config.key(),
        faction_id: faction_state.faction_id,
        faction_state: ctx.accounts.faction_state.key(),
        rank: rank as u8,
        total_reward: reward_amount,
        minebtc_staker_reward: minebtc_reward,
        lp_staker_reward: lp_reward,
        minebtc_emission_vault: ctx.accounts.minebtc_emission_vault.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Finish distribution round (check all factions claimed and reset state)
pub fn internal_finish_distribution_round(ctx: Context<FinishDistributionRound>) -> Result<()> {
    msg!("🏁 [finish_distribution_round] Finishing distribution round");

    let tax_config = &mut ctx.accounts.tax_config;
    let clock = Clock::get()?;

    require!(tax_config.round_active, ErrorCode::InvalidState);

    require!(tax_config.rewards_calculated, ErrorCode::InvalidState);

    // Check all factions with rewards have claimed
    let mut all_claimed = true;
    for i in 0..MAX_FACTIONS {
        if tax_config.faction_rewards[i] > 0 {
            let faction_id = tax_config.leaderboard_faction_ids[i];
            if !tax_config.faction_claimed[faction_id as usize] {
                all_claimed = false;
                msg!(
                    "   ⚠️ Faction {} (rank {}) has not claimed rewards",
                    faction_id,
                    i
                );
            }
        }
    }

    require!(all_claimed, ErrorCode::InvalidState);

    // Reset distribution round state
    tax_config.round_active = false;
    tax_config.end_timestamp = clock.unix_timestamp;
    tax_config.rewards_calculated = false;
    tax_config.leaderboard_factions_count = 0;
    tax_config.factions_claimed_count = 0;

    // Clear leaderboard and rewards
    tax_config.leaderboard_faction_ids.clear();
    tax_config.leaderboard_hashpower.clear();
    tax_config.faction_rewards.clear();
    tax_config.faction_claimed.clear();

    let end_timestamp = tax_config.end_timestamp;
    let next_round_start_after = end_timestamp + TaxConfig::DISTRIBUTION_COOLDOWN_SECONDS;

    msg!("✅ [finish_distribution_round] Distribution round finished");
    msg!(
        "   Next round can start after: {} seconds",
        TaxConfig::DISTRIBUTION_COOLDOWN_SECONDS
    );

    emit!(DistributionRoundFinished {
        tax_config: ctx.accounts.tax_config.key(),
        end_timestamp,
        next_round_start_after,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ========================================================================================
// ============================= ACCOUNT CONTEXTS =========================================
// ========================================================================================

#[derive(Accounts)]
pub struct InitializeTaxConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = TaxConfig::LEN,
        seeds = [TAX_CONFIG_SEED.as_ref()],
        bump
    )]
    pub tax_config: Account<'info, TaxConfig>,

    /// CHECK: Withdraw withheld authority PDA (signer-only, no data)
    #[account(
        init_if_needed,
        payer = authority,
        space = 0,
        seeds = [WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref()],
        bump
    )]
    pub withdraw_withheld_authority: AccountInfo<'info>,

    /// CHECK: Faction treasury vault token account (will be created)
    #[account(
        init,
        payer = authority,
        token::mint = minebtc_mint,
        token::authority = withdraw_withheld_authority,
        token::token_program = token_program_2022,
        seeds = [FACTION_TREASURY_VAULT_SEED.as_ref()],
        bump
    )]
    pub faction_treasury_vault: InterfaceAccount<'info, TokenAccount2022>,

    /// CHECK: NFT floor sweep vault token account (will be created)
    #[account(
        init,
        payer = authority,
        token::mint = minebtc_mint,
        token::authority = withdraw_withheld_authority,
        token::token_program = token_program_2022,
        seeds = [NFT_FLOOR_SWEEP_VAULT_SEED.as_ref()],
        bump
    )]
    pub nft_floor_sweep_vault: InterfaceAccount<'info, TokenAccount2022>,

    /// CHECK: NFT sale SOL vault PDA (system account for SOL)
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [NFT_SALE_SOL_VAULT_SEED.as_ref()],
        bump,
        owner = system_program.key()
    )]
    pub nft_sale_sol_vault: UncheckedAccount<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub minebtc_mint: InterfaceAccount<'info, Mint>,

    pub token_program_2022: Program<'info, anchor_spl::token_2022::Token2022>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateTaxConfig<'info> {
    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED.as_ref()],
        bump = tax_config.bump
    )]
    pub tax_config: Account<'info, TaxConfig>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateNftFloorSweepWhitelist<'info> {
    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED.as_ref()],
        bump = tax_config.bump
    )]
    pub tax_config: Account<'info, TaxConfig>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawNftFloorSweepFunds<'info> {
    #[account(
        seeds = [TAX_CONFIG_SEED.as_ref()],
        bump = tax_config.bump
    )]
    pub tax_config: Account<'info, TaxConfig>,

    /// CHECK: Whitelisted address that can withdraw funds
    pub whitelisted_address: Signer<'info>,

    /// CHECK: Withdraw withheld authority PDA (signs for transfers from vault)
    #[account(
        seeds = [WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref()],
        bump
    )]
    pub withdraw_withheld_authority: AccountInfo<'info>,

    #[account(mut)]
    pub nft_floor_sweep_vault: InterfaceAccount<'info, TokenAccount2022>,

    #[account(mut)]
    /// CHECK: Whitelisted address's token account (receives MineBtc)
    pub whitelisted_token_account: InterfaceAccount<'info, TokenAccount2022>,

    pub minebtc_mint: InterfaceAccount<'info, Mint>,

    pub token_program_2022: Program<'info, anchor_spl::token_2022::Token2022>,
}

#[derive(Accounts)]
pub struct CrankHarvestFees<'info> {
    #[account(mut)]
    /// CHECK: The mint account must be the one that is configured with the TransferFeeConfig extension
    pub minebtc_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: This is the Token-2022 Program.
    /// We use AccountInfo<'info> instead of Interface<>
    /// to solve the lifetime invariance error.
    #[account(address = token_2022::ID @ ErrorCode::InvalidProgramId)]
    pub token_program_2022: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CrankDistributeTax<'info> {
    /// CHECK: The PDA authority for withdrawing fees
    #[account(seeds = [WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref()], bump)]
    pub withdraw_withheld_authority: AccountInfo<'info>,

    #[account(mut)]
    pub minebtc_mint: InterfaceAccount<'info, Mint>,

    /// The temporary vault that receives the full tax amount before splitting
    #[account(mut)]
    pub withdraw_authority_token_account: InterfaceAccount<'info, TokenAccount2022>,

    /// NFT Floor Sweep vault
    #[account(mut)]
    pub nft_floor_sweep_vault: InterfaceAccount<'info, TokenAccount2022>,

    /// Faction Treasury vault
    #[account(mut)]
    pub faction_treasury_vault: InterfaceAccount<'info, TokenAccount2022>,

    /// MineBtc token vault (receives remainder)
    #[account(mut)]
    pub minebtc_token_vault: InterfaceAccount<'info, TokenAccount2022>,

    #[account(mut, seeds = [TAX_CONFIG_SEED.as_ref()], bump = tax_config.bump)]
    pub tax_config: Account<'info, TaxConfig>,

    pub token_program_2022: Program<'info, anchor_spl::token_2022::Token2022>,
}

#[derive(Accounts)]
pub struct StartDistributionRound<'info> {
    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED.as_ref()],
        bump = tax_config.bump
    )]
    pub tax_config: Account<'info, TaxConfig>,

    #[account(mut)]
    /// CHECK: Faction treasury vault (checked for balance)
    pub faction_treasury_vault: InterfaceAccount<'info, TokenAccount2022>,
}

#[derive(Accounts)]
pub struct CalculateFactionLeaderboard<'info> {
    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED.as_ref()],
        bump = tax_config.bump
    )]
    pub tax_config: Account<'info, TaxConfig>,

    #[account()]
    pub faction_state: Account<'info, FactionState>,
}

#[derive(Accounts)]
pub struct CalculateFactionRewards<'info> {
    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED.as_ref()],
        bump = tax_config.bump
    )]
    pub tax_config: Account<'info, TaxConfig>,

    #[account(mut)]
    /// CHECK: Faction treasury vault (checked for balance)
    pub faction_treasury_vault: InterfaceAccount<'info, TokenAccount2022>,
}

#[derive(Accounts)]
pub struct ClaimFactionTreasuryRewards<'info> {
    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED.as_ref()],
        bump = tax_config.bump
    )]
    pub tax_config: Account<'info, TaxConfig>,

    #[account(mut)]
    pub faction_state: Account<'info, FactionState>,

    #[account(mut)]
    /// CHECK: Faction treasury vault (authority = withdraw_withheld_authority)
    pub faction_treasury_vault: InterfaceAccount<'info, TokenAccount2022>,

    #[account(mut)]
    /// CHECK: MineBtc emission vault (receives transferred tokens)
    pub minebtc_emission_vault: InterfaceAccount<'info, TokenAccount2022>,

    /// CHECK: Withdraw withheld authority PDA — signs transfers from faction_treasury_vault
    /// (this is the authority that was set when faction_treasury_vault was initialized)
    #[account(
        seeds = [WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref()],
        bump
    )]
    pub withdraw_withheld_authority: AccountInfo<'info>,

    /// CHECK: MineBtc mint (for transfer decimals)
    pub minebtc_mint: InterfaceAccount<'info, Mint>,

    pub token_program_2022: Program<'info, anchor_spl::token_2022::Token2022>,
}

#[derive(Accounts)]
pub struct FinishDistributionRound<'info> {
    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED.as_ref()],
        bump = tax_config.bump
    )]
    pub tax_config: Account<'info, TaxConfig>,
}
