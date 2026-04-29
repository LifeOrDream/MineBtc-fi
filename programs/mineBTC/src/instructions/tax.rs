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
// Deflationary tax system using Token-2022 transfer fees.
//
// ## Tax Mechanics
//
// All dogeBTC transfers incur a 0.1% tax, split into:
// - **Burn**: Reducing total supply (default 25%)
// - **NFT Floor Sweep**: Funded for market-making (default 10%)
// - **Faction Treasury**: Distributed to stakers via faction_war leaderboard (default 40%)
// - **Back to Vault**: Recycled into mining emission pool (remainder)
//
// ## Faction Treasury Distribution
//
// After each faction_war settles, `claim_faction_treasury_for_faction_war` distributes
// the treasury vault based on story-event leaderboard rankings:
// - 80% rank-weighted (higher rank = more reward, everyone gets something)
// - 20% lucky draw (one random underdog faction from rank 5+)
//
// ## Key Functions
//
// - `crank_harvest_fees`: Harvests withheld fees from token accounts to mint.
// - `crank_distribute_tax`: Withdraws from mint, splits to vaults.
// - `claim_faction_treasury_for_faction_war`: Distributes treasury to stakers using faction_war rankings.
//

use crate::errors::ErrorCode;
use crate::events::*;
use crate::instructions::helper;
use crate::state::*;

