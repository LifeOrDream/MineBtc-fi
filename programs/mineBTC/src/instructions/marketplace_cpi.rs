// # Marketplace CPI + Inventory Disposition Helpers
//
// This module routes the program-owned NFT inventory (recycled + swept Doges)
// through the standalone `degenbtc_market` program. mineBTC's `inventory_pda`
// signs marketplace operations as a PDA via signer seeds.
//
// All entry points are `crank_authority`-gated so off-chain bots (DI cranker,
// disposition cranker, sweep cranker, proceeds sweeper) drive the lifecycle.
//
// Flows:
// - `inventory_list_nft` / `inventory_cancel_listing` / `inventory_update_price`
//   — operate on a single recycled entry, CPI into the marketplace.
// - `inventory_set_lootbox` — flip status Pending → Lootbox without involving
//   the marketplace.
// - `inventory_buy_listing` — sweep buy: pre-fund inventory_pda from the
//   sweep vault, then CPI buy_listing as the buyer.
// - `handle_inventory_proceeds` — split SOL accumulated on inventory_pda
//   from sale proceeds: 50% to sweep vault, 50% to sol_treasury.
// - `update_market_metrics` — crank pushes the demand-index snapshot.

use anchor_lang::prelude::*;
use anchor_lang::system_program::{self as sys_prog, Transfer};

use crate::errors::ErrorCode;
use crate::events::*;
use crate::state::*;

// ========================================================================================
// ============================== inventory_list_nft ======================================
// ========================================================================================

