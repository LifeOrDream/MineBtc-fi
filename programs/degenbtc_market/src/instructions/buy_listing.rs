use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};
use mpl_core::accounts::BaseCollectionV1;

use crate::errors::MarketError;
use crate::events::NftSold;
use crate::log_fn;
use crate::mpl_core_helpers::transfer_mpl_core_asset;
use crate::state::{Listing, MarketplaceConfig, BPS_DENOMINATOR, ESCROW_SEED, LISTING_SEED};

#[derive(Accounts)]
pub struct BuyListing<'info> {
    /// Pays SOL (price), receives the asset.
    #[account(mut)]
    pub buyer: Signer<'info>,

    /// Receives `price - fee` in SOL plus the listing rent refund.
    /// CHECK: pubkey is verified against `listing.seller`.
    #[account(
        mut,
        constraint = seller.key() == listing.seller @ MarketError::SellerMismatch,
    )]
    pub seller: AccountInfo<'info>,

    #[account(
        seeds = [crate::state::MARKETPLACE_CONFIG_SEED, marketplace_config.collection_mint.as_ref()],
        bump = marketplace_config.bump,
    )]
    pub marketplace_config: Account<'info, MarketplaceConfig>,

    #[account(
        mut,
        close = seller,
        seeds = [LISTING_SEED, marketplace_config.key().as_ref(), asset.key().as_ref()],
        bump = listing.bump,
        has_one = asset @ MarketError::InvalidAsset,
    )]
    pub listing: Account<'info, Listing>,

    /// CHECK: Asset transferred escrow -> buyer. Seeds verified via `has_one` on listing.
    #[account(mut)]
    pub asset: AccountInfo<'info>,

    #[account(
        mut,
        constraint = collection.key() == marketplace_config.collection_mint @ MarketError::InvalidCollection,
    )]
    pub collection: Account<'info, BaseCollectionV1>,

    /// CHECK: Escrow PDA, signs via seeds.
    #[account(
        seeds = [ESCROW_SEED, marketplace_config.key().as_ref(), asset.key().as_ref()],
        bump,
    )]
    pub escrow: UncheckedAccount<'info>,

    /// CHECK: Receives `fee` lamports. Pubkey verified against config.fee_recipient.
    #[account(
        mut,
        constraint = fee_recipient.key() == marketplace_config.fee_recipient @ MarketError::InvalidFeeRecipient,
    )]
    pub fee_recipient: AccountInfo<'info>,

    /// CHECK: mpl-core program. Verified against the cached pubkey in config.
    #[account(
        constraint = mpl_core_program.key() == marketplace_config.mpl_core_program @ MarketError::InvalidMplCoreProgram,
    )]
    pub mpl_core_program: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<BuyListing>) -> Result<()> {
    log_fn!("market", "buy_listing");

    let config = &ctx.accounts.marketplace_config;
    require!(config.enabled, MarketError::MarketplaceDisabled);

    let price = ctx.accounts.listing.price_lamports;
    let fee = (price as u128)
        .checked_mul(config.fee_bps as u128)
        .ok_or(MarketError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(MarketError::MathOverflow)? as u64;
    let to_seller = price.checked_sub(fee).ok_or(MarketError::MathOverflow)?;

    msg!(
        "💰 Buy: price={} fee={} ({}bps) to_seller={}",
        price,
        fee,
        config.fee_bps,
        to_seller
    );

    // Cheap pre-flight: refuse if buyer balance is below price. The system
    // transfer will fail anyway, but a typed error is friendlier.
    require!(
        ctx.accounts.buyer.lamports() >= price,
        MarketError::InsufficientFunds
    );

    // 1. SOL: buyer -> fee_recipient (3%).
    if fee > 0 {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.buyer.to_account_info(),
                to: ctx.accounts.fee_recipient.to_account_info(),
            },
        );
        system_program::transfer(cpi_ctx, fee)?;
    }

    // 2. SOL: buyer -> seller (97%).
    if to_seller > 0 {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.buyer.to_account_info(),
                to: ctx.accounts.seller.to_account_info(),
            },
        );
        system_program::transfer(cpi_ctx, to_seller)?;
    }

    // 3. Asset: escrow -> buyer (escrow PDA signs via seeds).
    let config_key = config.key();
    let asset_key = ctx.accounts.asset.key();
    let escrow_bump = ctx.bumps.escrow;

    let signer_seeds_inner: &[&[u8]] = &[
        ESCROW_SEED,
        config_key.as_ref(),
        asset_key.as_ref(),
        &[escrow_bump],
    ];
    let signers: &[&[&[u8]]] = &[signer_seeds_inner];

    // `payer` here is the buyer — they're a signer with lamports, and TransferV1
    // may need to pay for asset reallocation (plugin add/remove side effects).
    transfer_mpl_core_asset(
        &ctx.accounts.asset.to_account_info(),
        Some(&ctx.accounts.collection.to_account_info()),
        &ctx.accounts.buyer.to_account_info(),
        &ctx.accounts.escrow.to_account_info(),
        &ctx.accounts.buyer.to_account_info(),
        &ctx.accounts.mpl_core_program,
        Some(signers),
    )?;

    let now = Clock::get()?.unix_timestamp;

    msg!(
        "🎉 NFT sold: asset={} buyer={} seller={}",
        asset_key,
        ctx.accounts.buyer.key(),
        ctx.accounts.seller.key()
    );

    emit!(NftSold {
        asset: asset_key,
        buyer: ctx.accounts.buyer.key(),
        seller: ctx.accounts.seller.key(),
        price_lamports: price,
        fee_lamports: fee,
        timestamp: now,
    });

    // Anchor closes `listing` to `seller` at instruction exit (rent refund).
    Ok(())
}
