use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use anchor_spl::associated_token::AssociatedToken;

pub mod state;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod constants;
pub mod math;

use instructions::*;
use state::*;
use errors::*;

declare_id!("TokenLaunchpadProgram11111111111111111111");

#[program]
pub mod token_launchpad {
    use super::*;

    /// Initialize the global launchpad configuration
    pub fn initialize_global_config(
        ctx: Context<InitializeGlobalConfig>,
        fee_recipient: Pubkey,
        platform_fee_bps: u16,
        token_creation_fee: u64,
        migration_fee_bps: u16,
    ) -> Result<()> {
        instructions::initialize_global_config(
            ctx,
            fee_recipient,
            platform_fee_bps,
            token_creation_fee,
            migration_fee_bps,
        )
    }

    /// Update global configuration (admin only)
    pub fn update_global_config(
        ctx: Context<UpdateGlobalConfig>,
        new_authority: Option<Pubkey>,
        new_fee_recipient: Option<Pubkey>,
        new_platform_fee_bps: Option<u16>,
        new_token_creation_fee: Option<u64>,
        new_migration_fee_bps: Option<u16>,
    ) -> Result<()> {
        instructions::update_global_config(
            ctx,
            new_authority,
            new_fee_recipient,
            new_platform_fee_bps,
            new_token_creation_fee,
            new_migration_fee_bps,
        )
    }

    /// Create a new token with bonding curve
    pub fn create_token(
        ctx: Context<CreateToken>,
        name: String,
        symbol: String,
        uri: String,
        initial_virtual_sol_reserves: u64,
        initial_virtual_token_reserves: u64,
        initial_real_token_reserves: u64,
    ) -> Result<()> {
        instructions::create_token(
            ctx,
            name,
            symbol,
            uri,
            initial_virtual_sol_reserves,
            initial_virtual_token_reserves,
            initial_real_token_reserves,
        )
    }

    /// Buy tokens using the bonding curve
    pub fn buy(
        ctx: Context<Buy>,
        sol_amount: u64,
        min_tokens_out: u64,
    ) -> Result<()> {
        instructions::buy(ctx, sol_amount, min_tokens_out)
    }

    /// Sell tokens using the bonding curve
    pub fn sell(
        ctx: Context<Sell>,
        token_amount: u64,
        min_sol_out: u64,
    ) -> Result<()> {
        instructions::sell(ctx, token_amount, min_sol_out)
    }

    /// Migrate liquidity to AMM when bonding curve completes
    pub fn migrate_to_amm(
        ctx: Context<MigrateToAmm>,
        weight_token: u64,
        weight_sol: u64,
    ) -> Result<()> {
        instructions::migrate_to_amm(ctx, weight_token, weight_sol)
    }

    /// Withdraw platform fees (admin only)
    pub fn withdraw_fees(ctx: Context<WithdrawFees>) -> Result<()> {
        instructions::withdraw_fees(ctx)
    }
}