pub fn internal_inventory_list_nft(
    ctx: Context<InventoryListNft>,
    price_lamports: u64,
) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_inventory_list_nft");

    require_keys_eq!(
        ctx.accounts.crank_authority.key(),
        ctx.accounts.inventory_pool.crank_authority,
        ErrorCode::InvalidCrankAuthority
    );
    require_keys_eq!(
        ctx.accounts.marketplace_program.key(),
        ctx.accounts.inventory_pool.marketplace_program,
        ErrorCode::InvalidMarketplaceProgram
    );
    require_keys_eq!(
        ctx.accounts.marketplace_config.key(),
        ctx.accounts.inventory_pool.marketplace_config,
        ErrorCode::InvalidMarketplaceConfig
    );

    let entry = &ctx.accounts.recycled_entry;
    let source_status = entry.status;
    require!(
        source_status == RecycledStatus::Pending as u8
            || source_status == RecycledStatus::Lootbox as u8,
        ErrorCode::InvalidRecycledStatus
    );

    let pool_bump = ctx.accounts.inventory_pool.bump;
    let inventory_seeds_inner: &[&[u8]] = &[INVENTORY_POOL_SEED, &[pool_bump]];
    let signers: &[&[&[u8]]] = &[inventory_seeds_inner];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.marketplace_program.to_account_info(),
        degenbtc_market::cpi::accounts::ListNft {
            seller: ctx.accounts.inventory_pda.to_account_info(),
            marketplace_config: ctx.accounts.marketplace_config.to_account_info(),
            listing: ctx.accounts.marketplace_listing.to_account_info(),
            asset: ctx.accounts.doge_asset.to_account_info(),
            collection: ctx.accounts.doge_collection.to_account_info(),
            escrow: ctx.accounts.marketplace_escrow.to_account_info(),
            mpl_core_program: ctx.accounts.mpl_core_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        },
        signers,
    );
    degenbtc_market::cpi::list_nft(cpi_ctx, price_lamports)?;

    {
        let entry_mut = &mut ctx.accounts.recycled_entry;
        entry_mut.status = RecycledStatus::Listed as u8;
        entry_mut.listing_price = price_lamports;
    }

    let pool = &mut ctx.accounts.inventory_pool;
    if source_status == RecycledStatus::Pending as u8 {
        pool.pending_count = pool
            .pending_count
            .checked_sub(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    } else {
        pool.lootbox_count = pool
            .lootbox_count
            .checked_sub(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    pool.listed_count = pool
        .listed_count
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    pool.total_listed = pool
        .total_listed
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    emit!(InventoryListed {
        asset: ctx.accounts.doge_asset.key(),
        price_lamports,
        source_status,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct InventoryListNft<'info> {
    #[account(mut)]
    pub crank_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    /// CHECK: Inventory custody PDA — same address as `inventory_pool`.
    /// Signs as the marketplace seller via inventory PDA seeds.
    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pda: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [RECYCLED_ENTRY_SEED, doge_asset.key().as_ref()],
        bump = recycled_entry.bump,
    )]
    pub recycled_entry: Box<Account<'info, RecycledEntry>>,

    /// CHECK: mpl-core asset; current owner is `inventory_pda`.
    #[account(mut)]
    pub doge_asset: UncheckedAccount<'info>,

    /// CHECK: Doge collection — passed through to mpl-core for transfer.
    #[account(mut)]
    pub doge_collection: UncheckedAccount<'info>,

    /// CHECK: marketplace_config PDA inside the market program. Constraint
    /// enforced by `inventory_pool.marketplace_config`.
    #[account(mut)]
    pub marketplace_config: UncheckedAccount<'info>,

    /// CHECK: Listing PDA inside the market program — initialized by the CPI.
    #[account(mut)]
    pub marketplace_listing: UncheckedAccount<'info>,

    /// CHECK: Escrow PDA inside the market program — receives the asset via
    /// mpl-core TransferV1.
    #[account(mut)]
    pub marketplace_escrow: UncheckedAccount<'info>,

    /// CHECK: standalone marketplace program. Constraint enforced by
    /// `inventory_pool.marketplace_program`.
    pub marketplace_program: UncheckedAccount<'info>,

    /// CHECK: mpl-core program.
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

// ========================================================================================
// ============================== inventory_cancel_listing =================================
// ========================================================================================

pub fn internal_inventory_cancel_listing(ctx: Context<InventoryCancelListing>) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_inventory_cancel_listing");

    require_keys_eq!(
        ctx.accounts.crank_authority.key(),
        ctx.accounts.inventory_pool.crank_authority,
        ErrorCode::InvalidCrankAuthority
    );
    require_keys_eq!(
        ctx.accounts.marketplace_program.key(),
        ctx.accounts.inventory_pool.marketplace_program,
        ErrorCode::InvalidMarketplaceProgram
    );

    require!(
        ctx.accounts.recycled_entry.status == RecycledStatus::Listed as u8,
        ErrorCode::InvalidRecycledStatus
    );

    let pool_bump = ctx.accounts.inventory_pool.bump;
    let inventory_seeds_inner: &[&[u8]] = &[INVENTORY_POOL_SEED, &[pool_bump]];
    let signers: &[&[&[u8]]] = &[inventory_seeds_inner];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.marketplace_program.to_account_info(),
        degenbtc_market::cpi::accounts::CancelListing {
            seller: ctx.accounts.inventory_pda.to_account_info(),
            marketplace_config: ctx.accounts.marketplace_config.to_account_info(),
            listing: ctx.accounts.marketplace_listing.to_account_info(),
            asset: ctx.accounts.doge_asset.to_account_info(),
            collection: ctx.accounts.doge_collection.to_account_info(),
            escrow: ctx.accounts.marketplace_escrow.to_account_info(),
            mpl_core_program: ctx.accounts.mpl_core_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        },
        signers,
    );
    degenbtc_market::cpi::cancel_listing(cpi_ctx)?;

    {
        let entry_mut = &mut ctx.accounts.recycled_entry;
        entry_mut.status = RecycledStatus::Pending as u8;
        entry_mut.listing_price = 0;
    }

    let pool = &mut ctx.accounts.inventory_pool;
    pool.listed_count = pool
        .listed_count
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    pool.pending_count = pool
        .pending_count
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    emit!(InventoryCancelled {
        asset: ctx.accounts.doge_asset.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct InventoryCancelListing<'info> {
    #[account(mut)]
    pub crank_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    /// CHECK: Inventory custody PDA.
    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pda: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [RECYCLED_ENTRY_SEED, doge_asset.key().as_ref()],
        bump = recycled_entry.bump,
    )]
    pub recycled_entry: Box<Account<'info, RecycledEntry>>,

    /// CHECK: mpl-core asset.
    #[account(mut)]
    pub doge_asset: UncheckedAccount<'info>,

    /// CHECK: Doge collection.
    #[account(mut)]
    pub doge_collection: UncheckedAccount<'info>,

    /// CHECK: marketplace_config PDA.
    #[account(mut)]
    pub marketplace_config: UncheckedAccount<'info>,

    /// CHECK: Listing PDA — closed by the CPI; rent refund routes to inventory_pda.
    #[account(mut)]
    pub marketplace_listing: UncheckedAccount<'info>,

    /// CHECK: Escrow PDA.
    #[account(mut)]
    pub marketplace_escrow: UncheckedAccount<'info>,

    /// CHECK: standalone marketplace program.
    pub marketplace_program: UncheckedAccount<'info>,

    /// CHECK: mpl-core program.
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

// ========================================================================================
// ============================== inventory_update_price ===================================
// ========================================================================================

pub fn internal_inventory_update_price(
    ctx: Context<InventoryUpdatePrice>,
    new_price_lamports: u64,
) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_inventory_update_price");

    require_keys_eq!(
        ctx.accounts.crank_authority.key(),
        ctx.accounts.inventory_pool.crank_authority,
        ErrorCode::InvalidCrankAuthority
    );
    require_keys_eq!(
        ctx.accounts.marketplace_program.key(),
        ctx.accounts.inventory_pool.marketplace_program,
        ErrorCode::InvalidMarketplaceProgram
    );

    require!(
        ctx.accounts.recycled_entry.status == RecycledStatus::Listed as u8,
        ErrorCode::InvalidRecycledStatus
    );

    let pool_bump = ctx.accounts.inventory_pool.bump;
    let inventory_seeds_inner: &[&[u8]] = &[INVENTORY_POOL_SEED, &[pool_bump]];
    let signers: &[&[&[u8]]] = &[inventory_seeds_inner];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.marketplace_program.to_account_info(),
        degenbtc_market::cpi::accounts::UpdateListingPrice {
            seller: ctx.accounts.inventory_pda.to_account_info(),
            marketplace_config: ctx.accounts.marketplace_config.to_account_info(),
            listing: ctx.accounts.marketplace_listing.to_account_info(),
        },
        signers,
    );
    degenbtc_market::cpi::update_listing_price(cpi_ctx, new_price_lamports)?;

    ctx.accounts.recycled_entry.listing_price = new_price_lamports;

    emit!(InventoryPriceUpdated {
        asset: ctx.accounts.doge_asset.key(),
        price_lamports: new_price_lamports,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct InventoryUpdatePrice<'info> {
    #[account(mut)]
    pub crank_authority: Signer<'info>,

    #[account(
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    /// CHECK: Inventory custody PDA.
    #[account(
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pda: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [RECYCLED_ENTRY_SEED, doge_asset.key().as_ref()],
        bump = recycled_entry.bump,
    )]
    pub recycled_entry: Box<Account<'info, RecycledEntry>>,

    /// CHECK: mpl-core asset (read-only here, just used for the seed).
    pub doge_asset: UncheckedAccount<'info>,

    /// CHECK: marketplace_config PDA.
    pub marketplace_config: UncheckedAccount<'info>,

    /// CHECK: Listing PDA.
    #[account(mut)]
    pub marketplace_listing: UncheckedAccount<'info>,

    /// CHECK: standalone marketplace program.
    pub marketplace_program: UncheckedAccount<'info>,
}