fn seed_empty_faction_war_treasury_bucket(
    faction_war_config: &FactionWarConfig,
    faction_war_state: &mut FactionWarState,
    faction_war_state_bump: u8,
    seeded_treasury_base: u64,
) {
    faction_war_state.bump = faction_war_state_bump;
    faction_war_state.faction_war_id = faction_war_config.current_faction_war_id;
    faction_war_state.start_timestamp = 0;
    faction_war_state.stage = 0;
    faction_war_state.active_faction_count = 0;
    faction_war_state.total_dogebtc_mined_in_faction_war = 0;
    faction_war_state.faction_war_mining_pool = 0;
    faction_war_state.start_ranks = faction_war_config.prev_faction_war_mutation_ranks;
    faction_war_state.final_ranks = faction_war_config.prev_faction_war_mutation_ranks;
    faction_war_state.rank_deltas = [0i8; NUM_FACTIONS];
    faction_war_state.resolved_directions =
        [PredictionDirection::Neutral.as_index() as u8; NUM_FACTIONS];
    faction_war_state.faction_direction_totals = [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
    faction_war_state.loyalty_direction_totals = [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
    faction_war_state.faction_reward_pools = [0u64; NUM_FACTIONS];
    faction_war_state.loyalty_reward_pools = [0u64; NUM_FACTIONS];
    faction_war_state.faction_doge_reward_pools = [0u64; NUM_FACTIONS];
    faction_war_state.faction_round_wins = [0u16; NUM_FACTIONS];
    faction_war_state.faction_sol_totals = [0u64; NUM_FACTIONS];
    faction_war_state.faction_mutation_scores = [0u64; NUM_FACTIONS];
    faction_war_state.faction_mvp_user = [Pubkey::default(); NUM_FACTIONS];
    faction_war_state.faction_mvp_score = [0u64; NUM_FACTIONS];
    faction_war_state.faction_mvp_bonus = [0u64; NUM_FACTIONS];
    faction_war_state.eligible_doge_direction_totals =
        [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
    faction_war_state.treasury_reward_base_amount = seeded_treasury_base;
    faction_war_state.treasury_claimed_bitmap = 0;
}

fn ensure_active_faction_war_treasury_bucket<'info>(
    tax_config: &mut Account<'info, TaxConfig>,
    faction_war_config: &Account<'info, FactionWarConfig>,
    faction_war_state: &mut FactionWarState,
    faction_war_state_bump: u8,
) -> Result<()> {
    if faction_war_state.faction_war_id == 0 {
        let seeded_treasury_base = tax_config.unassigned_faction_war_treasury_amount;
        seed_empty_faction_war_treasury_bucket(
            faction_war_config,
            faction_war_state,
            faction_war_state_bump,
            seeded_treasury_base,
        );
        tax_config.unassigned_faction_war_treasury_amount = 0;
        msg!(
            "   🧱 Seeded empty faction-war treasury bucket for war {} (carried_unassigned={})",
            faction_war_state.faction_war_id,
            faction_war_state.treasury_reward_base_amount
        );
    } else {
        require!(
            faction_war_state.faction_war_id == faction_war_config.current_faction_war_id,
            ErrorCode::InvalidState
        );
        require!(
            faction_war_state.stage == 0,
            ErrorCode::FactionWarAlreadySettled
        );
    }

    Ok(())
}

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
    crate::log_fn!("tax", "internal_initialize_tax_config");
    msg!("🔧 [initialize_tax_config] Initializing tax system");

    require!(
        (nft_floor_sweep_pct as u64) + (faction_treasury_pct as u64) + (burn_pct as u64)
            <= M_HUNDRED,
        ErrorCode::InvalidAmount
    );

    let tax_config = &mut ctx.accounts.tax_config;

    tax_config.bump = ctx.bumps.tax_config;
    tax_config.nft_floor_sweep_pct = nft_floor_sweep_pct;
    tax_config.faction_treasury_pct = faction_treasury_pct;
    tax_config.burn_pct = burn_pct;
    tax_config.total_burnt = 0;
    tax_config.unassigned_faction_war_treasury_amount = 0;

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
    crate::log_fn!("tax", "internal_update_tax_config");
    msg!("🔧 [update_tax_config] Updating tax distribution percentages");

    require!(
        (nft_floor_sweep_pct as u64) + (faction_treasury_pct as u64) + (burn_pct as u64)
            <= M_HUNDRED,
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
    crate::log_fn!("tax", "internal_update_nft_floor_sweep_whitelist");
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
    crate::log_fn!("tax", "internal_withdraw_nft_floor_sweep_funds");
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

/// STEP 1: Harvest withheld fees from token accounts → mint.
///
/// A keeper bot discovers token accounts with withheld fees off-chain (via Helius
/// DAS API or getProgramAccounts) and passes them as `remaining_accounts`.
/// This CPI aggregates their fees into `minebtc_mint.withheld_amount`.
///
/// Callable by anyone. Designed to be called in batches (~20 accounts per tx).
pub fn internal_crank_harvest_fees<'info>(
    ctx: Context<'_, '_, '_, 'info, CrankHarvestFees<'info>>,
) -> Result<()> {
    crate::log_fn!("tax", "internal_crank_harvest_fees");
    let sources = ctx.remaining_accounts;
    if sources.is_empty() {
        msg!("🌱 Harvest: no source accounts, skipping");
        return Ok(());
    }
    msg!(
        "🌱 Harvesting withheld fees from {} accounts → mint {}",
        sources.len(),
        ctx.accounts.minebtc_mint.key()
    );

    harvest_withheld_tokens_to_mint(
        CpiContext::new(
            ctx.accounts.token_program_2022.to_account_info(),
            HarvestWithheldTokensToMint {
                mint: ctx.accounts.minebtc_mint.to_account_info(),
                token_program_id: ctx.accounts.token_program_2022.to_account_info(),
            },
        ),
        sources.to_vec(),
    )?;

    msg!("   ✅ Harvest complete");
    Ok(())
}

/// STEP 2: "Withdraw & Distribute" tax from the MINT
///
/// This function should be called *after* `crank_harvest_fees` has
/// been run. It withdraws the *total* accumulated tax from the
/// mint account and distributes it according to TaxConfig percentages.
///
/// Callable by anyone - program-controlled withdraw authority
#[inline(never)]
fn init_or_load_tax_faction_war_state<'info>(
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    faction_war_state_info: &AccountInfo<'info>,
    faction_war_id: u64,
    faction_war_state_bump: u8,
) -> Result<Box<FactionWarState>> {
    let faction_war_id_bytes = faction_war_id.to_le_bytes();
    let faction_war_state_bump_seed = [faction_war_state_bump];
    let faction_war_state_seeds: &[&[u8]] = &[
        FACTION_WAR_STATE_SEED,
        faction_war_id_bytes.as_ref(),
        faction_war_state_bump_seed.as_ref(),
    ];
    let created = helper::init_pda_account_if_needed(
        payer,
        faction_war_state_info,
        system_program,
        faction_war_state_seeds,
        FactionWarState::LEN,
        &FactionWarState::blank(),
    )?;
    msg!(
        "🏛️ [init_or_load_tax_faction_war_state] faction_war_id={} account={} created={}",
        faction_war_id,
        faction_war_state_info.key(),
        created
    );
    Ok(Box::new(helper::load_account_data::<FactionWarState>(
        faction_war_state_info,
    )?))
}

#[inline(never)]
pub fn internal_crank_distribute_tax<'info>(
    accounts: &mut CrankDistributeTax<'info>,
    faction_war_id: u64,
    faction_war_state_bump: u8,
    withdraw_authority_bump: u8,
) -> Result<()> {
    crate::log_fn!("tax", "internal_crank_distribute_tax");
    msg!("💰 [crank_distribute_tax] Withdrawing *total* tax from mint");
    let faction_war_state_info = accounts.faction_war_state.as_ref();
    let mut faction_war_state = init_or_load_tax_faction_war_state(
        accounts.caller.as_ref(),
        accounts.system_program.as_ref(),
        faction_war_state_info,
        faction_war_id,
        faction_war_state_bump,
    )?;

    // 1. Get the total amount of tax sitting on the mint account
    // We must reload to get the most up-to-date data after harvesting
    accounts.minebtc_mint.reload()?;

    // Read withheld_amount from TransferFeeConfig extension.
    // Scoped block so the borrow is dropped before CPI calls below.
    let withheld_amount = {
        let mint_account_info = accounts.minebtc_mint.to_account_info();
        let mint_data = mint_account_info.try_borrow_data()?;
        let mint =
            StateWithExtensions::<anchor_spl::token_2022::spl_token_2022::state::Mint>::unpack(
                &mint_data,
            )?;
        let transfer_fee_config = <StateWithExtensions<
            anchor_spl::token_2022::spl_token_2022::state::Mint,
        > as BaseStateWithExtensions<
            anchor_spl::token_2022::spl_token_2022::state::Mint,
        >>::get_extension::<TransferFeeConfig>(&mint)?;
        u64::from(transfer_fee_config.withheld_amount)
    }; // borrow dropped here

    if withheld_amount == 0 {
        msg!("   ❌ No withheld tokens on mint to withdraw. Run harvest crank first.");
        return Ok(());
    }

    msg!(
        "   Mint has {} tokens to withdraw",
        (withheld_amount as f64) / 1e6
    );

    // 2. Withdraw ALL tokens from Mint -> Authority's Temp Vault
    let withdraw_authority_seeds = &[
        WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref(),
        &[withdraw_authority_bump],
    ];
    let withdraw_authority_signer = &[&withdraw_authority_seeds[..]];

    withdraw_withheld_tokens_from_mint(CpiContext::new_with_signer(
        accounts.token_program_2022.to_account_info(),
        WithdrawWithheldTokensFromMint {
            destination: accounts.withdraw_authority_token_account.to_account_info(),
            authority: accounts.withdraw_withheld_authority.to_account_info(),
            mint: accounts.minebtc_mint.to_account_info(),
            token_program_id: accounts.token_program_2022.to_account_info(),
        },
        withdraw_authority_signer,
    ))?;

    // 3. Calculate distribution amounts based on TaxConfig percentages
    let tax_config = &mut accounts.tax_config;
    require!(
        accounts.faction_war_config.current_faction_war_id == faction_war_id,
        ErrorCode::InvalidParameters
    );
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
                accounts.token_program_2022.to_account_info(),
                TransferChecked {
                    from: accounts.withdraw_authority_token_account.to_account_info(),
                    mint: accounts.minebtc_mint.to_account_info(),
                    to: accounts.nft_floor_sweep_vault.to_account_info(),
                    authority: accounts.withdraw_withheld_authority.to_account_info(),
                },
                withdraw_authority_signer,
            ),
            nft_floor_sweep_amount,
            accounts.minebtc_mint.decimals,
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
                accounts.token_program_2022.to_account_info(),
                TransferChecked {
                    from: accounts.withdraw_authority_token_account.to_account_info(),
                    mint: accounts.minebtc_mint.to_account_info(),
                    to: accounts.faction_treasury_vault.to_account_info(),
                    authority: accounts.withdraw_withheld_authority.to_account_info(),
                },
                withdraw_authority_signer,
            ),
            faction_treasury_amount,
            accounts.minebtc_mint.decimals,
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
                accounts.token_program_2022.to_account_info(),
                Burn {
                    mint: accounts.minebtc_mint.to_account_info(),
                    from: accounts.withdraw_authority_token_account.to_account_info(),
                    authority: accounts.withdraw_withheld_authority.to_account_info(),
                },
                withdraw_authority_signer,
            ),
            burn_amount,
        )?;

        tax_config.total_burnt = tax_config
            .total_burnt
            .checked_add(burn_amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "   ✅ Burnt {} tokens (Total burnt: {})",
            (burn_amount as f64) / 1e6,
            (tax_config.total_burnt as f64) / 1e6
        );
    }

    // Transfer remainder back to minebtc vault
    if vault_return > 0 {
        token_2022::transfer_checked(
            CpiContext::new_with_signer(
                accounts.token_program_2022.to_account_info(),
                TransferChecked {
                    from: accounts.withdraw_authority_token_account.to_account_info(),
                    mint: accounts.minebtc_mint.to_account_info(),
                    to: accounts.minebtc_token_vault.to_account_info(),
                    authority: accounts.withdraw_withheld_authority.to_account_info(),
                },
                withdraw_authority_signer,
            ),
            vault_return,
            accounts.minebtc_mint.decimals,
        )?;
        msg!(
            "   ✅ Returned {} tokens to minebtc vault",
            (vault_return as f64) / 1e6
        );
    }

    if faction_treasury_amount > 0 {
        if accounts.faction_war_config.is_active {
            ensure_active_faction_war_treasury_bucket(
                tax_config,
                &accounts.faction_war_config,
                faction_war_state.as_mut(),
                faction_war_state_bump,
            )?;
            faction_war_state.treasury_reward_base_amount = faction_war_state
                .treasury_reward_base_amount
                .checked_add(faction_treasury_amount)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!(
                "   🏴 Attributed {} dogeBTC treasury tax to faction war {} (base now {})",
                (faction_treasury_amount as f64) / 1e6,
                faction_war_state.faction_war_id,
                (faction_war_state.treasury_reward_base_amount as f64) / 1e6
            );
        } else {
            tax_config.unassigned_faction_war_treasury_amount = tax_config
                .unassigned_faction_war_treasury_amount
                .checked_add(faction_treasury_amount)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!(
                "   💤 Faction wars inactive; queued {} dogeBTC treasury tax for the next war (queued total {})",
                (faction_treasury_amount as f64) / 1e6,
                (tax_config.unassigned_faction_war_treasury_amount as f64) / 1e6
            );
        }
    }

    let total_burnt = accounts.tax_config.total_burnt;
    helper::store_account_data(faction_war_state_info, faction_war_state.as_ref())?;
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
// ============================= FACTION TREASURY DISTRIBUTION ============================
// ========================================================================================

