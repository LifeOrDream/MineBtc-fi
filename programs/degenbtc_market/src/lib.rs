#![allow(unexpected_cfgs, deprecated)]
#![allow(
    clippy::identity_op,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::useless_asref
)]

//! # degenbtc_market
//!
//! Minimal NFT marketplace for the MineBTC mpl-core HashBeast collection. Standalone
//! Anchor program — `mineBTC` CPIs into it (see `programs/mineBTC` for the
//! inventory + lootbox flows that consume this surface).
//!
//! Surface area (intentionally small):
//! - `initialize_marketplace` — admin one-shot per collection
//! - `update_marketplace_config` — admin tunes fee / recipient / min price / enabled
//! - `list_nft` — owner escrows the asset under a deterministic PDA
//! - `cancel_listing` — owner pulls escrow back
//! - `update_listing_price` — owner re-prices in place
//! - `buy_listing` — buyer pays SOL, asset hops escrow → buyer
//!
//! No bids, no expiry, no auctions. 3% flat fee (admin-tunable, hard cap 10%).

use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod mpl_core_helpers;
pub mod state;

use instructions::*;

declare_id!("AUFtfniP5h2UYNgzfVRkcKpQu3R45uSCtL7VZRCvCNnE");

#[macro_export]
macro_rules! log_fn {
    ($module:expr, $name:expr) => {
        msg!(concat!("📍 [", $module, ".", $name, "]"));
    };
}

#[program]
pub mod degenbtc_market {
    use super::*;

    /// One-shot per collection. Initializes a `MarketplaceConfig` PDA, records
    /// the verified collection mint, and caches the mpl-core program id used
    /// by every subsequent listing/transfer.
    pub fn initialize_marketplace(
        ctx: Context<InitializeMarketplace>,
        fee_bps: u16,
        fee_recipient: Pubkey,
        min_price_lamports: u64,
        mpl_core_program: Pubkey,
    ) -> Result<()> {
        instructions::initialize::handler(
            ctx,
            fee_bps,
            fee_recipient,
            min_price_lamports,
            mpl_core_program,
        )
    }

    /// Admin-only. Each `Some` field is overwritten; `None` leaves the field as-is.
    pub fn update_marketplace_config(
        ctx: Context<UpdateMarketplaceConfig>,
        fee_bps: Option<u16>,
        fee_recipient: Option<Pubkey>,
        min_price_lamports: Option<u64>,
        enabled: Option<bool>,
    ) -> Result<()> {
        instructions::update_config::handler(
            ctx,
            fee_bps,
            fee_recipient,
            min_price_lamports,
            enabled,
        )
    }

    /// Escrow asset to `[b"escrow", config, asset]` PDA and create a `Listing`.
    pub fn list_nft(ctx: Context<ListNft>, price_lamports: u64) -> Result<()> {
        instructions::list_nft::handler(ctx, price_lamports)
    }

    /// Seller cancels — asset returns to seller, listing closes (rent refund).
    pub fn cancel_listing(ctx: Context<CancelListing>) -> Result<()> {
        instructions::cancel_listing::handler(ctx)
    }

    /// Seller re-prices an existing listing. Subject to `min_price_lamports`.
    pub fn update_listing_price(
        ctx: Context<UpdateListingPrice>,
        new_price_lamports: u64,
    ) -> Result<()> {
        instructions::update_listing_price::handler(ctx, new_price_lamports)
    }

    /// Buyer pays SOL (fee + proceeds), asset hops escrow → buyer, listing closes.
    pub fn buy_listing(ctx: Context<BuyListing>, max_price_lamports: u64) -> Result<()> {
        instructions::buy_listing::handler(ctx, max_price_lamports)
    }

    /// Permissionless reclaim of a stale listing whose asset owner no longer
    /// matches the recorded seller. Closes the listing and refunds rent to caller.
    pub fn reclaim_stale_listing(ctx: Context<ReclaimStaleListing>) -> Result<()> {
        instructions::reclaim_stale_listing::handler(ctx)
    }
}