// ========================================================================================
// ============================== inventory_set_lootbox ====================================
// ========================================================================================

pub fn internal_inventory_set_lootbox(ctx: Context<InventorySetLootbox>) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_inventory_set_lootbox");

    require_keys_eq!(
        ctx.accounts.crank_authority.key(),
        ctx.accounts.inventory_pool.crank_authority,
        ErrorCode::InvalidCrankAuthority
    );

    require!(
        ctx.accounts.recycled_entry.status == RecycledStatus::Pending as u8,
        ErrorCode::InvalidRecycledStatus
    );

    ctx.accounts.recycled_entry.status = RecycledStatus::Lootbox as u8;

    let pool = &mut ctx.accounts.inventory_pool;
    pool.pending_count = pool
        .pending_count
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    pool.lootbox_count = pool
        .lootbox_count
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    emit!(InventoryLootboxFlagged {
        asset: ctx.accounts.recycled_entry.asset,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct InventorySetLootbox<'info> {
    #[account(mut)]
    pub crank_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    #[account(
        mut,
        seeds = [RECYCLED_ENTRY_SEED, recycled_entry.asset.as_ref()],
        bump = recycled_entry.bump,
    )]
    pub recycled_entry: Box<Account<'info, RecycledEntry>>,
}

// ========================================================================================
// ============================== inventory_buy_listing (sweep) ============================
// ========================================================================================

