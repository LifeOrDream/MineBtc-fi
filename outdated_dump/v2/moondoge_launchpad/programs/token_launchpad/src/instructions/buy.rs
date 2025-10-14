use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use anchor_spl::associated_token::AssociatedToken;

use crate::state::*;
use crate::constants::*;
use crate::errors::*;
use crate::events::*;
use crate::math::*;

#[derive(Accounts)]
pub struct Buy<'info> {
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
        init_if_needed,
        payer = buyer,
        associated_token::mint = mint,
        associated_token::authority = buyer,
    )]
    pub buyer_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        mut,
        address = global_config.fee_recipient
    )]
    pub fee_recipient: SystemAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn buy(
    ctx: Context<Buy>,
    sol_amount: u64,
    min_tokens_out: u64,
) -> Result<()> {
    require!(
        sol_amount >= MIN_SOL_AMOUNT,
        LaunchpadError::InsufficientSolAmount
    );

    let global_config = &ctx.accounts.global_config;
    let bonding_curve = &mut ctx.accounts.bonding_curve;
    let buyer = &ctx.accounts.buyer;

    // Calculate platform fee
    let platform_fee = calculate_fee(sol_amount, global_config.platform_fee_bps)?;
    let sol_amount_after_fee = sol_amount
        .checked_sub(platform_fee)
        .ok_or(LaunchpadError::MathOverflow)?;

    // Calculate tokens to receive
    let tokens_out = calculate_tokens_out(
        sol_amount_after_fee,
        bonding_curve.virtual_sol_reserves,
        bonding_curve.virtual_token_reserves,
    )?;

    require!(
        tokens_out >= min_tokens_out,
        LaunchpadError::SlippageExceeded
    );

    require!(
        tokens_out <= bonding_curve.real_token_reserves,
        LaunchpadError::InsufficientTokenAmount
    );

    // Transfer SOL from buyer to program
    let transfer_instruction = anchor_lang::system_program::Transfer {
        from: buyer.to_account_info(),
        to: bonding_curve.to_account_info(),
    };
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            transfer_instruction,
        ),
        sol_amount_after_fee,
    )?;

    // Transfer platform fee to fee recipient
    if platform_fee > 0 {
        let fee_transfer_instruction = anchor_lang::system_program::Transfer {
            from: buyer.to_account_info(),
            to: ctx.accounts.fee_recipient.to_account_info(),
        };
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                fee_transfer_instruction,
            ),
            platform_fee,
        )?;
    }

    // Transfer tokens from curve to buyer
    let mint_key = ctx.accounts.mint.key();
    let seeds = &[
        BONDING_CURVE_SEED,
        mint_key.as_ref(),
        &[bonding_curve.bump],
    ];
    let signer_seeds = &[&seeds[..]];

    let transfer_accounts = Transfer {
        from: ctx.accounts.curve_token_account.to_account_info(),
        to: ctx.accounts.buyer_token_account.to_account_info(),
        authority: bonding_curve.to_account_info(),
    };
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            transfer_accounts,
            signer_seeds,
        ),
        tokens_out,
    )?;

    // Update bonding curve state
    bonding_curve.virtual_sol_reserves = bonding_curve
        .virtual_sol_reserves
        .checked_add(sol_amount_after_fee)
        .ok_or(LaunchpadError::MathOverflow)?;
    
    bonding_curve.virtual_token_reserves = bonding_curve
        .virtual_token_reserves
        .checked_sub(tokens_out)
        .ok_or(LaunchpadError::MathOverflow)?;
    
    bonding_curve.real_sol_reserves = bonding_curve
        .real_sol_reserves
        .checked_add(sol_amount_after_fee)
        .ok_or(LaunchpadError::MathOverflow)?;
    
    bonding_curve.real_token_reserves = bonding_curve
        .real_token_reserves
        .checked_sub(tokens_out)
        .ok_or(LaunchpadError::MathOverflow)?;

    // Check if bonding curve is complete
    if is_curve_complete(bonding_curve.real_sol_reserves) {
        bonding_curve.complete = true;
        bonding_curve.completed_at = Some(Clock::get()?.unix_timestamp);

        emit!(BondingCurveCompleted {
            mint: ctx.accounts.mint.key(),
            final_sol_reserves: bonding_curve.real_sol_reserves,
            final_token_reserves: bonding_curve.real_token_reserves,
            total_supply: bonding_curve.total_supply,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    emit!(TokenPurchased {
        mint: ctx.accounts.mint.key(),
        buyer: buyer.key(),
        sol_amount,
        token_amount: tokens_out,
        virtual_sol_reserves: bonding_curve.virtual_sol_reserves,
        virtual_token_reserves: bonding_curve.virtual_token_reserves,
        real_sol_reserves: bonding_curve.real_sol_reserves,
        real_token_reserves: bonding_curve.real_token_reserves,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Buy executed: buyer={}, sol_amount={}, tokens_out={}, platform_fee={}",
        buyer.key(),
        sol_amount,
        tokens_out,
        platform_fee
    );

    Ok(())
}
