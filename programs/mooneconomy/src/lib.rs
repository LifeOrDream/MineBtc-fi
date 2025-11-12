use anchor_lang::prelude::*;

mod errors;
mod events;
pub mod instructions;
mod state;

use instructions::admin::*;
use instructions::user::*;

declare_id!("AXDoen9KXArDwmvvxGcHqScrYrUdqc2ByqV4eYRGdGZA");

#[program]
pub mod mooneconomy {
    use super::*;
    use crate::instructions::admin;

    // ----------------------------------------------------------------------------------------
    // ------------ INITIALIZE GLOBAL CONFIG ------------
    // ----------------------------------------------------------------------------------------
    pub fn initialize_global_config(
        ctx: Context<InitializeGlobalConfig>,
        dev_address: Pubkey,
        dogebtc_allocation: u8,
        liquidity_allocation: u8,
        min_lockup_days: u64,
        max_lockup_days: u64,
        base_multiplier: u16,
        max_multiplier: u16,
    ) -> Result<()> {
        admin::initialize_global_config(
            ctx,
            dev_address,
            dogebtc_allocation,
            liquidity_allocation,
            min_lockup_days,
            max_lockup_days,
            base_multiplier,
            max_multiplier,
        )
    }

    pub fn update_configuration(
        ctx: Context<UpdateConfig>,
        new_authority: Option<Pubkey>,
        new_dev_address: Option<Pubkey>,
        new_dogebtc_allocation: Option<u8>,
        new_liquidity_allocation: Option<u8>,
        new_electricity_per_weighted_sol: Option<u64>,
        new_emergency_tax: Option<u8>,
    ) -> Result<()> {
        admin::internal_update_configuration(
            ctx,
            new_authority,
            new_dev_address,
            new_dogebtc_allocation,
            new_liquidity_allocation,
            new_electricity_per_weighted_sol,
            new_emergency_tax,
        )
    }

    /// Set DOGE_BTC to SOL price (admin only)
    /// Price is stored with 9-decimal precision (same as MoonBase)
    pub fn set_dbtc_sol_price(ctx: Context<UpdateConfig>, price: u64) -> Result<()> {
        admin::set_dbtc_sol_price_internal(ctx, price)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ INITIALIZE VAULTS ------------
    // ----------------------------------------------------------------------------------------

    pub fn initialize_dbtc_vault(
        ctx: Context<InitializeDbtcVault>,
        dogebtc_mint: Pubkey,
    ) -> Result<()> {
        admin::initialize_dbtc_vault(ctx, dogebtc_mint)
    }

    pub fn initialize_liquidity_vault(
        ctx: Context<InitializeLiquidityVault>,
        lp_token_mint: Pubkey,
    ) -> Result<()> {
        admin::initialize_liquidity_vault(ctx, lp_token_mint)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ WITHDRAW COLLECTED SOL FEES (admin can only call this) ------------
    // ----------------------------------------------------------------------------------------

    pub fn withdraw_dev_earnings(ctx: Context<WithdrawDevEarnings>) -> Result<()> {
        admin::withdraw_dev_earnings(ctx)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ DISTRIBUTE SOL FEES FROM MOONFACILITY TO RESPECTIVE VAULTS ------------
    // ----------------------------------------------------------------------------------------

    pub fn claim_moonbase_sol(ctx: Context<ClaimMoonBaseSOL>) -> Result<()> {
        instructions::admin::internal_claim_moonbase_sol(ctx)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ ENABLE/DISABLE SOL DISTRIBUTION (admin only) ------------
    // ----------------------------------------------------------------------------------------

    pub fn set_sol_distribution_enabled(
        ctx: Context<SetSolDistributionEnabled>,
        enabled: bool,
    ) -> Result<()> {
        instructions::admin::set_sol_distribution_enabled(ctx, enabled)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ USER INSTRUCTIONS :: STAKE & UNSTAKE MOONDOGE / LP TOKENs  ------------
    // ----------------------------------------------------------------------------------------

    pub fn initialize_electricity_account(ctx: Context<InitializeElectricityAc>) -> Result<()> {
        instructions::user::initialize_electricity_account(ctx)
    }

    pub fn stake_moondoge(
        ctx: Context<StakeDogeBtc>,
        amount: u64,
        lockup_duration: u64,
        lockup_index: u8,
    ) -> Result<()> {
        instructions::user::stake_moondoge(ctx, amount, lockup_duration, lockup_index)
    }

    pub fn unstake_moondoge(ctx: Context<UnstakeDogeBtc>, position_index: u8) -> Result<()> {
        instructions::user::unstake_moondoge(ctx, position_index)
    }

    pub fn stake_lp_tokens(
        ctx: Context<StakeLpTokens>,
        amount: u64,
        lockup_duration: u64,
        lockup_index: u8,
    ) -> Result<()> {
        instructions::user::stake_lp_tokens(ctx, amount, lockup_duration, lockup_index)
    }

    pub fn unstake_lp_tokens(ctx: Context<UnstakeLpTokens>, lockup_index: u8) -> Result<()> {
        instructions::user::unstake_lp_tokens(ctx, lockup_index)
    }

    pub fn claim_sol_rewards(ctx: Context<ClaimSolRewards>) -> Result<()> {
        instructions::user::claim_sol_rewards(ctx)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ CPI WRAPPERS :: MOONBASE FUNCTIONS WITH ELECTRICITY UPDATES --------------
    // ----------------------------------------------------------------------------------------

    /// Claim DOGE_BTC mining rewards (calculates electricity and calls MoonBase via CPI)
    pub fn claim_dbtc_tokens(ctx: Context<ClaimDbtcTokens>) -> Result<()> {
        instructions::user::claim_dbtc_tokens_wrapper(ctx)
    }

    /// Claim referral rewards (calculates electricity and calls MoonBase via CPI)
    pub fn claim_referral_rewards(ctx: Context<ClaimReferralRewardsWrapper>) -> Result<()> {
        instructions::user::claim_referral_rewards_wrapper(ctx)
    }

    /// Claim attraction XP (calculates electricity and calls MoonBase via CPI)
    pub fn claim_attraction_xp(
        ctx: Context<ClaimAttractionXpWrapper>,
        module_index: u8,
    ) -> Result<()> {
        instructions::user::claim_attraction_xp_wrapper(ctx, module_index)
    }
}
