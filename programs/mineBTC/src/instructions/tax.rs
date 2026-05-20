//! Token-2022 tax harvesting and faction-treasury distribution.
//!
//! degenBTC uses Token-2022 transfer fees. This module owns the protocol side
//! of that tax loop:
//!
//! 1. `internal_crank_harvest_fees` harvests withheld fees from user token
//!    accounts into the mint's withheld-fee bucket.
//! 2. `internal_crank_distribute_tax` withdraws the mint bucket into the
//!    program-controlled withdraw-authority vault, then splits it into:
//!    faction treasury, burn, and the canonical mining emission vault.
//! 3. `internal_claim_faction_treasury_for_faction_war` releases the settled
//!    war's attributed faction-treasury amount into faction staking indexes,
//!    using the same faction-war ranks computed in `faction_war.rs`.
//!
//! Token-2022 fees also apply to the protocol's own transfers. Accounting in
//! this file therefore credits reward pools with post-fee delivered amounts,
//! not pre-fee transfer amounts. Any withheld fees created by these transfers
//! are harvested again in a later tax crank.
//!
//! File layout follows call order: tax config, harvest/distribute, faction
//! treasury claims, then account contexts.

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

use crate::errors::ErrorCode;
use crate::events::*;
use crate::instructions::helper;
use crate::state::*;

// ========================================================================================
// ============================= ADMIN FUNCTIONS ==========================================
// ========================================================================================