pub fn internal_inventory_buy_listing(
    ctx: Context<InventoryBuyListing>,
    listing_price_lamports: u64,
) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_inventory_buy_listing");

    require_keys_eq!(
        ctx.accounts.crank_authority.key(),
        ctx.accounts.inventory_pool.crank_authority,
        ErrorCode::InvalidCrankAuthority
    );
    require_keys_eq!(
        ctx.accounts.marketplace_program.key(),
        ctx.accounts.inventory_pool.marketplace_program,
        ErrorCode::InvalidMarketplaceProgram
    );

    let listing_price = listing_price_lamports;

    // Capacity guard.
    require!(
        ctx.accounts.inventory_pool.total_count < MAX_INVENTORY,
        ErrorCode::InventoryFull
    );

    // Pre-fund inventory_pda from sweep vault. The marketplace's `buy_listing`
    // pulls SOL from the buyer (which is `inventory_pda` in this flow) via
    // a system::transfer, so the lamports must be on inventory_pda BEFORE
    // the CPI.
    {
        let sweep_seeds_inner: &[&[u8]] = &[
            INVENTORY_SWEEP_VAULT_SEED,
            &[ctx.bumps.inventory_sweep_vault],
        ];
        let sweep_signers: &[&[&[u8]]] = &[sweep_seeds_inner];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.inventory_sweep_vault.to_account_info(),
                to: ctx.accounts.inventory_pda.to_account_info(),
            },
            sweep_signers,
        );
        sys_prog::transfer(cpi_ctx, listing_price)?;
    }

    // CPI buy_listing as inventory_pda.
    let pool_bump = ctx.accounts.inventory_pool.bump;
    let inventory_seeds_inner: &[&[u8]] = &[INVENTORY_POOL_SEED, &[pool_bump]];
    let signers: &[&[&[u8]]] = &[inventory_seeds_inner];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.marketplace_program.to_account_info(),
        degenbtc_market::cpi::accounts::BuyListing {
            buyer: ctx.accounts.inventory_pda.to_account_info(),
            seller: ctx.accounts.seller.to_account_info(),
            marketplace_config: ctx.accounts.marketplace_config.to_account_info(),
            listing: ctx.accounts.marketplace_listing.to_account_info(),
            asset: ctx.accounts.doge_asset.to_account_info(),
            collection: ctx.accounts.doge_collection.to_account_info(),
            escrow: ctx.accounts.marketplace_escrow.to_account_info(),
            fee_recipient: ctx.accounts.fee_recipient.to_account_info(),
            mpl_core_program: ctx.accounts.mpl_core_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        },
        signers,
    );
    degenbtc_market::cpi::buy_listing(cpi_ctx)?;

    // Init RecycledEntry for the swept asset.
    {
        let entry = &mut ctx.accounts.recycled_entry;
        entry.bump = ctx.bumps.recycled_entry;
        entry.asset = ctx.accounts.doge_asset.key();
        entry.faction_id = ctx.accounts.doge_metadata.faction_id;
        entry.quality_score = compute_quality_score(
            ctx.accounts.doge_metadata.multiplier,
            ctx.accounts.doge_metadata.xp,
            ctx.accounts.doge_metadata.breed_count,
        );
        entry.recycled_at = Clock::get()?.unix_timestamp;
        entry.status = RecycledStatus::Pending as u8;
        entry.listing_price = 0;
        entry.origin = RecycledOrigin::Swept as u8;
    }

    let pool = &mut ctx.accounts.inventory_pool;
    pool.total_count = pool
        .total_count
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    pool.pending_count = pool
        .pending_count
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    pool.total_swept = pool
        .total_swept
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    emit!(InventorySwept {
        asset: ctx.accounts.doge_asset.key(),
        price_lamports: listing_price,
        seller: ctx.accounts.seller.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct InventoryBuyListing<'info> {
    #[account(mut)]
    pub crank_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    /// CHECK: Inventory custody PDA.
    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pda: UncheckedAccount<'info>,

    /// CHECK: SOL source for the sweep buy.
    #[account(
        mut,
        seeds = [INVENTORY_SWEEP_VAULT_SEED],
        bump,
    )]
    pub inventory_sweep_vault: UncheckedAccount<'info>,

    /// New entry created at sweep time. Payer = crank_authority (rent
    /// reclaimed when the entry closes via drop or sale).
    #[account(
        init,
        payer = crank_authority,
        space = RecycledEntry::LEN,
        seeds = [RECYCLED_ENTRY_SEED, doge_asset.key().as_ref()],
        bump,
    )]
    pub recycled_entry: Box<Account<'info, RecycledEntry>>,

    /// Doge metadata, read for quality score (multiplier/xp/breed_count).
    /// Faction match in claim/lootbox path uses this. Not mutated here —
    /// reset only happens when the asset transfers out of inventory.
    #[account(
        seeds = [DOGE_METADATA_SEED, doge_asset.key().as_ref()],
        bump = doge_metadata.bump,
    )]
    pub doge_metadata: Box<Account<'info, DogeMetadata>>,

    /// CHECK: mpl-core asset.
    #[account(mut)]
    pub doge_asset: UncheckedAccount<'info>,

    /// CHECK: Doge collection.
    #[account(mut)]
    pub doge_collection: UncheckedAccount<'info>,

    /// CHECK: marketplace_config PDA.
    pub marketplace_config: UncheckedAccount<'info>,

    /// CHECK: Listing PDA being purchased.
    #[account(mut)]
    pub marketplace_listing: UncheckedAccount<'info>,

    /// CHECK: Escrow PDA holding the asset.
    #[account(mut)]
    pub marketplace_escrow: UncheckedAccount<'info>,

    /// CHECK: Listing seller — receives proceeds.
    #[account(mut)]
    pub seller: UncheckedAccount<'info>,

    /// CHECK: Fee recipient (matches MarketplaceConfig.fee_recipient).
    #[account(mut)]
    pub fee_recipient: UncheckedAccount<'info>,

    /// CHECK: standalone marketplace program.
    pub marketplace_program: UncheckedAccount<'info>,

    /// CHECK: mpl-core program.
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

