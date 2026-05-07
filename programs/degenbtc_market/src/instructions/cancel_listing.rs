use anchor_lang::prelude::*;
use mpl_core::accounts::BaseCollectionV1;

use crate::errors::MarketError;
use crate::events::ListingCancelled;
use crate::log_fn;
use crate::mpl_core_helpers::transfer_mpl_core_asset;
use crate::state::{Listing, MarketplaceConfig, ESCROW_SEED, LISTING_SEED};

#[derive(Accounts)]
pub struct CancelListing<'info> {
    /// Original lister. Receives the asset back and the listing rent refund.
    #[account(mut)]
    pub seller: Signer<'info>,

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
        constraint = listing.seller == seller.key() @ MarketError::SellerMismatch,
    )]
    pub listing: Account<'info, Listing>,

    /// CHECK: Asset will be transferred from escrow back to seller. We
    /// re-check the asset key matches `listing.asset` via `has_one`.
    #[account(mut)]
    pub asset: AccountInfo<'info>,

    #[account(
        mut,
        constraint = collection.key() == marketplace_config.collection_mint @ MarketError::InvalidCollection,
    )]
    pub collection: Account<'info, BaseCollectionV1>,

    /// CHECK: Escrow PDA — derived from config + asset, signs the transfer
    /// back to seller via PDA seeds. No data, no init.
    #[account(
        seeds = [ESCROW_SEED, marketplace_config.key().as_ref(), asset.key().as_ref()],
        bump,
    )]
    pub escrow: UncheckedAccount<'info>,

    /// CHECK: mpl-core program. Verified against the cached pubkey in config.
    #[account(
        constraint = mpl_core_program.key() == marketplace_config.mpl_core_program @ MarketError::InvalidMplCoreProgram,
    )]
    pub mpl_core_program: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CancelListing>) -> Result<()> {
    log_fn!("market", "cancel_listing");

    let config_key = ctx.accounts.marketplace_config.key();
    let asset_key = ctx.accounts.asset.key();
    let escrow_bump = ctx.bumps.escrow;

    // Build escrow signer seeds. Order matters: must match the `seeds = [...]`
    // in the Accounts struct exactly.
    let signer_seeds_inner: &[&[u8]] = &[
        ESCROW_SEED,
        config_key.as_ref(),
        asset_key.as_ref(),
        &[escrow_bump],
    ];
    let signers: &[&[&[u8]]] = &[signer_seeds_inner];

    // Transfer escrow -> seller. The escrow PDA signs via seeds.
    // NOTE: `payer` must be the human seller (signer), not the escrow PDA —
    // mpl-core may grow the asset account on transfer (plugin reallocation),
    // and PDAs without lamports can't pay rent.
    transfer_mpl_core_asset(
        &ctx.accounts.asset.to_account_info(),
        Some(&ctx.accounts.collection.to_account_info()),
        &ctx.accounts.seller.to_account_info(),
        &ctx.accounts.escrow.to_account_info(),
        &ctx.accounts.seller.to_account_info(),
        &ctx.accounts.mpl_core_program,
        Some(signers),
    )?;

    msg!(
        "🛑 Listing cancelled: asset={} seller={}",
        asset_key,
        ctx.accounts.seller.key()
    );

    let now = Clock::get()?.unix_timestamp;
    emit!(ListingCancelled {
        asset: asset_key,
        seller: ctx.accounts.seller.key(),
        timestamp: now,
    });

    // Anchor closes `listing` to `seller` at instruction exit. The escrow
    // signer we just used was derived from config+asset, not from the
    // listing PDA, so this ordering is fine.
    Ok(())
}
