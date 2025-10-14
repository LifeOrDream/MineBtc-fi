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

declare_id!("WeightedAmmProgram11111111111111111111111");

#[program]
pub mod weighted_amm {
    use super::*;

    /// Initialize the AMM configuration
    pub fn initialize_amm_config(
        ctx: Context<InitializeAmmConfig>,
        index: u16,
        trade_fee_rate: u64,
        protocol_fee_rate: u64,
        fund_fee_rate: u64,
        create_pool_fee: u64,
    ) -> Result<()> {
        instructions::initialize_amm_config(
            ctx,
            index,
            trade_fee_rate,
            protocol_fee_rate,
            fund_fee_rate,
            create_pool_fee,
        )
    }

    /// Update AMM configuration (admin only)
    pub fn update_amm_config(
        ctx: Context<UpdateAmmConfig>,
        param: u8,
        value: u64,
    ) -> Result<()> {
        instructions::update_amm_config(ctx, param, value)
    }

    /// Create a new weighted pool
    pub fn create_pool(
        ctx: Context<CreatePool>,
        token_0_weight: u64,
        token_1_weight: u64,
        init_amount_0: u64,
        init_amount_1: u64,
        open_time: u64,
    ) -> Result<()> {
        instructions::create_pool(
            ctx,
            token_0_weight,
            token_1_weight,
            init_amount_0,
            init_amount_1,
            open_time,
        )
    }

    /// Add liquidity to a pool
    pub fn deposit(
        ctx: Context<Deposit>,
        lp_token_amount: u64,
        maximum_token_0_amount: u64,
        maximum_token_1_amount: u64,
    ) -> Result<()> {
        instructions::deposit(
            ctx,
            lp_token_amount,
            maximum_token_0_amount,
            maximum_token_1_amount,
        )
    }

    /// Remove liquidity from a pool
    pub fn withdraw(
        ctx: Context<Withdraw>,
        lp_token_amount: u64,
        minimum_token_0_amount: u64,
        minimum_token_1_amount: u64,
    ) -> Result<()> {
        instructions::withdraw(
            ctx,
            lp_token_amount,
            minimum_token_0_amount,
            minimum_token_1_amount,
        )
    }

    /// Swap tokens (exact input)
    pub fn swap_exact_input(
        ctx: Context<Swap>,
        amount_in: u64,
        minimum_amount_out: u64,
    ) -> Result<()> {
        instructions::swap_exact_input(ctx, amount_in, minimum_amount_out)
    }

    /// Swap tokens (exact output)
    pub fn swap_exact_output(
        ctx: Context<Swap>,
        max_amount_in: u64,
        amount_out: u64,
    ) -> Result<()> {
        instructions::swap_exact_output(ctx, max_amount_in, amount_out)
    }

    /// Collect protocol fees (admin only)
    pub fn collect_protocol_fee(
        ctx: Context<CollectProtocolFee>,
        amount_0_requested: u64,
        amount_1_requested: u64,
    ) -> Result<()> {
        instructions::collect_protocol_fee(ctx, amount_0_requested, amount_1_requested)
    }

    /// Collect fund fees (admin only)
    pub fn collect_fund_fee(
        ctx: Context<CollectFundFee>,
        amount_0_requested: u64,
        amount_1_requested: u64,
    ) -> Result<()> {
        instructions::collect_fund_fee(ctx, amount_0_requested, amount_1_requested)
    }

    /// Update pool status (admin only)
    pub fn update_pool_status(
        ctx: Context<UpdatePoolStatus>,
        status: u8,
    ) -> Result<()> {
        instructions::update_pool_status(ctx, status)
    }
}
