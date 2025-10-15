use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, MintTo, Transfer};
use anchor_spl::associated_token::AssociatedToken;

use crate::state::*;
use crate::constants::*;
use crate::errors::*;
use crate::events::*;
use crate::math::*;

#[derive(Accounts)]
pub struct CreatePool<'info> {
    #[account(
        seeds = [AMM_CONFIG_SEED, &amm_config.index.to_le_bytes()],
        bump = amm_config.bump
    )]
    pub amm_config: Account<'info, AmmConfig>,

    #[account(
        init,
        payer = pool_creator,
        space = PoolState::SPACE,
        seeds = [
            POOL_SEED,
            amm_config.key().as_ref(),
            token_0_mint.key().as_ref(),
            token_1_mint.key().as_ref()
        ],
        bump
    )]
    pub pool_state: Account<'info, PoolState>,

    /// CHECK: This account will be used as the authority for the pool vaults and LP mint
    #[account(
        seeds = [POOL_AUTH_SEED],
        bump
    )]
    pub pool_authority: UncheckedAccount<'info>,

    pub token_0_mint: Account<'info, Mint>,
    pub token_1_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = pool_creator,
        token::mint = token_0_mint,
        token::authority = pool_authority,
        seeds = [
            POOL_VAULT_SEED,
            pool_state.key().as_ref(),
            token_0_mint.key().as_ref()
        ],
        bump
    )]
    pub token_0_vault: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = pool_creator,
        token::mint = token_1_mint,
        token::authority = pool_authority,
        seeds = [
            POOL_VAULT_SEED,
            pool_state.key().as_ref(),
            token_1_mint.key().as_ref()
        ],
        bump
    )]
    pub token_1_vault: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = pool_creator,
        mint::decimals = 6,
        mint::authority = pool_authority,
        seeds = [
            POOL_LP_MINT_SEED,
            pool_state.key().as_ref()
        ],
        bump
    )]
    pub lp_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = pool_creator,
        associated_token::mint = lp_mint,
        associated_token::authority = pool_creator,
    )]
    pub creator_lp_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = token_0_mint,
        token::authority = pool_creator,
    )]
    pub creator_token_0_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = token_1_mint,
        token::authority = pool_creator,
    )]
    pub creator_token_1_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub pool_creator: Signer<'info>,

    #[account(
        mut,
        address = amm_config.fund_owner
    )]
    pub fund_owner: SystemAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn create_pool(
    ctx: Context<CreatePool>,
    token_0_weight: u64,
    token_1_weight: u64,
    init_amount_0: u64,
    init_amount_1: u64,
    open_time: u64,
) -> Result<()> {
    // Validate weights
    require!(
        token_0_weight >= MIN_WEIGHT && token_0_weight <= MAX_WEIGHT,
        AmmError::InvalidWeights
    );
    require!(
        token_1_weight >= MIN_WEIGHT && token_1_weight <= MAX_WEIGHT,
        AmmError::InvalidWeights
    );
    require!(
        token_0_weight + token_1_weight == TOTAL_WEIGHT,
        AmmError::InvalidWeights
    );

    // Validate initial amounts
    require!(
        init_amount_0 > 0 && init_amount_1 > 0,
        AmmError::InvalidTokenAmount
    );

    let amm_config = &ctx.accounts.amm_config;
    let pool_state = &mut ctx.accounts.pool_state;
    let pool_creator = &ctx.accounts.pool_creator;

    // Collect pool creation fee
    if amm_config.create_pool_fee > 0 {
        let transfer_instruction = anchor_lang::system_program::Transfer {
            from: pool_creator.to_account_info(),
            to: ctx.accounts.fund_owner.to_account_info(),
        };
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                transfer_instruction,
            ),
            amm_config.create_pool_fee,
        )?;
    }

    // Initialize pool state
    pool_state.amm_config = amm_config.key();
    pool_state.pool_creator = pool_creator.key();
    pool_state.token_0_vault = ctx.accounts.token_0_vault.key();
    pool_state.token_1_vault = ctx.accounts.token_1_vault.key();
    pool_state.lp_mint = ctx.accounts.lp_mint.key();
    pool_state.token_0_mint = ctx.accounts.token_0_mint.key();
    pool_state.token_1_mint = ctx.accounts.token_1_mint.key();
    pool_state.token_0_protocol_fee = 0;
    pool_state.token_1_protocol_fee = 0;
    pool_state.token_0_fund_fee = 0;
    pool_state.token_1_fund_fee = 0;
    pool_state.open_time = open_time;
    pool_state.recent_epoch = Clock::get()?.epoch;
    pool_state.token_0_weight = token_0_weight;
    pool_state.token_1_weight = token_1_weight;
    pool_state.total_weight = TOTAL_WEIGHT;
    pool_state.status = POOL_STATUS_ENABLED;
    pool_state.bump = ctx.bumps.pool_state;
    pool_state.padding = [0; 32];

    // Transfer initial tokens from creator to pool vaults
    let transfer_0_accounts = Transfer {
        from: ctx.accounts.creator_token_0_account.to_account_info(),
        to: ctx.accounts.token_0_vault.to_account_info(),
        authority: pool_creator.to_account_info(),
    };
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            transfer_0_accounts,
        ),
        init_amount_0,
    )?;

    let transfer_1_accounts = Transfer {
        from: ctx.accounts.creator_token_1_account.to_account_info(),
        to: ctx.accounts.token_1_vault.to_account_info(),
        authority: pool_creator.to_account_info(),
    };
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            transfer_1_accounts,
        ),
        init_amount_1,
    )?;

    // Calculate initial LP tokens using geometric mean
    let lp_amount = calculate_lp_tokens_for_deposit(
        0,
        0,
        init_amount_0 as u128,
        init_amount_1 as u128,
        token_0_weight as u128,
        token_1_weight as u128,
        0,
    )? as u64;

    // Mint LP tokens to creator
    let auth_seeds = &[POOL_AUTH_SEED, &[ctx.bumps.pool_authority]];
    let signer_seeds = &[&auth_seeds[..]];

    let mint_to_accounts = MintTo {
        mint: ctx.accounts.lp_mint.to_account_info(),
        to: ctx.accounts.creator_lp_token_account.to_account_info(),
        authority: ctx.accounts.pool_authority.to_account_info(),
    };
    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            mint_to_accounts,
            signer_seeds,
        ),
        lp_amount,
    )?;

    emit!(PoolCreated {
        pool_state: pool_state.key(),
        token_0_mint: ctx.accounts.token_0_mint.key(),
        token_1_mint: ctx.accounts.token_1_mint.key(),
        token_0_vault: ctx.accounts.token_0_vault.key(),
        token_1_vault: ctx.accounts.token_1_vault.key(),
        lp_mint: ctx.accounts.lp_mint.key(),
        token_0_weight,
        token_1_weight,
        init_amount_0,
        init_amount_1,
        lp_amount,
    });

    msg!(
        "Pool created: token_0={}, token_1={}, weights={}:{}, amounts={}:{}, lp_amount={}",
        ctx.accounts.token_0_mint.key(),
        ctx.accounts.token_1_mint.key(),
        token_0_weight,
        token_1_weight,
        init_amount_0,
        init_amount_1,
        lp_amount
    );

    Ok(())
}
