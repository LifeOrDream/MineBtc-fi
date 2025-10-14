use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::state::*;
use crate::constants::*;
use crate::errors::*;
use crate::events::*;
use crate::math::*;

#[derive(Accounts)]
pub struct Sell<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [BONDING_CURVE_SEED, mint.key().as_ref()],
        bump = bonding_curve.bump,
        constraint = !bonding_curve.complete @ LaunchpadError::BondingCurveComplete
    )]
    pub bonding_curve: Account<'info, BondingCurve>,

    pub mint: Account<'info, token::Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = bonding_curve,
    )]
    pub curve_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = seller,
    )]
    pub seller_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub seller: Signer<'info>,

    #[account(
        mut,
        address = global_config.fee_recipient
    )]
    pub fee_recipient: SystemAccount<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn sell(
    ctx: Context<Sell>,
    token_amount: u64,
    min_sol_out: u64,
) -> Result<()> {
    require!(
        token_amount >= MIN_TOKEN_AMOUNT,
        LaunchpadError::InsufficientTokenAmount
    );

    let global_config = &ctx.accounts.global_config;
    let bonding_curve = &mut ctx.accounts.bonding_curve;
    let seller = &ctx.accounts.seller;

    // Check seller has enough tokens
    require!(
        ctx.accounts.seller_token_account.amount >= token_amount,
        LaunchpadError::InsufficientTokenAmount
    );

    // Calculate SOL to receive
    let sol_out = calculate_sol_out(
        token_amount,
        bonding_curve.virtual_sol_reserves,
        bonding_curve.virtual_token_reserves,
    )?;

    // Calculate platform fee
    let platform_fee = calculate_fee(sol_out, global_config.platform_fee_bps)?;
    let sol_out_after_fee = sol_out
        .checked_sub(platform_fee)
        .ok_or(LaunchpadError::MathOverflow)?;

    require!(
        sol_out_after_fee >= min_sol_out,
        LaunchpadError::SlippageExceeded
    );

    require!(
        bonding_curve.real_sol_reserves >= sol_out,
        LaunchpadError::InsufficientSolAmount
    );

    // Transfer tokens from seller to curve
    let transfer_accounts = Transfer {
        from: ctx.accounts.seller_token_account.to_account_info(),
        to: ctx.accounts.curve_token_account.to_account_info(),
        authority: seller.to_account_info(),
    };
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            transfer_accounts,
        ),
        token_amount,
    )?;

    // Transfer SOL from curve to seller
    **bonding_curve.to_account_info().try_borrow_mut_lamports()? = bonding_curve
        .to_account_info()
        .lamports()
        .checked_sub(sol_out_after_fee)
        .ok_or(LaunchpadError::MathOverflow)?;
    
    **seller.to_account_info().try_borrow_mut_lamports()? = seller
        .to_account_info()
        .lamports()
        .checked_add(sol_out_after_fee)
        .ok_or(LaunchpadError::MathOverflow)?;

    // Transfer platform fee to fee recipient
    if platform_fee > 0 {
        **bonding_curve.to_account_info().try_borrow_mut_lamports()? = bonding_curve
            .to_account_info()
            .lamports()
            .checked_sub(platform_fee)
            .ok_or(LaunchpadError::MathOverflow)?;
        
        **ctx.accounts.fee_recipient.to_account_info().try_borrow_mut_lamports()? = ctx
            .accounts
            .fee_recipient
            .to_account_info()
            .lamports()
            .checked_add(platform_fee)
            .ok_or(LaunchpadError::MathOverflow)?;
    }

    // Update bonding curve state
    bonding_curve.virtual_sol_reserves = bonding_curve
        .virtual_sol_reserves
        .checked_sub(sol_out)
        .ok_or(LaunchpadError::MathOverflow)?;
    
    bonding_curve.virtual_token_reserves = bonding_curve
        .virtual_token_reserves
        .checked_add(token_amount)
        .ok_or(LaunchpadError::MathOverflow)?;
    
    bonding_curve.real_sol_reserves = bonding_curve
        .real_sol_reserves
        .checked_sub(sol_out)
        .ok_or(LaunchpadError::MathOverflow)?;
    
    bonding_curve.real_token_reserves = bonding_curve
        .real_token_reserves
        .checked_add(token_amount)
        .ok_or(LaunchpadError::MathOverflow)?;

    emit!(TokenSold {
        mint: ctx.accounts.mint.key(),
        seller: seller.key(),
        token_amount,
        sol_amount: sol_out,
        virtual_sol_reserves: bonding_curve.virtual_sol_reserves,
        virtual_token_reserves: bonding_curve.virtual_token_reserves,
        real_sol_reserves: bonding_curve.real_sol_reserves,
        real_token_reserves: bonding_curve.real_token_reserves,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Sell executed: seller={}, token_amount={}, sol_out={}, platform_fee={}",
        seller.key(),
        token_amount,
        sol_out,
        platform_fee
    );

    Ok(())
}