// ========================================================================================
// ============================== handle_inventory_proceeds ================================
// ========================================================================================

pub fn internal_handle_inventory_proceeds(ctx: Context<HandleInventoryProceeds>) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_handle_inventory_proceeds");

    let pool_info = ctx.accounts.inventory_pda.to_account_info();
    let rent_exempt = Rent::get()?.minimum_balance(pool_info.data_len());
    let available = pool_info.lamports().saturating_sub(rent_exempt);
    if available == 0 {
        msg!("⚠️ No inventory proceeds to route");
        return Ok(());
    }

    let to_sweep = (available as u128 * INVENTORY_SWEEP_RESERVE_BPS as u128 / 10_000) as u64;
    let to_protocol = available
        .checked_sub(to_sweep)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    let pool_bump = ctx.accounts.inventory_pool.bump;
    let inventory_seeds_inner: &[&[u8]] = &[INVENTORY_POOL_SEED, &[pool_bump]];
    let signers: &[&[&[u8]]] = &[inventory_seeds_inner];

    if to_sweep > 0 {
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.inventory_pda.to_account_info(),
                to: ctx.accounts.inventory_sweep_vault.to_account_info(),
            },
            signers,
        );
        sys_prog::transfer(cpi_ctx, to_sweep)?;
    }

    if to_protocol > 0 {
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.inventory_pda.to_account_info(),
                to: ctx.accounts.sol_treasury.to_account_info(),
            },
            signers,
        );
        sys_prog::transfer(cpi_ctx, to_protocol)?;
    }

    let pool = &mut ctx.accounts.inventory_pool;
    pool.total_sold = pool.total_sold.saturating_add(1);

    emit!(InventoryProceedsRouted {
        to_sweep,
        to_protocol,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct HandleInventoryProceeds<'info> {
    #[account(mut)]
    pub crank_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
        constraint = inventory_pool.crank_authority == crank_authority.key() @ ErrorCode::InvalidCrankAuthority,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    /// CHECK: Inventory PDA — same address as inventory_pool. Source of
    /// SOL drained by this ix.
    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pda: UncheckedAccount<'info>,

    /// CHECK: SOL destination for sweep reserve.
    #[account(
        mut,
        seeds = [INVENTORY_SWEEP_VAULT_SEED],
        bump,
    )]
    pub inventory_sweep_vault: UncheckedAccount<'info>,

    /// CHECK: SOL treasury PDA — protocol pipeline picks up from here via
    /// `economy.rs::distribute_sol_fees_internal`.
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED],
        bump,
    )]
    pub sol_treasury: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

