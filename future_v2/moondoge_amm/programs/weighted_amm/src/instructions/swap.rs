use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::state::*;
use crate::constants::*;
use crate::errors::*;
use crate::events::*;
use crate::math::*;

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(
        seeds = [AMM_CONFIG_SEED, &amm_config.index.to_le_bytes()],
        bump = amm_config.bump
    )]
    pub amm_config: Account<'info, AmmConfig>,

    #[account(
        mut,
        seeds = [
            POOL_SEED,
            amm_config.key().as_ref(),
            pool_state.token_0_mint.as_ref(),
            pool_state.token_1_mint.as_ref()
        ],
        bump = pool_state.bump,
        constraint = pool_state.is_pool_enabled() @ AmmError::PoolDisabled,
        constraint = pool_state.is_pool_open() @ AmmError::PoolNotOpen
    )]
    pub pool_state: Account<'info, PoolState>,

    /// CHECK: This account is the authority for the pool vaults
    #[account(
        seeds = [POOL_AUTH_SEED],
        bump
    )]
    pub pool_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        address = pool_state.token_0_vault
    )]
    pub token_0_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        address = pool_state.token_1_vault
    )]
    pub token_1_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::authority = user,
    )]
    pub user_token_in_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::authority = user,
    )]
    pub user_token_out_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn swap_exact_input(
    ctx: Context<Swap>,
    amount_in: u64,
    minimum_amount_out: u64,
) -> Result<()> {
    require!(amount_in > 0, AmmError::InvalidTokenAmount);

    let pool_state = &mut ctx.accounts.pool_state;
    let amm_config = &ctx.accounts.amm_config;

    // Determine swap direction
    let (
        vault_in,
        vault_out,
        weight_in,
        weight_out,
        is_token_0_in,
    ) = if ctx.accounts.user_token_in_account.mint == pool_state.token_0_mint {
        (
            &ctx.accounts.token_0_vault,
            &ctx.accounts.token_1_vault,
            pool_state.token_0_weight,
            pool_state.token_1_weight,
            true,
        )
    } else if ctx.accounts.user_token_in_account.mint == pool_state.token_1_mint {
        (
            &ctx.accounts.token_1_vault,
            &ctx.accounts.token_0_vault,
            pool_state.token_1_weight,
            pool_state.token_0_weight,
            false,
        )
    } else {
        return Err(AmmError::InvalidTokenAmount.into());
    };

    // Verify output token account
    let expected_out_mint = if is_token_0_in {
        pool_state.token_1_mint
    } else {
        pool_state.token_0_mint
    };
    require!(
        ctx.accounts.user_token_out_account.mint == expected_out_mint,
        AmmError::InvalidTokenAmount
    );

    // Calculate trade fee
    let trade_fee = calculate_fee(amount_in as u128, amm_config.trade_fee_rate)? as u64;
    let amount_in_after_fee = amount_in
        .checked_sub(trade_fee)
        .ok_or(AmmError::MathOverflow)?;

    // Calculate amount out using weighted math
    let amount_out = calculate_out_given_in(
        vault_in.amount as u128,
        weight_in as u128,
        vault_out.amount as u128,
        weight_out as u128,
        amount_in_after_fee as u128,
    )? as u64;

    require!(
        amount_out >= minimum_amount_out,
        AmmError::SlippageExceeded
    );

    require!(
        amount_out <= vault_out.amount,
        AmmError::InsufficientLiquidity
    );

    // Transfer tokens from user to pool
    let transfer_in_accounts = Transfer {
        from: ctx.accounts.user_token_in_account.to_account_info(),
        to: vault_in.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            transfer_in_accounts,
        ),
        amount_in,
    )?;

    // Transfer tokens from pool to user
    let auth_seeds = &[POOL_AUTH_SEED, &[ctx.bumps.pool_authority]];
    let signer_seeds = &[&auth_seeds[..]];

    let transfer_out_accounts = Transfer {
        from: vault_out.to_account_info(),
        to: ctx.accounts.user_token_out_account.to_account_info(),
        authority: ctx.accounts.pool_authority.to_account_info(),
    };
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            transfer_out_accounts,
            signer_seeds,
        ),
        amount_out,
    )?;

    // Update protocol and fund fees
    let protocol_fee = calculate_fee(trade_fee as u128, amm_config.protocol_fee_rate)? as u64;
    let fund_fee = calculate_fee(trade_fee as u128, amm_config.fund_fee_rate)? as u64;

    if is_token_0_in {
        pool_state.token_0_protocol_fee = pool_state
            .token_0_protocol_fee
            .checked_add(protocol_fee)
            .ok_or(AmmError::MathOverflow)?;
        pool_state.token_0_fund_fee = pool_state
            .token_0_fund_fee
            .checked_add(fund_fee)
            .ok_or(AmmError::MathOverflow)?;
    } else {
        pool_state.token_1_protocol_fee = pool_state
            .token_1_protocol_fee
            .checked_add(protocol_fee)
            .ok_or(AmmError::MathOverflow)?;
        pool_state.token_1_fund_fee = pool_state
            .token_1_fund_fee
            .checked_add(fund_fee)
            .ok_or(AmmError::MathOverflow)?;
    }

    emit!(Swapped {
        pool_state: pool_state.key(),
        user: ctx.accounts.user.key(),
        token_in_mint: ctx.accounts.user_token_in_account.mint,
        token_out_mint: ctx.accounts.user_token_out_account.mint,
        amount_in,
        amount_out,
        fee_amount: trade_fee,
    });

    msg!(
        "Swap executed: user={}, amount_in={}, amount_out={}, fee={}",
        ctx.accounts.user.key(),
        amount_in,
        amount_out,
        trade_fee
    );

    Ok(())
}