/// Distribute faction treasury rewards for a settled faction_war.
///
/// Uses the faction_war's mutation-based `final_ranks` to determine reward tiers:
///   1st = 25% | 2nd = 15% | 3rd = 10% | random from 4th+ = 50%
///
/// Called once per faction per faction_war. Each call transfers that faction's share
/// from the treasury vault to the emission vault and updates staker reward indexes.
///
/// Permissionless: anyone can crank this after a faction_war settles.
pub fn internal_claim_faction_treasury_for_faction_war(
    ctx: Context<ClaimFactionTreasuryForFactionWar>,
    faction_war_id: u64,
) -> Result<()> {
    crate::log_fn!("tax", "internal_claim_faction_treasury_for_faction_war");
    let faction_war_state = &mut ctx.accounts.faction_war_state;
    let fs = &mut ctx.accounts.faction_state;
    let fid = fs.faction_id;

    msg!(
        "💰 [claim_faction_treasury] FactionWar #{}, faction {}, treasury={}",
        faction_war_id,
        fid,
        ctx.accounts.faction_treasury_vault.amount
    );

    require!(
        faction_war_state.stage == 1,
        ErrorCode::FactionWarNotSettled
    );
    require!(
        faction_war_state.faction_war_id == faction_war_id,
        ErrorCode::InvalidState
    );

    // Prevent double-claim for this faction
    let faction_bit = 1u16 << fid;
    require!(
        faction_war_state.treasury_claimed_bitmap & faction_bit == 0,
        ErrorCode::FactionWarRewardsAlreadyClaimed
    );

    let treasury_balance = ctx.accounts.faction_treasury_vault.amount;
    let treasury_base_amount = faction_war_state.treasury_reward_base_amount;

    let active_factions = faction_war_state.active_faction_count as usize;
    require!(
        (fid as usize) < active_factions,
        ErrorCode::InvalidFactionId
    );

    // Determine this faction's rank from faction_war final_ranks
    let rank = faction_war_state.final_ranks[fid as usize] as usize;

    // --- 80% rank-weighted: rank_points = active_factions - rank ---
    // #1 gets the most points, #last gets 1 point. Everyone gets something.
    let rank_points = active_factions.saturating_sub(rank) as u128;
    let total_rank_points: u128 = (1..=active_factions as u128).sum(); // N*(N+1)/2

    let rank_pool = (treasury_base_amount as u128)
        .checked_mul(TaxConfig::RANK_WEIGHTED_BPS as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(BASIS_POINTS_DENOMINATOR as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    let rank_reward = if total_rank_points > 0 {
        rank_pool
            .checked_mul(rank_points)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(total_rank_points)
            .ok_or(ErrorCode::ArithmeticOverflow)?
    } else {
        0
    };

    // --- 20% lucky draw: one random faction from rank 5+ wins the whole pot ---
    // Only lower-ranked countries are eligible (top 5 already benefit from rank weighting).
    // If fewer than 6 factions exist, the lowest-ranked faction gets it.
    let eligible_start = 5.min(active_factions.saturating_sub(1));
    let eligible_count = active_factions.saturating_sub(eligible_start);
    let lucky_rank = if eligible_count > 0 {
        eligible_start + (faction_war_id as usize % eligible_count)
    } else {
        active_factions.saturating_sub(1)
    };
    let lucky_reward = if rank == lucky_rank {
        (treasury_base_amount as u128)
            .checked_mul(TaxConfig::LUCKY_DRAW_BPS as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(BASIS_POINTS_DENOMINATOR as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
    } else {
        0
    };

    let total_reward_u128 = rank_reward
        .checked_add(lucky_reward)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let reward_amount =
        u64::try_from(total_reward_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;

    msg!(
        "   Rank {}: base={}, current_vault={}, rank_reward={}, lucky_rank={}, lucky_reward={}, total={}",
        rank,
        treasury_base_amount,
        treasury_balance,
        rank_reward,
        lucky_rank,
        lucky_reward,
        reward_amount
    );

    if reward_amount == 0 {
        msg!("   ⚠️ No reward for faction {} (rank {})", fid, rank);
        faction_war_state.treasury_claimed_bitmap |= faction_bit;
        return Ok(());
    }

    // Split 50/50 between dogeBTC stakers and LP stakers
    let dogebtc_active = fs.total_dogebtc_hashpower > 0;
    let lp_active = fs.total_lp_hashpower > 0;
    let (dbtc_share, lp_share) = match (dogebtc_active, lp_active) {
        (true, true) => {
            let half = reward_amount / 2;
            (half, reward_amount - half)
        }
        (true, false) => (reward_amount, 0),
        (false, true) => (0, reward_amount),
        (false, false) => (0, 0),
    };
    let distributed = dbtc_share + lp_share;
    let recycled_amount = if dogebtc_active || lp_active {
        0
    } else {
        reward_amount
    };
    let total_transfer = distributed
        .checked_add(recycled_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    if total_transfer > 0 {
        require!(
            treasury_balance >= total_transfer,
            ErrorCode::InsufficientFunds
        );
        let seeds: &[&[&[u8]]] = &[&[
            WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref(),
            &[ctx.bumps.withdraw_withheld_authority],
        ]];
        token_2022::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program_2022.to_account_info(),
                token_2022::TransferChecked {
                    from: ctx.accounts.faction_treasury_vault.to_account_info(),
                    mint: ctx.accounts.minebtc_mint.to_account_info(),
                    to: ctx.accounts.minebtc_emission_vault.to_account_info(),
                    authority: ctx.accounts.withdraw_withheld_authority.to_account_info(),
                },
                seeds,
            ),
            total_transfer,
            ctx.accounts.minebtc_mint.decimals,
        )?;
        msg!(
            "   ✅ Transferred {} dogeBTC from treasury (dbtc_stakers={}, lp_stakers={}, recycled={})",
            total_transfer,
            dbtc_share,
            lp_share,
            recycled_amount
        );
    }

    if dbtc_share > 0 && fs.total_dogebtc_hashpower > 0 {
        fs.dogebtc_dogebtc_reward_index = fs
            .dogebtc_dogebtc_reward_index
            .checked_add(helper::mul_div(
                dbtc_share,
                INDEX_PRECISION,
                fs.total_dogebtc_hashpower,
            )?)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    if lp_share > 0 && fs.total_lp_hashpower > 0 {
        fs.lp_dogebtc_reward_index = fs
            .lp_dogebtc_reward_index
            .checked_add(helper::mul_div(
                lp_share,
                INDEX_PRECISION,
                fs.total_lp_hashpower,
            )?)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    faction_war_state.treasury_claimed_bitmap |= faction_bit;

    emit!(FactionTreasuryRewardsClaimed {
        faction_war_id,
        faction_id: fid,
        rank: rank as u8,
        reward_amount,
        dbtc_share,
        lp_share,
        recycled_amount,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

// ========================================================================================
// ============================= ACCOUNT CONTEXTS =========================================
// ========================================================================================

#[derive(Accounts)]
pub struct InitializeTaxConfig<'info> {
    #[account(init, payer = authority, space = TaxConfig::LEN, seeds = [TAX_CONFIG_SEED.as_ref()], bump)]
    pub tax_config: Account<'info, TaxConfig>,

    /// CHECK: Withdraw withheld authority PDA
    #[account(init_if_needed, payer = authority, space = 0, seeds = [WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref()], bump)]
    pub withdraw_withheld_authority: AccountInfo<'info>,

    #[account(init, payer = authority, token::mint = minebtc_mint, token::authority = withdraw_withheld_authority, token::token_program = token_program_2022, seeds = [FACTION_TREASURY_VAULT_SEED.as_ref()], bump)]
    pub faction_treasury_vault: InterfaceAccount<'info, TokenAccount2022>,

    #[account(init, payer = authority, token::mint = minebtc_mint, token::authority = withdraw_withheld_authority, token::token_program = token_program_2022, seeds = [NFT_FLOOR_SWEEP_VAULT_SEED.as_ref()], bump)]
    pub nft_floor_sweep_vault: InterfaceAccount<'info, TokenAccount2022>,

    /// CHECK: NFT sale SOL vault
    #[account(init, payer = authority, space = 0, seeds = [NFT_SALE_SOL_VAULT_SEED.as_ref()], bump, owner = system_program.key())]
    pub nft_sale_sol_vault: UncheckedAccount<'info>,

    pub minebtc_mint: InterfaceAccount<'info, Mint>,

    #[account(seeds = [GLOBAL_CONFIG_SEED.as_ref()], bump = global_config.bump, constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized)]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_program_2022: Program<'info, anchor_spl::token_2022::Token2022>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateTaxConfig<'info> {
    #[account(seeds = [GLOBAL_CONFIG_SEED.as_ref()], bump = global_config.bump, constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized)]
    pub global_config: Account<'info, GlobalConfig>,
    #[account(mut, seeds = [TAX_CONFIG_SEED.as_ref()], bump = tax_config.bump)]
    pub tax_config: Account<'info, TaxConfig>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateNftFloorSweepWhitelist<'info> {
    #[account(seeds = [GLOBAL_CONFIG_SEED.as_ref()], bump = global_config.bump, constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized)]
    pub global_config: Account<'info, GlobalConfig>,
    #[account(mut, seeds = [TAX_CONFIG_SEED.as_ref()], bump = tax_config.bump)]
    pub tax_config: Account<'info, TaxConfig>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawNftFloorSweepFunds<'info> {
    #[account(seeds = [TAX_CONFIG_SEED.as_ref()], bump = tax_config.bump)]
    pub tax_config: Account<'info, TaxConfig>,
    #[account(mut, constraint = nft_floor_sweep_vault.key() == tax_config.nft_floor_sweep_vault @ ErrorCode::InvalidAccount)]
    pub nft_floor_sweep_vault: InterfaceAccount<'info, TokenAccount2022>,
    /// CHECK: withdraw authority PDA
    #[account(seeds = [WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref()], bump)]
    pub withdraw_withheld_authority: AccountInfo<'info>,
    #[account(mut)]
    pub whitelisted_token_account: InterfaceAccount<'info, TokenAccount2022>,
    pub minebtc_mint: InterfaceAccount<'info, Mint>,
    pub whitelisted_address: Signer<'info>,
    pub token_program_2022: Program<'info, anchor_spl::token_2022::Token2022>,
}

#[derive(Accounts)]
pub struct CrankHarvestFees<'info> {
    #[account(mut)]
    pub minebtc_mint: InterfaceAccount<'info, Mint>,
    pub token_program_2022: Program<'info, anchor_spl::token_2022::Token2022>,
}

#[derive(Accounts)]
#[instruction(faction_war_id: u64)]
pub struct CrankDistributeTax<'info> {
    #[account(mut)]
    pub minebtc_mint: Box<InterfaceAccount<'info, Mint>>,
    /// CHECK: Withdraw withheld authority PDA
    #[account(seeds = [WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref()], bump)]
    pub withdraw_withheld_authority: AccountInfo<'info>,
    #[account(mut, constraint = withdraw_authority_token_account.owner == withdraw_withheld_authority.key() @ ErrorCode::Unauthorized)]
    pub withdraw_authority_token_account: Box<InterfaceAccount<'info, TokenAccount2022>>,
    #[account(mut, constraint = nft_floor_sweep_vault.key() == tax_config.nft_floor_sweep_vault @ ErrorCode::InvalidAccount)]
    pub nft_floor_sweep_vault: Box<InterfaceAccount<'info, TokenAccount2022>>,
    #[account(mut, constraint = faction_treasury_vault.key() == tax_config.faction_treasury_vault @ ErrorCode::InvalidAccount)]
    pub faction_treasury_vault: Box<InterfaceAccount<'info, TokenAccount2022>>,
    #[account(mut)]
    pub minebtc_token_vault: Box<InterfaceAccount<'info, TokenAccount2022>>,
    #[account(mut, seeds = [TAX_CONFIG_SEED.as_ref()], bump = tax_config.bump)]
    pub tax_config: Box<Account<'info, TaxConfig>>,
    #[account(
        seeds = [FACTION_WAR_CONFIG_SEED],
        bump = faction_war_config.bump
    )]
    pub faction_war_config: Box<Account<'info, FactionWarConfig>>,
    #[account(
        mut,
        seeds = [FACTION_WAR_STATE_SEED, &faction_war_id.to_le_bytes()],
        bump,
    )]
    /// CHECK: Program PDA; initialized manually in handler to keep parser stack small.
    /// Must remain `mut` because helper::store_account_data persists treasury attribution state.
    pub faction_war_state: UncheckedAccount<'info>,
    #[account(mut)]
    pub caller: Signer<'info>,
    pub token_program_2022: Program<'info, anchor_spl::token_2022::Token2022>,
    pub system_program: Program<'info, System>,
}

/// Claim faction treasury rewards for a settled faction_war.
/// Uses story-event leaderboard (faction_war final_ranks) -- no separate leaderboard needed.
#[derive(Accounts)]
#[instruction(faction_war_id: u64)]
pub struct ClaimFactionTreasuryForFactionWar<'info> {
    #[account(mut, seeds = [TAX_CONFIG_SEED.as_ref()], bump = tax_config.bump)]
    pub tax_config: Box<Account<'info, TaxConfig>>,
    #[account(
        mut,
        seeds = [FACTION_WAR_STATE_SEED, &faction_war_id.to_le_bytes()],
        bump = faction_war_state.bump
    )]
    pub faction_war_state: Box<Account<'info, FactionWarState>>,
    #[account(mut)]
    pub faction_state: Box<Account<'info, FactionState>>,
    #[account(mut, constraint = faction_treasury_vault.key() == tax_config.faction_treasury_vault @ ErrorCode::InvalidAccount)]
    pub faction_treasury_vault: Box<InterfaceAccount<'info, TokenAccount2022>>,
    #[account(mut)]
    pub minebtc_emission_vault: Box<InterfaceAccount<'info, TokenAccount2022>>,
    /// CHECK: PDA signer for treasury vault transfers
    #[account(seeds = [WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref()], bump)]
    pub withdraw_withheld_authority: AccountInfo<'info>,
    pub minebtc_mint: Box<InterfaceAccount<'info, Mint>>,
    pub token_program_2022: Program<'info, anchor_spl::token_2022::Token2022>,
}
