use anchor_lang::prelude::*;

use crate::errors::MarketError;
use crate::events::ListingPriceUpdated;
use crate::log_fn;
use crate::state::{Listing, MarketplaceConfig, LISTING_SEED};

#[derive(Accounts)]
pub struct UpdateListingPrice<'info> {
    pub seller: Signer<'info>,

    #[account(
        seeds = [crate::state::MARKETPLACE_CONFIG_SEED, marketplace_config.collection_mint.as_ref()],
        bump = marketplace_config.bump,
    )]
    pub marketplace_config: Account<'info, MarketplaceConfig>,

    #[account(
        mut,
        seeds = [LISTING_SEED, marketplace_config.key().as_ref(), listing.asset.as_ref()],
        bump = listing.bump,
        constraint = listing.seller == seller.key() @ MarketError::SellerMismatch,
    )]
    pub listing: Account<'info, Listing>,
}

pub fn handler(ctx: Context<UpdateListingPrice>, new_price_lamports: u64) -> Result<()> {
    log_fn!("market", "update_listing_price");

    let config = &ctx.accounts.marketplace_config;
    require!(config.enabled, MarketError::MarketplaceDisabled);
    require!(
        new_price_lamports >= config.min_price_lamports,
        MarketError::PriceTooLow
    );

    let listing = &mut ctx.accounts.listing;
    msg!(
        "💱 Repricing asset={} {} -> {}",
        listing.asset,
        listing.price_lamports,
        new_price_lamports
    );
    listing.price_lamports = new_price_lamports;

    let now = Clock::get()?.unix_timestamp;
    emit!(ListingPriceUpdated {
        asset: listing.asset,
        seller: listing.seller,
        new_price_lamports,
        timestamp: now,
    });

    Ok(())
}