pub fn swap_exact_output(
    ctx: Context<Swap>,
    max_amount_in: u64,
    amount_out: u64,
) -> Result<()> {
    require!(amount_out > 0, AmmError::InvalidTokenAmount);

    let pool_state = &mut ctx.accounts.pool_state;
    let amm_config = &ctx.accounts.amm_config;

    // Determine swap direction
    let (
        vault_in,
        vault_out,
        weight_in,
        weight_out,
        is_token_0_in,
    ) = if ctx.accounts.user_token_in_account.mint == pool_state.token_0_mint {
        (
            &ctx.accounts.token_0_vault,
            &ctx.accounts.token_1_vault,
            pool_state.token_0_weight,
            pool_state.token_1_weight,
            true,
        )
    } else if ctx.accounts.user_token_in_account.mint == pool_state.token_1_mint {
        (
            &ctx.accounts.token_1_vault,
            &ctx.accounts.token_0_vault,
            pool_state.token_1_weight,
            pool_state.token_0_weight,
            false,
        )
    } else {
        return Err(AmmError::InvalidTokenAmount.into());
    };

    // Verify output token account
    let expected_out_mint = if is_token_0_in {
        pool_state.token_1_mint
    } else {
        pool_state.token_0_mint
    };
    require!(
        ctx.accounts.user_token_out_account.mint == expected_out_mint,
        AmmError::InvalidTokenAmount
    );

    require!(
        amount_out <= vault_out.amount,
        AmmError::InsufficientLiquidity
    );

    // Calculate amount in using weighted math
    let amount_in_before_fee = calculate_in_given_out(
        vault_in.amount as u128,
        weight_in as u128,
        vault_out.amount as u128,
        weight_out as u128,
        amount_out as u128,
    )? as u64;

    // Add trade fee
    let trade_fee = calculate_fee(amount_in_before_fee as u128, amm_config.trade_fee_rate)? as u64;
    let amount_in = amount_in_before_fee
        .checked_add(trade_fee)
        .ok_or(AmmError::MathOverflow)?;

    require!(
        amount_in <= max_amount_in,
        AmmError::SlippageExceeded
    );

    // Transfer tokens from user to pool
    let transfer_in_accounts = Transfer {
        from: ctx.accounts.user_token_in_account.to_account_info(),
        to: vault_in.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            transfer_in_accounts,
        ),
        amount_in,
    )?;

    // Transfer tokens from pool to user
    let auth_seeds = &[POOL_AUTH_SEED, &[ctx.bumps.pool_authority]];
    let signer_seeds = &[&auth_seeds[..]];

    let transfer_out_accounts = Transfer {
        from: vault_out.to_account_info(),
        to: ctx.accounts.user_token_out_account.to_account_info(),
        authority: ctx.accounts.pool_authority.to_account_info(),
    };
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            transfer_out_accounts,
            signer_seeds,
        ),
        amount_out,
    )?;

    // Update protocol and fund fees
    let protocol_fee = calculate_fee(trade_fee as u128, amm_config.protocol_fee_rate)? as u64;
    let fund_fee = calculate_fee(trade_fee as u128, amm_config.fund_fee_rate)? as u64;

    if is_token_0_in {
        pool_state.token_0_protocol_fee = pool_state
            .token_0_protocol_fee
            .checked_add(protocol_fee)
            .ok_or(AmmError::MathOverflow)?;
        pool_state.token_0_fund_fee = pool_state
            .token_0_fund_fee
            .checked_add(fund_fee)
            .ok_or(AmmError::MathOverflow)?;
    } else {
        pool_state.token_1_protocol_fee = pool_state
            .token_1_protocol_fee
            .checked_add(protocol_fee)
            .ok_or(AmmError::MathOverflow)?;
        pool_state.token_1_fund_fee = pool_state
            .token_1_fund_fee
            .checked_add(fund_fee)
            .ok_or(AmmError::MathOverflow)?;
    }

    emit!(Swapped {
        pool_state: pool_state.key(),
        user: ctx.accounts.user.key(),
        token_in_mint: ctx.accounts.user_token_in_account.mint,
        token_out_mint: ctx.accounts.user_token_out_account.mint,
        amount_in,
        amount_out,
        fee_amount: trade_fee,
    });

    msg!(
        "Swap executed: user={}, amount_in={}, amount_out={}, fee={}",
        ctx.accounts.user.key(),
        amount_in,
        amount_out,
        trade_fee
    );

    Ok(())
}
