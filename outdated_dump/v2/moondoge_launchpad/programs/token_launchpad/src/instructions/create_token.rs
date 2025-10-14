use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, MintTo};
use anchor_spl::associated_token::AssociatedToken;
use mpl_token_metadata::instructions::{CreateMetadataAccountV3, CreateMetadataAccountV3InstructionArgs};
use mpl_token_metadata::types::{DataV2, Creator, Collection, Uses};
use solana_program::program::invoke;

use crate::state::*;
use crate::constants::*;
use crate::errors::*;
use crate::events::*;

#[derive(Accounts)]
#[instruction(name: String, symbol: String)]
pub struct CreateToken<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        init,
        payer = creator,
        mint::decimals = TOKEN_DECIMALS,
        mint::authority = bonding_curve,
        mint::freeze_authority = bonding_curve,
    )]
    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = creator,
        space = BondingCurve::SPACE,
        seeds = [BONDING_CURVE_SEED, mint.key().as_ref()],
        bump
    )]
    pub bonding_curve: Account<'info, BondingCurve>,

    #[account(
        init,
        payer = creator,
        space = TokenMetadata::SPACE,
        seeds = [TOKEN_METADATA_SEED, mint.key().as_ref()],
        bump
    )]
    pub token_metadata: Account<'info, TokenMetadata>,

    #[account(
        init,
        payer = creator,
        associated_token::mint = mint,
        associated_token::authority = bonding_curve,
    )]
    pub curve_token_account: Account<'info, TokenAccount>,

    /// CHECK: This account will be created by the metadata program
    #[account(mut)]
    pub metadata_account: UncheckedAccount<'info>,

    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        mut,
        address = global_config.fee_recipient
    )]
    pub fee_recipient: SystemAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
    /// CHECK: This is the token metadata program
    #[account(address = mpl_token_metadata::ID)]
    pub token_metadata_program: UncheckedAccount<'info>,
}

pub fn create_token(
    ctx: Context<CreateToken>,
    name: String,
    symbol: String,
    uri: String,
    initial_virtual_sol_reserves: u64,
    initial_virtual_token_reserves: u64,
    initial_real_token_reserves: u64,
) -> Result<()> {
    // Validate input parameters
    require!(
        name.len() <= MAX_TOKEN_NAME_LENGTH,
        LaunchpadError::InvalidTokenMetadata
    );
    require!(
        symbol.len() <= MAX_TOKEN_SYMBOL_LENGTH,
        LaunchpadError::InvalidTokenMetadata
    );
    require!(
        uri.len() <= MAX_TOKEN_URI_LENGTH,
        LaunchpadError::InvalidTokenMetadata
    );
    require!(
        initial_virtual_sol_reserves > 0 && initial_virtual_token_reserves > 0 && initial_real_token_reserves > 0,
        LaunchpadError::InvalidReserves
    );

    let global_config = &ctx.accounts.global_config;
    let bonding_curve = &mut ctx.accounts.bonding_curve;
    let token_metadata = &mut ctx.accounts.token_metadata;
    let mint = &ctx.accounts.mint;
    let creator = &ctx.accounts.creator;

    // Collect token creation fee
    if global_config.token_creation_fee > 0 {
        let transfer_instruction = anchor_lang::system_program::Transfer {
            from: creator.to_account_info(),
            to: ctx.accounts.fee_recipient.to_account_info(),
        };
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                transfer_instruction,
            ),
            global_config.token_creation_fee,
        )?;
    }

    // Initialize bonding curve
    bonding_curve.mint = mint.key();
    bonding_curve.creator = creator.key();
    bonding_curve.virtual_sol_reserves = initial_virtual_sol_reserves;
    bonding_curve.virtual_token_reserves = initial_virtual_token_reserves;
    bonding_curve.real_sol_reserves = 0;
    bonding_curve.real_token_reserves = initial_real_token_reserves;
    bonding_curve.total_supply = initial_real_token_reserves;
    bonding_curve.complete = false;
    bonding_curve.migrated = false;
    bonding_curve.amm_pool = None;
    bonding_curve.created_at = Clock::get()?.unix_timestamp;
    bonding_curve.completed_at = None;
    bonding_curve.migrated_at = None;
    bonding_curve.bump = ctx.bumps.bonding_curve;

    // Initialize token metadata
    token_metadata.mint = mint.key();
    token_metadata.name = name.clone();
    token_metadata.symbol = symbol.clone();
    token_metadata.uri = uri.clone();
    token_metadata.curve_type = CurveType::Exponential;
    token_metadata.bump = ctx.bumps.token_metadata;

    // Mint initial tokens to the curve
    let mint_seeds = &[
        BONDING_CURVE_SEED,
        mint.key().as_ref(),
        &[bonding_curve.bump],
    ];
    let signer_seeds = &[&mint_seeds[..]];

    let mint_to_accounts = MintTo {
        mint: mint.to_account_info(),
        to: ctx.accounts.curve_token_account.to_account_info(),
        authority: bonding_curve.to_account_info(),
    };
    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            mint_to_accounts,
            signer_seeds,
        ),
        initial_real_token_reserves,
    )?;

    // Create metadata account
    let creators = vec![Creator {
        address: creator.key(),
        verified: true,
        share: 100,
    }];

    let data_v2 = DataV2 {
        name: name.clone(),
        symbol: symbol.clone(),
        uri: uri.clone(),
        seller_fee_basis_points: 0,
        creators: Some(creators),
        collection: None,
        uses: None,
    };

    let create_metadata_account_v3_instruction = CreateMetadataAccountV3 {
        metadata: ctx.accounts.metadata_account.key(),
        mint: mint.key(),
        mint_authority: bonding_curve.key(),
        payer: creator.key(),
        update_authority: (bonding_curve.key(), true),
        system_program: ctx.accounts.system_program.key(),
        rent: Some(ctx.accounts.rent.key()),
    };

    let create_metadata_account_v3_instruction_args = CreateMetadataAccountV3InstructionArgs {
        data: data_v2,
        is_mutable: true,
        collection_details: None,
    };

    invoke(
        &create_metadata_account_v3_instruction.instruction(create_metadata_account_v3_instruction_args),
        &[
            ctx.accounts.metadata_account.to_account_info(),
            mint.to_account_info(),
            bonding_curve.to_account_info(),
            creator.to_account_info(),
            bonding_curve.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.rent.to_account_info(),
        ],
    )?;

    emit!(TokenCreated {
        mint: mint.key(),
        creator: creator.key(),
        name,
        symbol,
        uri,
        initial_virtual_sol_reserves,
        initial_virtual_token_reserves,
        initial_real_token_reserves,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Token created: mint={}, creator={}, virtual_sol={}, virtual_token={}, real_token={}",
        mint.key(),
        creator.key(),
        initial_virtual_sol_reserves,
        initial_virtual_token_reserves,
        initial_real_token_reserves
    );

    Ok(())
}