/// Initialize TaxConfig account and create vault token accounts.
/// Callable only by global config authority.
///
/// Tax splits: `treasury_pct` + `burn_pct` + (residual → mining vault).
/// NFT floor-sweep funding comes from `distribute_sol_fees`, not from this tax.
pub fn internal_initialize_tax_config(
    ctx: Context<InitializeTaxConfig>,
    treasury_pct: u8,
    burn_pct: u8,
) -> Result<()> {
    crate::log_fn!("tax", "internal_initialize_tax_config");
    msg!("🔧 [initialize_tax_config] Initializing tax system");

    let configured_pct_total = (treasury_pct as u64)
        .checked_add(burn_pct as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    require!(configured_pct_total <= M_HUNDRED, ErrorCode::InvalidAmount);

    let tax_config = &mut ctx.accounts.tax_config;

    tax_config.bump = ctx.bumps.tax_config;
    tax_config.treasury_pct = treasury_pct;
    tax_config.burn_pct = burn_pct;
    tax_config.total_burnt = 0;
    tax_config.unassigned_war_treasury_amount = 0;

    tax_config.withdraw_withheld_authority = ctx.accounts.withdraw_withheld_authority.key();
    tax_config.faction_treasury_vault = ctx.accounts.faction_treasury_vault.key();

    msg!("   ✅ TaxConfig initialized");
    msg!("   Faction Treasury: {}%", treasury_pct);
    msg!("   Burn: {}%", burn_pct);
    let vault_pct = u8::try_from(M_HUNDRED - configured_pct_total)
        .map_err(|_| ErrorCode::ArithmeticOverflow)?;
    msg!("   Back to Vault: {}%", vault_pct);
    msg!(
        "   Withdraw Authority: {}",
        tax_config.withdraw_withheld_authority
    );
    msg!(
        "   Faction Treasury Vault: {}",
        tax_config.faction_treasury_vault
    );

    Ok(())
}

/// Update tax distribution percentages
/// Callable only by global config authority
pub fn internal_update_tax_config(
    ctx: Context<UpdateTaxConfig>,
    treasury_pct: u8,
    burn_pct: u8,
) -> Result<()> {
    crate::log_fn!("tax", "internal_update_tax_config");
    msg!("🔧 [update_tax_config] Updating tax distribution percentages");

    let configured_pct_total = (treasury_pct as u64)
        .checked_add(burn_pct as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    require!(configured_pct_total <= M_HUNDRED, ErrorCode::InvalidAmount);

    let tax_config = &mut ctx.accounts.tax_config;
    tax_config.treasury_pct = treasury_pct;
    tax_config.burn_pct = burn_pct;

    msg!("   ✅ TaxConfig updated");
    msg!(
        "   Faction Treasury: {}%, Burn: {}%",
        treasury_pct,
        burn_pct
    );
    let vault_pct = M_HUNDRED as u8 - treasury_pct - burn_pct;
    msg!("   Back to Vault: {}%", vault_pct);

    Ok(())
}

// ========================================================================================
// ============================= TAX HARVESTING & DISTRIBUTION ===========================
// ========================================================================================

/// STEP 1: Harvest withheld fees from token accounts → mint.
///
/// A keeper bot discovers token accounts with withheld fees off-chain (via Helius
/// DAS API or getProgramAccounts) and passes them as `remaining_accounts`.
/// This CPI aggregates their fees into `degenbtc_mint.withheld_amount`.
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
        ctx.accounts.degenbtc_mint.key()
    );

    harvest_withheld_tokens_to_mint(
        CpiContext::new(
            ctx.accounts.token_program_2022.to_account_info(),
            HarvestWithheldTokensToMint {
                mint: ctx.accounts.degenbtc_mint.to_account_info(),
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
fn post_fee_amount<'info>(
    mint_account_info: &AccountInfo<'info>,
    pre_fee_amount: u64,
    epoch: u64,
) -> Result<u64> {
    if pre_fee_amount == 0 {
        return Ok(0);
    }
    Ok(
        helper::get_token2022_transfer_fee_info(mint_account_info, pre_fee_amount, epoch)?
            .post_fee_amount,
    )
}

fn proportional_post_fee_share(
    post_fee_total: u64,
    pre_fee_share: u64,
    pre_fee_total: u64,
) -> Result<u64> {
    if post_fee_total == 0 || pre_fee_share == 0 || pre_fee_total == 0 {
        return Ok(0);
    }
    u64::try_from(helper::mul_div(
        post_fee_total,
        pre_fee_share,
        pre_fee_total,
    )?)
    .map_err(|_| ErrorCode::ArithmeticOverflow.into())
}

fn require_canonical_faction_state(
    global_config: &GlobalConfig,
    faction_state_key: Pubkey,
    faction_id: usize,
) -> Result<()> {
    let faction_name = global_config
        .supported_factions
        .get(faction_id)
        .ok_or(ErrorCode::InvalidFactionId)?;
    let (expected_faction_state, _) = Pubkey::find_program_address(
        &[FACTION_STATE_SEED.as_ref(), faction_name.as_bytes()],
        &crate::id(),
    );
    require_keys_eq!(
        faction_state_key,
        expected_faction_state,
        ErrorCode::InvalidAccount
    );
    Ok(())
}

#[inline(never)]
pub fn internal_crank_distribute_tax<'info>(
    accounts: &mut CrankDistributeTax<'info>,
    war_id: u64,
    withdraw_authority_bump: u8,
) -> Result<()> {
    crate::log_fn!("tax", "internal_crank_distribute_tax");
    msg!("💰 [crank_distribute_tax] Withdrawing *total* tax from mint");

    // 1. Get the total amount of tax sitting on the mint account
    // We must reload to get the most up-to-date data after harvesting
    accounts.degenbtc_mint.reload()?;

    // Read withheld_amount from TransferFeeConfig extension.
    // Scoped block so the borrow is dropped before CPI calls below.
    let withheld_amount = {
        let mint_account_info = accounts.degenbtc_mint.to_account_info();
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
            mint: accounts.degenbtc_mint.to_account_info(),
            token_program_id: accounts.token_program_2022.to_account_info(),
        },
        withdraw_authority_signer,
    ))?;

    // 3. Calculate distribution amounts based on TaxConfig percentages.
    // Token-2022 transfer fees apply to the transfer into faction_treasury_vault,
    // so the war accounting below credits only the post-fee delivered amount.
    let tax_config = &mut accounts.tax_config;
    let clock = Clock::get()?;
    require!(
        accounts.war_config.current_war_id == war_id,
        ErrorCode::InvalidParameters
    );
    let faction_treasury_amount = u64::try_from(helper::mul_div(
        withheld_amount,
        tax_config.treasury_pct as u64,
        M_HUNDRED,
    )?)
    .map_err(|_| ErrorCode::ArithmeticOverflow)?;
    let burn_amount = u64::try_from(helper::mul_div(
        withheld_amount,
        tax_config.burn_pct as u64,
        M_HUNDRED,
    )?)
    .map_err(|_| ErrorCode::ArithmeticOverflow)?;
    // Remainder goes back to the degenBTC vault.
    let vault_return = withheld_amount
        .checked_sub(faction_treasury_amount)
        .and_then(|remaining| remaining.checked_sub(burn_amount))
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let faction_treasury_credit = post_fee_amount(
        &accounts.degenbtc_mint.to_account_info(),
        faction_treasury_amount,
        clock.epoch,
    )?;

    msg!("   Splitting {} tokens:", (withheld_amount as f64) / 1e6);
    msg!(
        "   - Faction Treasury: {} ({}%)",
        (faction_treasury_amount as f64) / 1e6,
        tax_config.treasury_pct
    );
    msg!(
        "   - Burn: {} ({}%)",
        (burn_amount as f64) / 1e6,
        tax_config.burn_pct
    );
    msg!("   - Back to Vault: {}", (vault_return as f64) / 1e6);

    // 4. Distribute the funds (all signed by the same PDA)

    // Transfer to faction treasury
    if faction_treasury_amount > 0 {
        token_2022::transfer_checked(
            CpiContext::new_with_signer(
                accounts.token_program_2022.to_account_info(),
                TransferChecked {
                    from: accounts.withdraw_authority_token_account.to_account_info(),
                    mint: accounts.degenbtc_mint.to_account_info(),
                    to: accounts.faction_treasury_vault.to_account_info(),
                    authority: accounts.withdraw_withheld_authority.to_account_info(),
                },
                withdraw_authority_signer,
            ),
            faction_treasury_amount,
            accounts.degenbtc_mint.decimals,
        )?;
        msg!(
            "   ✅ Sent {} tokens to Faction treasury vault (post-fee credit={})",
            (faction_treasury_amount as f64) / 1e6,
            (faction_treasury_credit as f64) / 1e6
        );
    }

    // Burn
    if burn_amount > 0 {
        token_2022::burn(
            CpiContext::new_with_signer(
                accounts.token_program_2022.to_account_info(),
                Burn {
                    mint: accounts.degenbtc_mint.to_account_info(),
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

    // Transfer remainder back to degenBTC vault
    if vault_return > 0 {
        token_2022::transfer_checked(
            CpiContext::new_with_signer(
                accounts.token_program_2022.to_account_info(),
                TransferChecked {
                    from: accounts.withdraw_authority_token_account.to_account_info(),
                    mint: accounts.degenbtc_mint.to_account_info(),
                    to: accounts.dbtc_token_vault.to_account_info(),
                    authority: accounts.withdraw_withheld_authority.to_account_info(),
                },
                withdraw_authority_signer,
            ),
            vault_return,
            accounts.degenbtc_mint.decimals,
        )?;
        msg!(
            "   ✅ Returned {} tokens to degenBTC vault",
            (vault_return as f64) / 1e6
        );
    }

    let mut credited_to_active_war = false;
    if faction_treasury_credit > 0 {
        let war_state = &mut accounts.war_state;
        if war_state.war_id == accounts.war_config.current_war_id && war_state.stage == 0 {
            war_state.treasury_reward_base_amount = war_state
                .treasury_reward_base_amount
                .checked_add(faction_treasury_credit)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            credited_to_active_war = true;
            msg!(
                "   🏴 Attributed {} delivered degenBTC treasury tax to faction war {} (base now {})",
                (faction_treasury_credit as f64) / 1e6,
                war_state.war_id,
                (war_state.treasury_reward_base_amount as f64) / 1e6
            );
        } else {
            tax_config.unassigned_war_treasury_amount = tax_config
                .unassigned_war_treasury_amount
                .checked_add(faction_treasury_credit)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!(
                "   💤 Faction war state not ready; queued {} delivered degenBTC treasury tax for the next war (queued total {})",
                (faction_treasury_credit as f64) / 1e6,
                (tax_config.unassigned_war_treasury_amount as f64) / 1e6
            );
        }
    }

    let total_burnt = tax_config.total_burnt;
    emit!(TaxDistributed {
        total_tax_amount: withheld_amount,
        faction_treasury_amount,
        faction_treasury_credit,
        burn_amount,
        vault_return_amount: vault_return,
        total_burnt,
        war_id,
        credited_to_active_war,
        unassigned_war_treasury_amount: tax_config.unassigned_war_treasury_amount,
        timestamp: clock.unix_timestamp,
    });

    msg!("✅ [crank_distribute_tax] Tax distribution complete");
    Ok(())
}

// ========================================================================================
// ============================= FACTION TREASURY DISTRIBUTION ============================
// ========================================================================================

/// Distribute faction treasury rewards for a settled faction_war.
///
/// Uses the faction_war's final ranks to distribute the cycle's credited
/// treasury amount:
/// - 80% rank-weighted: higher ranks get more, every faction gets a share.
/// - 20% lucky draw: one lower-ranked faction receives the underdog slice.
///
/// Called once per faction per faction_war. Each call transfers that faction's share
/// from the treasury vault to the emission vault and updates staker reward indexes.
///
/// Permissionless: anyone can crank this after a faction_war settles.
pub fn internal_claim_faction_treasury_for_faction_war(
    ctx: Context<ClaimFactionTreasuryForFactionWar>,
    war_id: u64,
) -> Result<()> {
    crate::log_fn!("tax", "internal_claim_faction_treasury_for_faction_war");
    let war_state = &ctx.accounts.war_state;
    let war_settlement = &mut ctx.accounts.war_settlement;
    let fs = &mut ctx.accounts.faction_state;
    let fid = fs.faction_id;

    msg!(
        "💰 [claim_faction_treasury] FactionWar #{}, faction {}, treasury={}",
        war_id,
        fid,
        ctx.accounts.faction_treasury_vault.amount
    );

    require!(war_state.stage == 1, ErrorCode::FactionWarNotSettled);
    require!(war_state.war_id == war_id, ErrorCode::InvalidState);

    let active_factions = war_state.faction_count as usize;
    require!(
        (fid as usize) < active_factions,
        ErrorCode::InvalidFactionId
    );
    require!(
        active_factions <= ctx.accounts.global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    require_canonical_faction_state(&ctx.accounts.global_config, fs.key(), fid as usize)?;
    require!(
        (fid as usize) < NUM_FACTIONS && (fid as u32) < u16::BITS,
        ErrorCode::InvalidFactionId
    );

    // Prevent double-claim for this faction.
    let faction_bit = 1u16
        .checked_shl(fid as u32)
        .ok_or(ErrorCode::InvalidFactionId)?;
    require!(
        war_settlement.treasury_claimed_bitmap & faction_bit == 0,
        ErrorCode::FactionWarRewardsAlreadyClaimed
    );

    let treasury_balance = ctx.accounts.faction_treasury_vault.amount;
    let treasury_base_amount = war_state.treasury_reward_base_amount;

    // Determine this faction's rank from faction_war final_ranks
    let rank = war_settlement.final_ranks[fid as usize] as usize;
    require!(rank < active_factions, ErrorCode::InvalidState);

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
        eligible_start + (war_id as usize % eligible_count)
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
    let pre_fee_reward_amount =
        u64::try_from(total_reward_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;

    msg!(
        "   Rank {}: base={}, current_vault={}, rank_reward={}, lucky_rank={}, lucky_reward={}, total={}",
        rank,
        treasury_base_amount,
        treasury_balance,
        rank_reward,
        lucky_rank,
        lucky_reward,
        pre_fee_reward_amount
    );

    if pre_fee_reward_amount == 0 {
        msg!("   ⚠️ No reward for faction {} (rank {})", fid, rank);
        war_settlement.treasury_claimed_bitmap |= faction_bit;
        return Ok(());
    }

    // Split 50/50 between degenBTC stakers and LP stakers
    let degenbtc_active = fs.total_degenbtc_hashpower > 0;
    let lp_active = fs.total_lp_hashpower > 0;
    let (pre_fee_dbtc_share, pre_fee_lp_share) = match (degenbtc_active, lp_active) {
        (true, true) => {
            let half = pre_fee_reward_amount / 2;
            (half, pre_fee_reward_amount - half)
        }
        (true, false) => (pre_fee_reward_amount, 0),
        (false, true) => (0, pre_fee_reward_amount),
        (false, false) => (0, 0),
    };
    let pre_fee_distributed = pre_fee_dbtc_share
        .checked_add(pre_fee_lp_share)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let pre_fee_reborn_amount = if degenbtc_active || lp_active {
        0
    } else {
        pre_fee_reward_amount
    };
    let total_transfer = pre_fee_distributed
        .checked_add(pre_fee_reborn_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let post_fee_transfer = post_fee_amount(
        &ctx.accounts.degenbtc_mint.to_account_info(),
        total_transfer,
        Clock::get()?.epoch,
    )?;
    let dbtc_share =
        proportional_post_fee_share(post_fee_transfer, pre_fee_dbtc_share, total_transfer)?;
    let lp_share =
        proportional_post_fee_share(post_fee_transfer, pre_fee_lp_share, total_transfer)?;
    let reborn_amount = post_fee_transfer
        .checked_sub(dbtc_share)
        .and_then(|remaining| remaining.checked_sub(lp_share))
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let reward_amount = dbtc_share
        .checked_add(lp_share)
        .and_then(|distributed| distributed.checked_add(reborn_amount))
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
                    mint: ctx.accounts.degenbtc_mint.to_account_info(),
                    to: ctx.accounts.dbtc_emission_vault.to_account_info(),
                    authority: ctx.accounts.withdraw_withheld_authority.to_account_info(),
                },
                seeds,
            ),
            total_transfer,
            ctx.accounts.degenbtc_mint.decimals,
        )?;
        msg!(
            "   ✅ Sent {} degenBTC from treasury; post-fee credited {} (dbtc_stakers={}, lp_stakers={}, reborn={})",
            total_transfer,
            reward_amount,
            dbtc_share,
            lp_share,
            reborn_amount
        );
    }

    if dbtc_share > 0 && fs.total_degenbtc_hashpower > 0 {
        fs.degenbtc_degenbtc_reward_index = fs
            .degenbtc_degenbtc_reward_index
            .checked_add(helper::mul_div(
                dbtc_share,
                INDEX_PRECISION,
                fs.total_degenbtc_hashpower,
            )?)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    if lp_share > 0 && fs.total_lp_hashpower > 0 {
        fs.lp_degenbtc_reward_index = fs
            .lp_degenbtc_reward_index
            .checked_add(helper::mul_div(
                lp_share,
                INDEX_PRECISION,
                fs.total_lp_hashpower,
            )?)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    war_settlement.treasury_claimed_bitmap |= faction_bit;

    emit!(FactionTreasuryRewardsClaimed {
        war_id,
        faction_id: fid,
        rank: rank as u8,
        reward_amount,
        dbtc_share,
        lp_share,
        reborn_amount,
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

    #[account(init, payer = authority, token::mint = degenbtc_mint, token::authority = withdraw_withheld_authority, token::token_program = token_program_2022, seeds = [FACTION_TREASURY_VAULT_SEED.as_ref()], bump)]
    pub faction_treasury_vault: InterfaceAccount<'info, TokenAccount2022>,

    pub degenbtc_mint: InterfaceAccount<'info, Mint>,

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
pub struct CrankHarvestFees<'info> {
    #[account(mut)]
    pub degenbtc_mint: InterfaceAccount<'info, Mint>,
    pub token_program_2022: Program<'info, anchor_spl::token_2022::Token2022>,
}

#[derive(Accounts)]
#[instruction(war_id: u64)]
pub struct CrankDistributeTax<'info> {
    #[account(mut)]
    pub degenbtc_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Withdraw withheld authority PDA
    #[account(seeds = [WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref()], bump)]
    pub withdraw_withheld_authority: AccountInfo<'info>,

    #[account(
        mut,
        constraint = withdraw_authority_token_account.owner == withdraw_withheld_authority.key() @ ErrorCode::Unauthorized,
        constraint = withdraw_authority_token_account.mint == degenbtc_mint.key() @ ErrorCode::InvalidMint,
    )]
    pub withdraw_authority_token_account: Box<InterfaceAccount<'info, TokenAccount2022>>,

    #[account(
        mut,
        constraint = faction_treasury_vault.key() == tax_config.faction_treasury_vault @ ErrorCode::InvalidAccount,
        constraint = faction_treasury_vault.mint == degenbtc_mint.key() @ ErrorCode::InvalidMint,
    )]
    pub faction_treasury_vault: Box<InterfaceAccount<'info, TokenAccount2022>>,

    #[account(seeds = [MINE_BTC_MINING_SEED.as_ref()], bump = dbtc_mining.bump)]
    pub dbtc_mining: Box<Account<'info, DegenBtcMining>>,

    /// CHECK: Canonical degenBTC vault authority PDA.
    #[account(
        seeds = [DEGEN_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump = dbtc_mining.vault_auth_bump,
    )]
    pub vault_authority: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [DEGEN_BTC_VAULT_SEED.as_ref(), dbtc_mining.key().as_ref()],
        bump,
        constraint = dbtc_token_vault.key() == dbtc_mining.dbtc_token_vault @ ErrorCode::InvalidAccount,
        constraint = dbtc_token_vault.mint == degenbtc_mint.key() @ ErrorCode::InvalidMint,
        constraint = dbtc_token_vault.owner == vault_authority.key() @ ErrorCode::Unauthorized,
    )]
    pub dbtc_token_vault: Box<InterfaceAccount<'info, TokenAccount2022>>,

    #[account(mut, seeds = [TAX_CONFIG_SEED.as_ref()], bump = tax_config.bump)]
    pub tax_config: Box<Account<'info, TaxConfig>>,

    #[account(
        seeds = [FACTION_WAR_CONFIG_SEED],
        bump = war_config.bump
    )]
    pub war_config: Box<Account<'info, FactionWarConfig>>,
    #[account(
        mut,
        seeds = [FACTION_WAR_STATE_SEED, &war_id.to_le_bytes()],
        bump = war_state.bump,
    )]
    pub war_state: Box<Account<'info, FactionWarState>>,

    #[account(mut)]
    pub caller: Signer<'info>,

    pub token_program_2022: Program<'info, anchor_spl::token_2022::Token2022>,
    pub system_program: Program<'info, System>,
}