// ========================================================================================
// ============================== update_market_metrics ====================================
// ========================================================================================

pub fn internal_update_market_metrics(
    ctx: Context<UpdateMarketMetrics>,
    demand_index: i16,
    floor_price_lamports: u64,
    avg_sell_price_24h: u64,
    listings_count: u32,
    sales_count_24h: u32,
) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_update_market_metrics");

    require!(
        demand_index >= -100 && demand_index <= 100,
        ErrorCode::DemandIndexOutOfRange
    );
    require_keys_eq!(
        ctx.accounts.crank_authority.key(),
        ctx.accounts.market_metrics.crank_authority,
        ErrorCode::InvalidCrankAuthority
    );

    let metrics = &mut ctx.accounts.market_metrics;
    metrics.demand_index = demand_index;
    metrics.floor_price_lamports = floor_price_lamports;
    metrics.avg_sell_price_24h = avg_sell_price_24h;
    metrics.listings_count = listings_count;
    metrics.sales_count_24h = sales_count_24h;
    let now = Clock::get()?.unix_timestamp;
    metrics.last_updated = now;

    emit!(MarketMetricsUpdated {
        demand_index,
        floor_price_lamports,
        avg_sell_price_24h,
        listings_count,
        sales_count_24h,
        timestamp: now,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateMarketMetrics<'info> {
    pub crank_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [MARKET_METRICS_SEED],
        bump = market_metrics.bump,
    )]
    pub market_metrics: Box<Account<'info, MarketMetrics>>,
}

// ========================================================================================
// ============================== inventory_finalize_sale ==================================
// ========================================================================================

/// Called by the cranker after observing a marketplace `NftSold` event where
/// `seller == inventory_pda`. Closes the corresponding `RecycledEntry` (rent
/// refunded to inventory_pda where it joins sale proceeds for the next
/// `handle_inventory_proceeds` sweep) and decrements pool counters.
///
/// We can't enforce that the sale actually happened on-chain without re-doing
/// the marketplace's work, but the indexer + cranker contract is one-way: only
/// finalize entries whose asset is no longer owned by inventory_pda. The
/// cranker checks the asset's owner pre-call.
///
/// Idempotency note: if this is called twice for the same entry, the second
/// call fails because the RecycledEntry account has been closed.
pub fn internal_inventory_finalize_sale(
    ctx: Context<InventoryFinalizeSale>,
    price_lamports: u64,
    fee_lamports: u64,
    buyer: Pubkey,
) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_inventory_finalize_sale");

    require_keys_eq!(
        ctx.accounts.crank_authority.key(),
        ctx.accounts.inventory_pool.crank_authority,
        ErrorCode::InvalidCrankAuthority
    );

    require!(
        ctx.accounts.recycled_entry.status == RecycledStatus::Listed as u8,
        ErrorCode::InvalidRecycledStatus
    );

    let asset = ctx.accounts.recycled_entry.asset;

    // Decrement pool counters.
    {
        let pool = &mut ctx.accounts.inventory_pool;
        pool.listed_count = pool
            .listed_count
            .checked_sub(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        pool.total_count = pool
            .total_count
            .checked_sub(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        pool.total_sold = pool
            .total_sold
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    emit!(InventorySaleFinalized {
        asset,
        buyer,
        price_lamports,
        fee_lamports,
        timestamp: Clock::get()?.unix_timestamp,
    });

    // Anchor closes `recycled_entry` to `inventory_pda` at instruction exit
    // (rent joins the sale proceeds already on inventory_pda).
    Ok(())
}

#[derive(Accounts)]
pub struct InventoryFinalizeSale<'info> {
    #[account(mut)]
    pub crank_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    /// CHECK: Inventory PDA — receives the closed-account rent refund.
    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pda: UncheckedAccount<'info>,

    #[account(
        mut,
        close = inventory_pda,
        seeds = [RECYCLED_ENTRY_SEED, recycled_entry.asset.as_ref()],
        bump = recycled_entry.bump,
    )]
    pub recycled_entry: Box<Account<'info, RecycledEntry>>,
}
