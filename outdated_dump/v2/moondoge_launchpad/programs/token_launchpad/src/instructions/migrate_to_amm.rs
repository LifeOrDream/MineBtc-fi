use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::state::*;
use crate::constants::*;
use crate::errors::*;
use crate::events::*;
use crate::math::*;

#[derive(Accounts)]
pub struct MigrateToAmm<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [BONDING_CURVE_SEED, mint.key().as_ref()],
        bump = bonding_curve.bump,
        constraint = bonding_curve.can_migrate() @ LaunchpadError::CannotMigrateIncomplete
    )]
    pub bonding_curve: Account<'info, BondingCurve>,

    pub mint: Account<'info, token::Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = bonding_curve,
    )]
    pub curve_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        mut,
        address = global_config.fee_recipient
    )]
    pub fee_recipient: SystemAccount<'info>,

    /// CHECK: This will be the AMM pool account (external program)
    #[account(mut)]
    pub amm_pool: UncheckedAccount<'info>,

    /// CHECK: This will be the AMM program (external)
    pub amm_program: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn migrate_to_amm(
    ctx: Context<MigrateToAmm>,
    weight_token: u64,
    weight_sol: u64,
) -> Result<()> {
    require!(
        weight_token > 0 && weight_sol > 0,
        LaunchpadError::InvalidWeightParameters
    );
    require!(
        weight_token + weight_sol == 100,
        LaunchpadError::InvalidWeightParameters
    );

    let global_config = &ctx.accounts.global_config;
    let bonding_curve = &mut ctx.accounts.bonding_curve;
    let creator = &ctx.accounts.creator;

    // Verify creator is the original token creator
    require!(
        bonding_curve.creator == creator.key(),
        LaunchpadError::Unauthorized
    );

    let sol_amount = bonding_curve.real_sol_reserves;
    let token_amount = bonding_curve.real_token_reserves;

    // Calculate migration fee
    let migration_fee = calculate_fee(sol_amount, global_config.migration_fee_bps)?;
    let sol_amount_after_fee = sol_amount
        .checked_sub(migration_fee)
        .ok_or(LaunchpadError::MathOverflow)?;

    // Transfer migration fee to fee recipient
    if migration_fee > 0 {
        **bonding_curve.to_account_info().try_borrow_mut_lamports()? = bonding_curve
            .to_account_info()
            .lamports()
            .checked_sub(migration_fee)
            .ok_or(LaunchpadError::MathOverflow)?;
        
        **ctx.accounts.fee_recipient.to_account_info().try_borrow_mut_lamports()? = ctx
            .accounts
            .fee_recipient
            .to_account_info()
            .lamports()
            .checked_add(migration_fee)
            .ok_or(LaunchpadError::MathOverflow)?;
    }

    // Create CPI call to weighted AMM program to create pool
    // This is a simplified version - in production you'd need proper CPI integration
    
    // The actual implementation would involve:
    // 1. CPI call to weighted_amm::create_pool with the specified weights
    // 2. Transfer SOL and tokens from bonding curve to the new AMM pool
    // 3. Receive LP tokens from the pool creation
    // 4. Burn LP tokens to ensure permanent liquidity (pump.fun style)
    
    // For this example, we simulate the migration
    bonding_curve.migrated = true;
    bonding_curve.migrated_at = Some(Clock::get()?.unix_timestamp);
    bonding_curve.amm_pool = Some(ctx.accounts.amm_pool.key());

    // Clear the reserves since they've been migrated
    bonding_curve.real_sol_reserves = 0;
    bonding_curve.real_token_reserves = 0;

    emit!(TokenMigrated {
        mint: ctx.accounts.mint.key(),
        amm_pool: ctx.accounts.amm_pool.key(),
        sol_amount: sol_amount_after_fee,
        token_amount,
        weight_token,
        weight_sol,
        lp_tokens_burned: 0, // TODO: Calculate actual LP tokens burned
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Token migrated: mint={}, amm_pool={}, sol_amount={}, token_amount={}, weights={}:{}",
        ctx.accounts.mint.key(),
        ctx.accounts.amm_pool.key(),
        sol_amount_after_fee,
        token_amount,
        weight_token,
        weight_sol
    );

    Ok(())
}