/// Claim faction treasury rewards for a settled faction_war.
/// Uses gameplay-score leaderboard (faction_war final_ranks) -- no separate leaderboard needed.
#[derive(Accounts)]
#[instruction(war_id: u64)]
pub struct ClaimFactionTreasuryForFactionWar<'info> {
    #[account(mut, seeds = [TAX_CONFIG_SEED.as_ref()], bump = tax_config.bump)]
    pub tax_config: Box<Account<'info, TaxConfig>>,
    #[account(
        seeds = [FACTION_WAR_STATE_SEED, &war_id.to_le_bytes()],
        bump = war_state.bump
    )]
    pub war_state: Box<Account<'info, FactionWarState>>,
    #[account(
        mut,
        seeds = [FACTION_WAR_SETTLEMENT_SEED, &war_id.to_le_bytes()],
        bump = war_settlement.bump
    )]
    pub war_settlement: Box<Account<'info, FactionWarSettlement>>,

    #[account(mut)]
    pub faction_state: Box<Account<'info, FactionState>>,

    #[account(seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    pub degenbtc_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        constraint = faction_treasury_vault.key() == tax_config.faction_treasury_vault @ ErrorCode::InvalidAccount,
        constraint = faction_treasury_vault.mint == degenbtc_mint.key() @ ErrorCode::InvalidMint,
    )]
    pub faction_treasury_vault: Box<InterfaceAccount<'info, TokenAccount2022>>,

    #[account(seeds = [MINE_BTC_MINING_SEED.as_ref()], bump = dbtc_mining.bump)]
    pub dbtc_mining: Box<Account<'info, DegenBtcMining>>,

    /// CHECK: Canonical degenBTC vault authority PDA.
    #[account(
        seeds = [DEGEN_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump = dbtc_mining.vault_auth_bump,
    )]
    pub vault_authority: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [DEGEN_BTC_VAULT_SEED.as_ref(), dbtc_mining.key().as_ref()],
        bump,
        constraint = dbtc_emission_vault.key() == dbtc_mining.dbtc_token_vault @ ErrorCode::InvalidAccount,
        constraint = dbtc_emission_vault.mint == degenbtc_mint.key() @ ErrorCode::InvalidMint,
        constraint = dbtc_emission_vault.owner == vault_authority.key() @ ErrorCode::Unauthorized,
    )]
    pub dbtc_emission_vault: Box<InterfaceAccount<'info, TokenAccount2022>>,

    /// CHECK: PDA signer for treasury vault transfers
    #[account(seeds = [WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref()], bump)]
    pub withdraw_withheld_authority: AccountInfo<'info>,

    pub token_program_2022: Program<'info, anchor_spl::token_2022::Token2022>,
}
