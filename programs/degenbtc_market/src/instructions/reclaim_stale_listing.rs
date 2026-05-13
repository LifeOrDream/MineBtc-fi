use anchor_lang::prelude::*;
use mpl_core::accounts::BaseAssetV1;

use crate::errors::MarketError;
use crate::events::ListingReclaimed;
use crate::log_fn;
use crate::state::{Listing, MarketplaceConfig, LISTING_SEED, MARKETPLACE_CONFIG_SEED};

#[derive(Accounts)]
pub struct ReclaimStaleListing<'info> {
    /// Caller receives the listing rent refund as incentive.
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(
        seeds = [MARKETPLACE_CONFIG_SEED, marketplace_config.collection_mint.as_ref()],
        bump = marketplace_config.bump,
    )]
    pub marketplace_config: Account<'info, MarketplaceConfig>,

    #[account(
        mut,
        close = caller,
        seeds = [LISTING_SEED, marketplace_config.key().as_ref(), asset.key().as_ref()],
        bump = listing.bump,
    )]
    pub listing: Account<'info, Listing>,

    /// CHECK: Asset account. We read its owner to verify the listing is stale.
    #[account(mut)]
    pub asset: AccountInfo<'info>,

    /// CHECK: mpl-core program. Verified against cached config.
    #[account(
        constraint = mpl_core_program.key() == marketplace_config.mpl_core_program @ MarketError::InvalidMplCoreProgram,
    )]
    pub mpl_core_program: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ReclaimStaleListing>) -> Result<()> {
    log_fn!("market", "reclaim_stale_listing");

    let asset_info = &ctx.accounts.asset;
    let asset_data: BaseAssetV1 =
        BaseAssetV1::try_from(asset_info).map_err(|_| MarketError::InvalidAsset)?;

    // Stale if the current asset owner is no longer the listing seller.
    require!(
        asset_data.owner != ctx.accounts.listing.seller,
        MarketError::ListingNotStale
    );

    let now = Clock::get()?.unix_timestamp;
    msg!(
        "🧹 Stale listing reclaimed: asset={} old_seller={} new_owner={}",
        ctx.accounts.asset.key(),
        ctx.accounts.listing.seller,
        asset_data.owner
    );

    emit!(ListingReclaimed {
        asset: ctx.accounts.asset.key(),
        old_seller: ctx.accounts.listing.seller,
        new_owner: asset_data.owner,
        timestamp: now,
    });

    Ok(())
}
