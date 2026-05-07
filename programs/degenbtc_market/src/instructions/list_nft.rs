use anchor_lang::prelude::*;
use mpl_core::{
    accounts::{BaseAssetV1, BaseCollectionV1},
    fetch_plugin,
    types::{FreezeDelegate, PluginType, Royalties, UpdateAuthority},
};

use crate::errors::MarketError;
use crate::events::NftListed;
use crate::log_fn;
use crate::mpl_core_helpers::transfer_mpl_core_asset;
use crate::state::{Listing, MarketplaceConfig, ESCROW_SEED, LISTING_SEED};

#[derive(Accounts)]
pub struct ListNft<'info> {
    /// Current asset owner. Pays rent for the new `Listing` account and signs
    /// the escrow `TransferV1`.
    #[account(mut)]
    pub seller: Signer<'info>,

    #[account(
        seeds = [crate::state::MARKETPLACE_CONFIG_SEED, marketplace_config.collection_mint.as_ref()],
        bump = marketplace_config.bump,
    )]
    pub marketplace_config: Account<'info, MarketplaceConfig>,

    #[account(
        init,
        payer = seller,
        space = Listing::LEN,
        seeds = [LISTING_SEED, marketplace_config.key().as_ref(), asset.key().as_ref()],
        bump,
    )]
    pub listing: Account<'info, Listing>,

    /// CHECK: mpl-core writes to this on TransferV1 (owner field flips to escrow PDA).
    /// Validated below by deserializing as `BaseAssetV1` and re-checking key.
    #[account(mut)]
    pub asset: AccountInfo<'info>,

    /// Must match `marketplace_config.collection_mint`. Mut because mpl-core
    /// updates `current_size` etc. on transfer for assets in a collection.
    #[account(
        mut,
        constraint = collection.key() == marketplace_config.collection_mint @ MarketError::InvalidCollection,
    )]
    pub collection: Account<'info, BaseCollectionV1>,

    /// CHECK: Escrow PDA — verified by seeds. mpl-core stores this pubkey as
    /// the asset's new `owner` field. We never `init` it; the address has no
    /// data account and zero lamports. mpl-core's TransferV1 only reads the
    /// pubkey of `new_owner`, so an AccountInfo with the right key is enough.
    #[account(
        seeds = [ESCROW_SEED, marketplace_config.key().as_ref(), asset.key().as_ref()],
        bump,
    )]
    pub escrow: UncheckedAccount<'info>,

    /// CHECK: mpl-core program. Verified against the cached pubkey in `config`.
    #[account(
        constraint = mpl_core_program.key() == marketplace_config.mpl_core_program @ MarketError::InvalidMplCoreProgram,
    )]
    pub mpl_core_program: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ListNft>, price_lamports: u64) -> Result<()> {
    log_fn!("market", "list_nft");

    let config = &ctx.accounts.marketplace_config;
    require!(config.enabled, MarketError::MarketplaceDisabled);
    require!(
        price_lamports >= config.min_price_lamports,
        MarketError::PriceTooLow
    );

    // 1. Owner check — read current asset state straight from the account.
    let asset_info = &ctx.accounts.asset;
    let asset_data: BaseAssetV1 =
        BaseAssetV1::try_from(asset_info).map_err(|_| MarketError::InvalidAsset)?;
    require_keys_eq!(
        asset_data.owner,
        ctx.accounts.seller.key(),
        MarketError::SellerMismatch
    );

    // 2. Collection membership — must be the registered collection.
    match asset_data.update_authority {
        UpdateAuthority::Collection(c) => {
            require_keys_eq!(c, config.collection_mint, MarketError::NotCollectionMember);
        }
        _ => return err!(MarketError::NotCollectionMember),
    }

    // 3. Plugin gate — block frozen assets and Royalties-equipped assets.
    //    Frozen would cause the escrow TransferV1 to fail; Royalties would
    //    intercept future transfers with the wrong rule_set.
    if let Ok((_authority, freeze, _offset)) =
        fetch_plugin::<BaseAssetV1, FreezeDelegate>(asset_info, PluginType::FreezeDelegate)
    {
        if freeze.frozen {
            msg!("❌ Asset has FreezeDelegate.frozen = true");
            return err!(MarketError::UnsupportedPlugin);
        }
    }

    if fetch_plugin::<BaseAssetV1, Royalties>(asset_info, PluginType::Royalties).is_ok() {
        msg!(
            "❌ Asset has Royalties plugin attached — not supported (marketplace fee handles this)"
        );
        return err!(MarketError::UnsupportedPlugin);
    }

    msg!("🔒 Escrow PDA: {}", ctx.accounts.escrow.key());
    let asset_key = asset_info.key();

    // 4. Transfer asset seller -> escrow PDA.
    transfer_mpl_core_asset(
        &ctx.accounts.asset.to_account_info(),
        Some(&ctx.accounts.collection.to_account_info()),
        &ctx.accounts.seller.to_account_info(),
        &ctx.accounts.seller.to_account_info(),
        &ctx.accounts.escrow.to_account_info(),
        &ctx.accounts.mpl_core_program,
        None,
    )?;

    // 5. Init listing.
    let now = Clock::get()?.unix_timestamp;
    let listing = &mut ctx.accounts.listing;
    listing.bump = ctx.bumps.listing;
    listing.seller = ctx.accounts.seller.key();
    listing.asset = asset_key;
    listing.price_lamports = price_lamports;
    listing.created_at = now;

    msg!(
        "📦 Listing created: asset={} seller={} price={}",
        listing.asset,
        listing.seller,
        listing.price_lamports
    );

    emit!(NftListed {
        asset: asset_key,
        seller: ctx.accounts.seller.key(),
        price_lamports,
        timestamp: now,
    });

    Ok(())
}
