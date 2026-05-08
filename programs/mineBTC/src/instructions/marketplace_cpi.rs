// # Marketplace CPI + Permissionless Market Making
//
// This module is the on-chain market maker for the program-owned HashBeast inventory.
// The crank-gated flow is gone; everything here is either user-signed or
// permissionless (anyone can call, with a small keeper bounty paid from
// `inventory_sweep_vault` where applicable).
//
// User-signed (asset owner is signer):
// - `list_user_nft` / `cancel_user_listing` / `update_user_listing_price`
//   — wrap the marketplace ix and keep `FloorQueue` in sync atomically.
// - `buy_user_listing` — wraps `degenbtc_market::buy_listing`, also records
//   the sale into `SaleHistory` if it qualifies as a real-demand signal.
//
// Permissionless:
// - `register_floor_listing` — anyone pushes an existing user listing into
//   the sorted `FloorQueue`. Bots will register their own listings + others
//   to stay competitive for sweep keeper rewards.
// - `sweep_floor_lowest` — buys queue.entries[0], or purges a stale head so
//   bots can retry, then disposes a live sweep (queue -> relist -> burn).
// - `record_floor_snapshot` — daily snapshot using qualifying user-to-user
//   sale median, with floor-queue median as low-volume fallback.
// - `expire_program_listing` — 7-day TTL trigger that re-disposes a stuck
//   inventory listing (relist with progressive discount, or burn).
// - `handle_inventory_proceeds` — splits accrued sale proceeds 50/50 to
//   sweep_vault and sol_treasury.
// - `inventory_finalize_sale` — closes a sold inventory `RebornEntry`
//   after verifying the asset's owner is no longer `inventory_pda`.
// - `claim_lootbox_nft` — delivers a reserved loser-roll HashBeast to its
//   recorded winner.

use anchor_lang::prelude::*;
use anchor_lang::system_program::{self as sys_prog, Transfer};
use mpl_core::{accounts::BaseAssetV1, types::UpdateAuthority};

use crate::errors::ErrorCode;
use crate::events::*;
use crate::state::*;

// ========================================================================================
// ============================== Helpers (private) =======================================
// ========================================================================================

/// Insert a new floor entry into the queue, maintaining ascending sort.
/// Evicts the worst entry if the queue is full and the new entry is cheaper.
/// Returns the inserted index, or `FloorQueueFull` if not cheap enough.
fn insert_floor_entry(queue: &mut FloorQueue, entry: FloorEntry) -> Result<u8> {
    // Reject duplicates by asset.
    for i in 0..(queue.entries_count as usize) {
        require!(
            queue.entries[i].asset != entry.asset,
            ErrorCode::AssetAlreadyInQueue
        );
    }
    let count = queue.entries_count as usize;
    if count >= FLOOR_QUEUE_SIZE {
        // Queue full: must beat the worst (last) entry.
        let worst_price = queue.entries[FLOOR_QUEUE_SIZE - 1].price;
        require!(entry.price < worst_price, ErrorCode::FloorQueueFull);
    }
    let mut insert_idx = count;
    for i in 0..count {
        if entry.price < queue.entries[i].price {
            insert_idx = i;
            break;
        }
    }
    // Shift right to make room.
    let end = if count >= FLOOR_QUEUE_SIZE {
        FLOOR_QUEUE_SIZE - 1
    } else {
        count
    };
    let mut j = end;
    while j > insert_idx {
        queue.entries[j] = queue.entries[j - 1];
        j -= 1;
    }
    queue.entries[insert_idx] = entry;
    if (queue.entries_count as usize) < FLOOR_QUEUE_SIZE {
        queue.entries_count = queue.entries_count.saturating_add(1);
    }
    Ok(insert_idx as u8)
}

fn assert_marketplace_config(info: &AccountInfo, expected: Pubkey) -> Result<()> {
    require_keys_eq!(info.key(), expected, ErrorCode::InvalidMarketplaceConfig);
    require!(
        info.owner == &degenbtc_market::ID,
        ErrorCode::InvalidAccount
    );
    Ok(())
}

fn assert_listing_pda(listing: Pubkey, marketplace_config: Pubkey, asset: Pubkey) -> Result<()> {
    let (expected_listing, _) = Pubkey::find_program_address(
        &[
            degenbtc_market::state::LISTING_SEED,
            marketplace_config.as_ref(),
            asset.as_ref(),
        ],
        &degenbtc_market::ID,
    );
    require_keys_eq!(listing, expected_listing, ErrorCode::InvalidAccount);
    Ok(())
}

fn read_marketplace_config_collection(config_info: &AccountInfo) -> Result<Pubkey> {
    require!(
        config_info.owner == &degenbtc_market::ID,
        ErrorCode::InvalidAccount
    );
    let data = config_info.try_borrow_data()?;
    let mut slice: &[u8] = &data;
    let config = degenbtc_market::state::MarketplaceConfig::try_deserialize(&mut slice)?;
    Ok(config.collection_mint)
}

/// Reads `min_price_lamports` from a marketplace_config account.
fn read_marketplace_min_price(config_info: &AccountInfo) -> Result<u64> {
    require!(
        config_info.owner == &degenbtc_market::ID,
        ErrorCode::InvalidAccount
    );
    let data = config_info.try_borrow_data()?;
    let mut slice: &[u8] = &data;
    let config = degenbtc_market::state::MarketplaceConfig::try_deserialize(&mut slice)?;
    Ok(config.min_price_lamports)
}

fn assert_asset_collection(asset_info: &AccountInfo, collection: Pubkey) -> Result<()> {
    let asset_data: BaseAssetV1 =
        BaseAssetV1::try_from(asset_info).map_err(|_| ErrorCode::InvalidAccount)?;
    match asset_data.update_authority {
        UpdateAuthority::Collection(asset_collection) => {
            require_keys_eq!(asset_collection, collection, ErrorCode::InvalidAccount);
            Ok(())
        }
        _ => err!(ErrorCode::InvalidAccount),
    }
}

fn current_rebirth_count(metadata: &HashBeastMetadata) -> u8 {
    metadata
        .rebirth_count
        .max(crate::genescience::get_rebirth_count(&metadata.dna))
}

/// Linearly scan the queue for an entry matching `asset`. Returns its index.
fn find_floor_entry_by_asset(queue: &FloorQueue, asset: Pubkey) -> Option<u8> {
    for i in 0..(queue.entries_count as usize) {
        if queue.entries[i].asset == asset {
            return Some(i as u8);
        }
    }
    None
}

/// Remove the entry at `idx`, shifting everything after it left.
fn remove_floor_entry_at(queue: &mut FloorQueue, idx: u8) {
    let count = queue.entries_count as usize;
    let i = idx as usize;
    if i >= count {
        return;
    }
    for j in i..(count - 1) {
        queue.entries[j] = queue.entries[j + 1];
    }
    queue.entries[count - 1] = FloorEntry::default();
    queue.entries_count = (count - 1) as u8;
}

const FLOOR_QUEUE_COUNT_OFFSET: usize = DISCRIMINATOR_SIZE + 1;
const FLOOR_QUEUE_ENTRIES_OFFSET: usize = DISCRIMINATOR_SIZE + 2;

const SALE_HISTORY_HEAD_OFFSET: usize = DISCRIMINATOR_SIZE;
const SALE_HISTORY_ENTRIES_OFFSET: usize = DISCRIMINATOR_SIZE + 1;

const FLOOR_HISTORY_HEAD_OFFSET: usize = DISCRIMINATOR_SIZE + 1;
const FLOOR_HISTORY_LAST_SNAPSHOT_OFFSET: usize = DISCRIMINATOR_SIZE + 2;
const FLOOR_HISTORY_SNAPSHOTS_OFFSET: usize = DISCRIMINATOR_SIZE + 10;

fn require_raw_account<T: Discriminator>(info: &AccountInfo<'_>, min_len: usize) -> Result<()> {
    require!(info.owner == &crate::ID, ErrorCode::InvalidAccount);
    let data = info.try_borrow_data()?;
    require!(data.len() >= min_len, ErrorCode::InvalidAccount);
    require!(
        data[..DISCRIMINATOR_SIZE] == T::DISCRIMINATOR[..],
        ErrorCode::InvalidAccount
    );
    Ok(())
}

fn read_u8_at(data: &[u8], offset: usize) -> Result<u8> {
    Ok(*data.get(offset).ok_or(ErrorCode::InvalidAccount)?)
}

fn write_u8_at(data: &mut [u8], offset: usize, value: u8) -> Result<()> {
    *data.get_mut(offset).ok_or(ErrorCode::InvalidAccount)? = value;
    Ok(())
}

fn read_u64_at(data: &[u8], offset: usize) -> Result<u64> {
    let bytes = data
        .get(offset..offset + 8)
        .ok_or(ErrorCode::InvalidAccount)?;
    let mut out = [0u8; 8];
    out.copy_from_slice(bytes);
    Ok(u64::from_le_bytes(out))
}

fn write_u64_at(data: &mut [u8], offset: usize, value: u64) -> Result<()> {
    data.get_mut(offset..offset + 8)
        .ok_or(ErrorCode::InvalidAccount)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn read_i64_at(data: &[u8], offset: usize) -> Result<i64> {
    let bytes = data
        .get(offset..offset + 8)
        .ok_or(ErrorCode::InvalidAccount)?;
    let mut out = [0u8; 8];
    out.copy_from_slice(bytes);
    Ok(i64::from_le_bytes(out))
}

fn write_i64_at(data: &mut [u8], offset: usize, value: i64) -> Result<()> {
    data.get_mut(offset..offset + 8)
        .ok_or(ErrorCode::InvalidAccount)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn read_pubkey_at(data: &[u8], offset: usize) -> Result<Pubkey> {
    let bytes = data
        .get(offset..offset + 32)
        .ok_or(ErrorCode::InvalidAccount)?;
    let mut out = [0u8; 32];
    out.copy_from_slice(bytes);
    Ok(Pubkey::new_from_array(out))
}

fn write_pubkey_at(data: &mut [u8], offset: usize, value: Pubkey) -> Result<()> {
    data.get_mut(offset..offset + 32)
        .ok_or(ErrorCode::InvalidAccount)?
        .copy_from_slice(value.as_ref());
    Ok(())
}

fn raw_floor_entry_offset(idx: usize) -> Result<usize> {
    require!(idx < FLOOR_QUEUE_SIZE, ErrorCode::InvalidAccount);
    FLOOR_QUEUE_ENTRIES_OFFSET
        .checked_add(
            idx.checked_mul(FloorEntry::SIZE)
                .ok_or(ErrorCode::ArithmeticOverflow)?,
        )
        .ok_or(ErrorCode::ArithmeticOverflow.into())
}

fn read_raw_floor_entry(data: &[u8], idx: usize) -> Result<FloorEntry> {
    let offset = raw_floor_entry_offset(idx)?;
    Ok(FloorEntry {
        listing: read_pubkey_at(data, offset)?,
        asset: read_pubkey_at(data, offset + 32)?,
        seller: read_pubkey_at(data, offset + 64)?,
        price: read_u64_at(data, offset + 96)?,
        registered_at: read_i64_at(data, offset + 104)?,
    })
}

fn write_raw_floor_entry(data: &mut [u8], idx: usize, entry: FloorEntry) -> Result<()> {
    let offset = raw_floor_entry_offset(idx)?;
    write_pubkey_at(data, offset, entry.listing)?;
    write_pubkey_at(data, offset + 32, entry.asset)?;
    write_pubkey_at(data, offset + 64, entry.seller)?;
    write_u64_at(data, offset + 96, entry.price)?;
    write_i64_at(data, offset + 104, entry.registered_at)?;
    Ok(())
}

fn raw_find_floor_entry_by_asset(
    queue_info: &AccountInfo<'_>,
    asset: Pubkey,
) -> Result<Option<(u8, FloorEntry)>> {
    require_raw_account::<FloorQueue>(queue_info, FloorQueue::LEN)?;
    let data = queue_info.try_borrow_data()?;
    let count = read_u8_at(&data, FLOOR_QUEUE_COUNT_OFFSET)?;
    require!(
        (count as usize) <= FLOOR_QUEUE_SIZE,
        ErrorCode::InvalidAccount
    );
    for i in 0..(count as usize) {
        let entry = read_raw_floor_entry(&data, i)?;
        if entry.asset == asset {
            return Ok(Some((i as u8, entry)));
        }
    }
    Ok(None)
}

fn raw_floor_queue_head(queue_info: &AccountInfo<'_>) -> Result<FloorEntry> {
    require_raw_account::<FloorQueue>(queue_info, FloorQueue::LEN)?;
    let data = queue_info.try_borrow_data()?;
    let count = read_u8_at(&data, FLOOR_QUEUE_COUNT_OFFSET)?;
    require!(count > 0, ErrorCode::FloorQueueEmpty);
    require!(
        (count as usize) <= FLOOR_QUEUE_SIZE,
        ErrorCode::InvalidAccount
    );
    read_raw_floor_entry(&data, 0)
}

fn raw_remove_floor_entry_at(queue_info: &AccountInfo<'_>, idx: u8) -> Result<()> {
    require_raw_account::<FloorQueue>(queue_info, FloorQueue::LEN)?;
    let mut data = queue_info.try_borrow_mut_data()?;
    let count = read_u8_at(&data, FLOOR_QUEUE_COUNT_OFFSET)? as usize;
    require!(count <= FLOOR_QUEUE_SIZE, ErrorCode::InvalidAccount);
    let i = idx as usize;
    if i >= count {
        return Ok(());
    }
    for j in i..(count - 1) {
        let next = read_raw_floor_entry(&data, j + 1)?;
        write_raw_floor_entry(&mut data, j, next)?;
    }
    write_raw_floor_entry(&mut data, count - 1, FloorEntry::default())?;
    write_u8_at(&mut data, FLOOR_QUEUE_COUNT_OFFSET, (count - 1) as u8)?;
    Ok(())
}

fn raw_floor_queue_median(queue_info: &AccountInfo<'_>) -> Result<(u64, u32)> {
    require_raw_account::<FloorQueue>(queue_info, FloorQueue::LEN)?;
    let data = queue_info.try_borrow_data()?;
    let count = read_u8_at(&data, FLOOR_QUEUE_COUNT_OFFSET)? as usize;
    require!(count <= FLOOR_QUEUE_SIZE, ErrorCode::InvalidAccount);
    if count == 0 {
        return Ok((0, 0));
    }
    let entry = read_raw_floor_entry(&data, count / 2)?;
    Ok((entry.price, count as u32))
}

fn raw_sale_entry_offset(idx: usize) -> Result<usize> {
    require!(idx < SALE_HISTORY_SIZE, ErrorCode::InvalidAccount);
    SALE_HISTORY_ENTRIES_OFFSET
        .checked_add(
            idx.checked_mul(SaleEntry::SIZE)
                .ok_or(ErrorCode::ArithmeticOverflow)?,
        )
        .ok_or(ErrorCode::ArithmeticOverflow.into())
}

fn write_raw_sale_entry(data: &mut [u8], idx: usize, entry: SaleEntry) -> Result<()> {
    let offset = raw_sale_entry_offset(idx)?;
    write_pubkey_at(data, offset, entry.asset)?;
    write_u64_at(data, offset + 32, entry.price)?;
    write_i64_at(data, offset + 40, entry.listed_at)?;
    write_i64_at(data, offset + 48, entry.sold_at)?;
    write_pubkey_at(data, offset + 56, entry.buyer)?;
    write_pubkey_at(data, offset + 88, entry.seller)?;
    Ok(())
}

fn raw_record_sale(sales_info: &AccountInfo<'_>, entry: SaleEntry) -> Result<()> {
    require_raw_account::<SaleHistory>(sales_info, SaleHistory::LEN)?;
    let mut data = sales_info.try_borrow_mut_data()?;
    let head = (read_u8_at(&data, SALE_HISTORY_HEAD_OFFSET)? as usize) % SALE_HISTORY_SIZE;
    write_raw_sale_entry(&mut data, head, entry)?;
    write_u8_at(
        &mut data,
        SALE_HISTORY_HEAD_OFFSET,
        ((head + 1) % SALE_HISTORY_SIZE) as u8,
    )?;
    Ok(())
}

fn raw_compute_snapshot_anchor(
    sales_info: &AccountInfo<'_>,
    queue_info: &AccountInfo<'_>,
    now: i64,
) -> Result<(u64, u8, u32)> {
    require_raw_account::<SaleHistory>(sales_info, SaleHistory::LEN)?;
    let sales_data = sales_info.try_borrow_data()?;
    let mut prices: [u64; SALE_HISTORY_SIZE] = [0u64; SALE_HISTORY_SIZE];
    let mut n = 0usize;
    for i in 0..SALE_HISTORY_SIZE {
        let offset = raw_sale_entry_offset(i)?;
        let price = read_u64_at(&sales_data, offset + 32)?;
        let listed_at = read_i64_at(&sales_data, offset + 40)?;
        let sold_at = read_i64_at(&sales_data, offset + 48)?;
        if price == 0 || sold_at == 0 {
            continue;
        }
        if now.saturating_sub(sold_at) > SALE_RECENT_WINDOW_SECS {
            continue;
        }
        if sold_at - listed_at < SALE_QUALIFY_MIN_LISTING_AGE_SECS {
            continue;
        }
        prices[n] = price;
        n += 1;
    }
    drop(sales_data);

    if n >= MIN_SALES_FOR_ANCHOR {
        prices[..n].sort_unstable();
        return Ok((prices[n / 2], 0, n as u32));
    }

    let (queue_median, queue_count) = raw_floor_queue_median(queue_info)?;
    Ok((queue_median, 1, queue_count))
}

fn raw_floor_snapshot_offset(idx: usize) -> Result<usize> {
    require!(idx < FLOOR_HISTORY_SIZE, ErrorCode::InvalidAccount);
    FLOOR_HISTORY_SNAPSHOTS_OFFSET
        .checked_add(
            idx.checked_mul(FloorSnapshot::SIZE)
                .ok_or(ErrorCode::ArithmeticOverflow)?,
        )
        .ok_or(ErrorCode::ArithmeticOverflow.into())
}

fn raw_floor_history_last_snapshot_at(history_info: &AccountInfo<'_>) -> Result<i64> {
    require_raw_account::<FloorHistory>(history_info, FloorHistory::LEN)?;
    let data = history_info.try_borrow_data()?;
    read_i64_at(&data, FLOOR_HISTORY_LAST_SNAPSHOT_OFFSET)
}

fn raw_record_floor_snapshot(
    history_info: &AccountInfo<'_>,
    timestamp: i64,
    anchor_price: u64,
) -> Result<()> {
    require_raw_account::<FloorHistory>(history_info, FloorHistory::LEN)?;
    let mut data = history_info.try_borrow_mut_data()?;
    let head = (read_u8_at(&data, FLOOR_HISTORY_HEAD_OFFSET)? as usize) % FLOOR_HISTORY_SIZE;
    let next_head = (head + 1) % FLOOR_HISTORY_SIZE;
    write_u8_at(&mut data, FLOOR_HISTORY_HEAD_OFFSET, next_head as u8)?;
    let offset = raw_floor_snapshot_offset(next_head)?;
    write_i64_at(&mut data, offset, timestamp)?;
    write_u64_at(&mut data, offset + 8, anchor_price)?;
    write_i64_at(&mut data, FLOOR_HISTORY_LAST_SNAPSHOT_OFFSET, timestamp)?;
    Ok(())
}

fn read_raw_floor_snapshot(data: &[u8], idx: usize) -> Result<FloorSnapshot> {
    let offset = raw_floor_snapshot_offset(idx)?;
    Ok(FloorSnapshot {
        timestamp: read_i64_at(data, offset)?,
        anchor_price: read_u64_at(data, offset + 8)?,
    })
}

fn raw_floor_history_current_anchor(history_info: &AccountInfo<'_>) -> Result<u64> {
    require_raw_account::<FloorHistory>(history_info, FloorHistory::LEN)?;
    let data = history_info.try_borrow_data()?;
    let head = (read_u8_at(&data, FLOOR_HISTORY_HEAD_OFFSET)? as usize) % FLOOR_HISTORY_SIZE;
    Ok(read_raw_floor_snapshot(&data, head)?.anchor_price)
}

fn raw_floor_history_compute_trend_bps(history_info: &AccountInfo<'_>) -> Result<i32> {
    require_raw_account::<FloorHistory>(history_info, FloorHistory::LEN)?;
    let data = history_info.try_borrow_data()?;
    let head_idx = (read_u8_at(&data, FLOOR_HISTORY_HEAD_OFFSET)? as usize) % FLOOR_HISTORY_SIZE;
    let mut oldest_idx = (head_idx + 1) % FLOOR_HISTORY_SIZE;
    let mut found_oldest = false;
    for _ in 0..FLOOR_HISTORY_SIZE {
        if read_raw_floor_snapshot(&data, oldest_idx)?.anchor_price > 0 {
            found_oldest = true;
            break;
        }
        oldest_idx = (oldest_idx + 1) % FLOOR_HISTORY_SIZE;
    }
    if !found_oldest || oldest_idx == head_idx {
        return Ok(0);
    }
    let oldest = read_raw_floor_snapshot(&data, oldest_idx)?.anchor_price as i128;
    let newest = read_raw_floor_snapshot(&data, head_idx)?.anchor_price as i128;
    if oldest == 0 {
        return Ok(0);
    }
    let bps = ((newest - oldest) * 10_000) / oldest;
    Ok(bps.clamp(-10_000, 10_000) as i32)
}

/// Pre-fund inventory_pda from sweep_vault by `lamports`. The vault PDA signs
/// via its seeds.
fn fund_inventory_from_sweep_vault<'info>(
    sweep_vault: &AccountInfo<'info>,
    inventory_pda: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    sweep_bump: u8,
    lamports: u64,
) -> Result<()> {
    let seeds_inner: &[&[u8]] = &[INVENTORY_SWEEP_VAULT_SEED, &[sweep_bump]];
    let signers: &[&[&[u8]]] = &[seeds_inner];
    let cpi_ctx = CpiContext::new_with_signer(
        system_program.to_account_info(),
        Transfer {
            from: sweep_vault.to_account_info(),
            to: inventory_pda.to_account_info(),
        },
        signers,
    );
    sys_prog::transfer(cpi_ctx, lamports)
}

/// Pay `lamports` keeper bounty out of sweep_vault.
fn pay_keeper<'info>(
    sweep_vault: &AccountInfo<'info>,
    keeper: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    sweep_bump: u8,
    lamports: u64,
) -> Result<()> {
    if lamports == 0 {
        return Ok(());
    }
    let reserve_floor = Rent::get()?
        .minimum_balance(0)
        .max(MIN_SWEEP_RESERVE_LAMPORTS);
    let available = sweep_vault.lamports().saturating_sub(reserve_floor);
    let amt = lamports.min(available);
    if amt == 0 {
        return Ok(());
    }
    let seeds_inner: &[&[u8]] = &[INVENTORY_SWEEP_VAULT_SEED, &[sweep_bump]];
    let signers: &[&[&[u8]]] = &[seeds_inner];
    let cpi_ctx = CpiContext::new_with_signer(
        system_program.to_account_info(),
        Transfer {
            from: sweep_vault.to_account_info(),
            to: keeper.to_account_info(),
        },
        signers,
    );
    sys_prog::transfer(cpi_ctx, amt)
}

/// Read mpl-core asset's current owner.
fn read_asset_owner(asset: &AccountInfo) -> Result<Pubkey> {
    let asset_data: BaseAssetV1 =
        BaseAssetV1::try_from(asset).map_err(|_| ErrorCode::InvalidAccount)?;
    Ok(asset_data.owner)
}

// ========================================================================================
// ============================== list_user_nft ===========================================
// ========================================================================================

/// User wraps `degenbtc_market::list_nft` and atomically registers the new
/// listing into `FloorQueue`. Permissionless from the protocol's POV — the
/// caller must own the asset and signs the listing.
pub fn internal_list_user_nft(ctx: Context<ListUserNft>, price_lamports: u64) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_list_user_nft");

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

    // Caller must not be inventory_pda — that path is internal-only.
    require!(
        ctx.accounts.seller.key() != ctx.accounts.inventory_pool.key(),
        ErrorCode::ProgramListingNotAllowed
    );

    let cpi_ctx = CpiContext::new(
        ctx.accounts.marketplace_program.to_account_info(),
        degenbtc_market::cpi::accounts::ListNft {
            seller: ctx.accounts.seller.to_account_info(),
            marketplace_config: ctx.accounts.marketplace_config.to_account_info(),
            listing: ctx.accounts.marketplace_listing.to_account_info(),
            asset: ctx.accounts.hashbeast_asset.to_account_info(),
            collection: ctx.accounts.hashbeast_collection.to_account_info(),
            escrow: ctx.accounts.marketplace_escrow.to_account_info(),
            mpl_core_program: ctx.accounts.mpl_core_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        },
    );
    degenbtc_market::cpi::list_nft(cpi_ctx, price_lamports)?;

    let now = Clock::get()?.unix_timestamp;
    let entry = FloorEntry {
        listing: ctx.accounts.marketplace_listing.key(),
        asset: ctx.accounts.hashbeast_asset.key(),
        seller: ctx.accounts.seller.key(),
        price: price_lamports,
        registered_at: now,
    };

    let queue = &mut ctx.accounts.floor_queue;
    let inserted = insert_floor_entry(queue, entry);

    // Floor-queue insertion is best-effort — if the queue is full and the
    // new price isn't competitive, that's not a failure to list. Log and
    // continue.
    let inserted_idx = match inserted {
        Ok(idx) => idx,
        Err(_) => {
            msg!("ℹ️  Floor queue full; listing not registered (price too high)");
            return Ok(());
        }
    };

    emit!(FloorEntryRegistered {
        listing: entry.listing,
        asset: entry.asset,
        seller: entry.seller,
        price: entry.price,
        queue_index: inserted_idx,
        queue_size_after: queue.entries_count,
        timestamp: now,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct ListUserNft<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,

    #[account(
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    #[account(
        mut,
        seeds = [FLOOR_QUEUE_SEED],
        bump = floor_queue.bump,
    )]
    pub floor_queue: Box<Account<'info, FloorQueue>>,

    /// CHECK: marketplace_config PDA, address-checked against pool cache.
    #[account(mut)]
    pub marketplace_config: UncheckedAccount<'info>,

    /// CHECK: Listing PDA — initialized by the CPI.
    #[account(mut)]
    pub marketplace_listing: UncheckedAccount<'info>,

    /// CHECK: mpl-core asset; transferred to escrow by the CPI.
    #[account(mut)]
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// CHECK: HashBeast collection.
    #[account(mut)]
    pub hashbeast_collection: UncheckedAccount<'info>,

    /// CHECK: Escrow PDA inside the market program.
    #[account(mut)]
    pub marketplace_escrow: UncheckedAccount<'info>,

    /// CHECK: standalone marketplace program. Constraint enforced by pool cache.
    pub marketplace_program: UncheckedAccount<'info>,

    /// CHECK: mpl-core program.
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

// ========================================================================================
// ============================== cancel_user_listing =====================================
// ========================================================================================

pub fn internal_cancel_user_listing(ctx: Context<CancelUserListing>) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_cancel_user_listing");

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
    let asset_key = ctx.accounts.hashbeast_asset.key();

    let cpi_ctx = CpiContext::new(
        ctx.accounts.marketplace_program.to_account_info(),
        degenbtc_market::cpi::accounts::CancelListing {
            seller: ctx.accounts.seller.to_account_info(),
            marketplace_config: ctx.accounts.marketplace_config.to_account_info(),
            listing: ctx.accounts.marketplace_listing.to_account_info(),
            asset: ctx.accounts.hashbeast_asset.to_account_info(),
            collection: ctx.accounts.hashbeast_collection.to_account_info(),
            escrow: ctx.accounts.marketplace_escrow.to_account_info(),
            mpl_core_program: ctx.accounts.mpl_core_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        },
    );
    degenbtc_market::cpi::cancel_listing(cpi_ctx)?;

    let queue = &mut ctx.accounts.floor_queue;
    if let Some(idx) = find_floor_entry_by_asset(queue, asset_key) {
        let listing_key = queue.entries[idx as usize].listing;
        remove_floor_entry_at(queue, idx);
        emit!(FloorEntryRemoved {
            listing: listing_key,
            asset: asset_key,
            queue_index: idx,
            reason: 1, // cancel
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    Ok(())
}

#[derive(Accounts)]
pub struct CancelUserListing<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,

    #[account(
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    #[account(
        mut,
        seeds = [FLOOR_QUEUE_SEED],
        bump = floor_queue.bump,
    )]
    pub floor_queue: Box<Account<'info, FloorQueue>>,

    /// CHECK: marketplace_config PDA.
    #[account(mut)]
    pub marketplace_config: UncheckedAccount<'info>,

    /// CHECK: Listing PDA — closed by CPI.
    #[account(mut)]
    pub marketplace_listing: UncheckedAccount<'info>,

    /// CHECK: mpl-core asset.
    #[account(mut)]
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// CHECK: HashBeast collection.
    #[account(mut)]
    pub hashbeast_collection: UncheckedAccount<'info>,

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
// ============================== update_user_listing_price ===============================
// ========================================================================================

pub fn internal_update_user_listing_price(
    ctx: Context<UpdateUserListingPrice>,
    new_price_lamports: u64,
) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_update_user_listing_price");

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
    let asset_key = ctx.accounts.hashbeast_asset.key();
    let listing_key = ctx.accounts.marketplace_listing.key();
    let seller_key = ctx.accounts.seller.key();

    let cpi_ctx = CpiContext::new(
        ctx.accounts.marketplace_program.to_account_info(),
        degenbtc_market::cpi::accounts::UpdateListingPrice {
            seller: ctx.accounts.seller.to_account_info(),
            marketplace_config: ctx.accounts.marketplace_config.to_account_info(),
            listing: ctx.accounts.marketplace_listing.to_account_info(),
        },
    );
    degenbtc_market::cpi::update_listing_price(cpi_ctx, new_price_lamports)?;

    let now = Clock::get()?.unix_timestamp;
    let queue = &mut ctx.accounts.floor_queue;

    // Pop existing entry (if any) and re-insert at the new sort position.
    if let Some(idx) = find_floor_entry_by_asset(queue, asset_key) {
        remove_floor_entry_at(queue, idx);
        emit!(FloorEntryRemoved {
            listing: listing_key,
            asset: asset_key,
            queue_index: idx,
            reason: 2, // price-update
            timestamp: now,
        });
    }
    let entry = FloorEntry {
        listing: listing_key,
        asset: asset_key,
        seller: seller_key,
        price: new_price_lamports,
        registered_at: now,
    };
    if let Ok(new_idx) = insert_floor_entry(queue, entry) {
        emit!(FloorEntryRegistered {
            listing: listing_key,
            asset: asset_key,
            seller: seller_key,
            price: new_price_lamports,
            queue_index: new_idx,
            queue_size_after: queue.entries_count,
            timestamp: now,
        });
    }

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateUserListingPrice<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,

    #[account(
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    #[account(
        mut,
        seeds = [FLOOR_QUEUE_SEED],
        bump = floor_queue.bump,
    )]
    pub floor_queue: Box<Account<'info, FloorQueue>>,

    /// CHECK: marketplace_config PDA.
    pub marketplace_config: UncheckedAccount<'info>,

    /// CHECK: Listing PDA.
    #[account(mut)]
    pub marketplace_listing: UncheckedAccount<'info>,

    /// CHECK: mpl-core asset (used as queue lookup key only).
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// CHECK: standalone marketplace program.
    pub marketplace_program: UncheckedAccount<'info>,
}

// ========================================================================================
// ============================== buy_user_listing ========================================
// ========================================================================================

/// Permissionless wrapper around `degenbtc_market::buy_listing`. If the buy
/// is between two non-protocol parties and the listing has been live for at
/// least `SALE_QUALIFY_MIN_LISTING_AGE_SECS`, records the sale into
/// `SaleHistory` so it can feed the floor snapshot anchor. Also pops the
/// listing from `FloorQueue` if present.
pub fn internal_buy_user_listing(
    ctx: Context<BuyUserListing>,
    max_price_lamports: u64,
) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_buy_user_listing");

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

    // Capture listing fields BEFORE the CPI closes the account.
    let (listing_price, listed_at, listing_seller, listing_asset) = {
        let info = ctx.accounts.marketplace_listing.to_account_info();
        require!(
            info.owner == &degenbtc_market::ID,
            ErrorCode::InvalidAccount
        );
        let data = info.try_borrow_data()?;
        let mut slice: &[u8] = &data;
        let listing = degenbtc_market::state::Listing::try_deserialize(&mut slice)?;
        (
            listing.price_lamports,
            listing.created_at,
            listing.seller,
            listing.asset,
        )
    };
    require_keys_eq!(
        listing_seller,
        ctx.accounts.seller.key(),
        ErrorCode::InvalidAccount
    );
    require_keys_eq!(
        listing_asset,
        ctx.accounts.hashbeast_asset.key(),
        ErrorCode::InvalidAccount
    );
    require!(
        listing_price <= max_price_lamports,
        ErrorCode::ListingPriceExceedsMax
    );

    let cpi_ctx = CpiContext::new(
        ctx.accounts.marketplace_program.to_account_info(),
        degenbtc_market::cpi::accounts::BuyListing {
            buyer: ctx.accounts.buyer.to_account_info(),
            seller: ctx.accounts.seller.to_account_info(),
            marketplace_config: ctx.accounts.marketplace_config.to_account_info(),
            listing: ctx.accounts.marketplace_listing.to_account_info(),
            asset: ctx.accounts.hashbeast_asset.to_account_info(),
            collection: ctx.accounts.hashbeast_collection.to_account_info(),
            escrow: ctx.accounts.marketplace_escrow.to_account_info(),
            fee_recipient: ctx.accounts.fee_recipient.to_account_info(),
            mpl_core_program: ctx.accounts.mpl_core_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        },
    );
    degenbtc_market::cpi::buy_listing(cpi_ctx, max_price_lamports)?;

    let now = Clock::get()?.unix_timestamp;
    let asset_key = listing_asset;
    let buyer_key = ctx.accounts.buyer.key();
    let inventory_key = ctx.accounts.inventory_pool.key();

    // Pop from floor queue if present. Read/write the queue in-place so the
    // large sorted buffer never lands in the generated account validator.
    let floor_queue_info = ctx.accounts.floor_queue.to_account_info();
    if let Some((idx, floor_entry)) = raw_find_floor_entry_by_asset(&floor_queue_info, asset_key)? {
        raw_remove_floor_entry_at(&floor_queue_info, idx)?;
        emit!(FloorEntryRemoved {
            listing: floor_entry.listing,
            asset: asset_key,
            queue_index: idx,
            reason: 0, // sweep / sale
            timestamp: now,
        });
    }

    // Sale recording: must be user-to-user with a 5-min minimum listing age.
    let qualifies = listing_seller != inventory_key
        && buyer_key != inventory_key
        && buyer_key != listing_seller
        && (now - listed_at) >= SALE_QUALIFY_MIN_LISTING_AGE_SECS;
    if qualifies {
        raw_record_sale(
            &ctx.accounts.sale_history.to_account_info(),
            SaleEntry {
                asset: asset_key,
                price: listing_price,
                listed_at,
                sold_at: now,
                buyer: buyer_key,
                seller: listing_seller,
            },
        )?;

        emit!(UserSaleRecorded {
            asset: asset_key,
            buyer: buyer_key,
            seller: listing_seller,
            price: listing_price,
            listing_age_secs: now - listed_at,
            timestamp: now,
        });
    }

    Ok(())
}

#[derive(Accounts)]
pub struct BuyUserListing<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    /// CHECK: Listing seller — receives proceeds.
    #[account(mut)]
    pub seller: UncheckedAccount<'info>,

    #[account(
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    #[account(mut, seeds = [FLOOR_QUEUE_SEED], bump)]
    /// CHECK: Seed-checked; read/written in-place to keep the validator stack small.
    pub floor_queue: UncheckedAccount<'info>,

    #[account(mut, seeds = [SALE_HISTORY_SEED], bump)]
    /// CHECK: Seed-checked; ringbuffer is updated in-place.
    pub sale_history: UncheckedAccount<'info>,

    /// CHECK: marketplace_config PDA.
    pub marketplace_config: UncheckedAccount<'info>,

    /// CHECK: Listing PDA — closed by CPI.
    #[account(mut)]
    pub marketplace_listing: UncheckedAccount<'info>,

    /// CHECK: mpl-core asset.
    #[account(mut)]
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// CHECK: HashBeast collection.
    #[account(mut)]
    pub hashbeast_collection: UncheckedAccount<'info>,

    /// CHECK: Escrow PDA.
    #[account(mut)]
    pub marketplace_escrow: UncheckedAccount<'info>,

    /// CHECK: Fee recipient.
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
// ============================== register_floor_listing ==================================
// ========================================================================================

/// Permissionless. Reads a marketplace `Listing` account and registers it
/// into the `FloorQueue` if it beats the worst-priced entry. Program-owned
/// listings are explicitly rejected.
pub fn internal_register_floor_listing(ctx: Context<RegisterFloorListing>) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_register_floor_listing");

    assert_marketplace_config(
        &ctx.accounts.marketplace_config.to_account_info(),
        ctx.accounts.inventory_pool.marketplace_config,
    )?;
    let collection = read_marketplace_config_collection(&ctx.accounts.marketplace_config)?;
    assert_asset_collection(&ctx.accounts.hashbeast_asset.to_account_info(), collection)?;

    let info = ctx.accounts.marketplace_listing.to_account_info();
    require!(
        info.owner == &degenbtc_market::ID,
        ErrorCode::InvalidAccount
    );
    let data = info.try_borrow_data()?;
    let mut slice: &[u8] = &data;
    let listing = degenbtc_market::state::Listing::try_deserialize(&mut slice)?;
    drop(data);

    let asset_key = ctx.accounts.hashbeast_asset.key();
    require_keys_eq!(listing.asset, asset_key, ErrorCode::InvalidAccount);
    require_keys_eq!(
        ctx.accounts.hashbeast_metadata.mint,
        asset_key,
        ErrorCode::InvalidAccount
    );
    assert_listing_pda(
        ctx.accounts.marketplace_listing.key(),
        ctx.accounts.marketplace_config.key(),
        asset_key,
    )?;

    let inventory_key = ctx.accounts.inventory_pool.key();
    require!(
        listing.seller != inventory_key,
        ErrorCode::ProgramListingNotAllowed
    );

    // Reject listings priced below the marketplace min — sweeping or trusting
    // these as floor anchors would let dust spam manipulate the queue.
    let min_price = read_marketplace_min_price(&ctx.accounts.marketplace_config.to_account_info())?;
    require!(
        listing.price_lamports >= min_price,
        ErrorCode::ListingPriceTooLow
    );

    let now = Clock::get()?.unix_timestamp;
    let entry = FloorEntry {
        listing: ctx.accounts.marketplace_listing.key(),
        asset: asset_key,
        seller: listing.seller,
        price: listing.price_lamports,
        registered_at: now,
    };

    let queue = &mut ctx.accounts.floor_queue;
    let idx = insert_floor_entry(queue, entry)?;

    emit!(FloorEntryRegistered {
        listing: entry.listing,
        asset: entry.asset,
        seller: entry.seller,
        price: entry.price,
        queue_index: idx,
        queue_size_after: queue.entries_count,
        timestamp: now,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct RegisterFloorListing<'info> {
    pub caller: Signer<'info>,

    #[account(
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    #[account(
        mut,
        seeds = [FLOOR_QUEUE_SEED],
        bump = floor_queue.bump,
    )]
    pub floor_queue: Box<Account<'info, FloorQueue>>,

    /// CHECK: marketplace Listing PDA, owned by degenbtc_market. Validated
    /// inline via owner check + try_deserialize.
    pub marketplace_listing: UncheckedAccount<'info>,

    /// CHECK: marketplace_config PDA, address-checked against pool cache.
    pub marketplace_config: UncheckedAccount<'info>,

    /// CHECK: mpl-core HashBeast asset referenced by the listing.
    pub hashbeast_asset: UncheckedAccount<'info>,

    #[account(
        seeds = [HASHBEAST_METADATA_SEED, hashbeast_asset.key().as_ref()],
        bump = hashbeast_metadata.bump,
        constraint = hashbeast_metadata.mint == hashbeast_asset.key() @ ErrorCode::InvalidAccount,
    )]
    pub hashbeast_metadata: Box<Account<'info, HashBeastMetadata>>,
}

// ========================================================================================
// ============================== record_floor_snapshot ===================================
// ========================================================================================

/// Permissionless. Once per `FLOOR_SNAPSHOT_INTERVAL_SECS` (24h), records a
/// fresh anchor into the 7-day rolling history. Pays a tiny keeper bounty.
pub fn internal_record_floor_snapshot(ctx: Context<RecordFloorSnapshot>) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_record_floor_snapshot");

    let now = Clock::get()?.unix_timestamp;
    let floor_history_info = ctx.accounts.floor_history.to_account_info();
    require!(
        now.saturating_sub(raw_floor_history_last_snapshot_at(&floor_history_info)?)
            >= FLOOR_SNAPSHOT_INTERVAL_SECS,
        ErrorCode::SnapshotTooSoon
    );

    let (anchor, source, samples) = raw_compute_snapshot_anchor(
        &ctx.accounts.sale_history.to_account_info(),
        &ctx.accounts.floor_queue.to_account_info(),
        now,
    )?;
    require!(samples > 0, ErrorCode::NoLiveFloorEntries);
    require!(
        anchor >= SWEEP_MIN_ANCHOR_LAMPORTS,
        ErrorCode::SweepAnchorTooLow
    );

    raw_record_floor_snapshot(&floor_history_info, now, anchor)?;

    // Keeper bounty.
    pay_keeper(
        &ctx.accounts.inventory_sweep_vault.to_account_info(),
        &ctx.accounts.caller.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.inventory_sweep_vault,
        KEEPER_REWARD_LAMPORTS,
    )?;

    emit!(FloorSnapshotRecorded {
        anchor_price: anchor,
        source,
        samples,
        timestamp: now,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct RecordFloorSnapshot<'info> {
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    #[account(seeds = [FLOOR_QUEUE_SEED], bump)]
    /// CHECK: Seed-checked; read in-place to keep the validator stack small.
    pub floor_queue: UncheckedAccount<'info>,

    #[account(seeds = [SALE_HISTORY_SEED], bump)]
    /// CHECK: Seed-checked; read in-place to keep the validator stack small.
    pub sale_history: UncheckedAccount<'info>,

    #[account(mut, seeds = [FLOOR_HISTORY_SEED], bump)]
    /// CHECK: Seed-checked; read/written in-place to keep the validator stack small.
    pub floor_history: UncheckedAccount<'info>,

    /// CHECK: SOL vault — keeper bounty source.
    #[account(
        mut,
        seeds = [INVENTORY_SWEEP_VAULT_SEED],
        bump,
    )]
    pub inventory_sweep_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

// ========================================================================================
// ============================== sweep_floor_lowest ======================================
// ========================================================================================

/// Permissionless. Buys queue.entries[0]. If the head is stale, this ix purges
/// that one entry and exits successfully so a bot can retry the next head.
/// For a live head, it disposes the swept asset:
///   - lootbox queue has space → push (RebornEntry::Lootbox)
///   - trend below burn threshold → burn (no RebornEntry)
///   - else → relist at formula price (RebornEntry::Listed)
/// Pays a keeper bounty out of `inventory_sweep_vault`.
pub fn internal_sweep_floor_lowest(ctx: Context<SweepFloorLowest>) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_sweep_floor_lowest");

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

    // Validate caller-provided listing matches queue.entries[0]. A stale head is
    // purged and the caller can retry the sweep against the next head.
    let target_listing = ctx.accounts.marketplace_listing.key();
    let inventory_key = ctx.accounts.inventory_pool.key();
    let floor_queue_info = ctx.accounts.floor_queue.to_account_info();

    let maybe_chosen_entry: Option<FloorEntry> = {
        let head = raw_floor_queue_head(&floor_queue_info)?;

        // Verify caller passed the matching listing PDA.
        if head.listing != target_listing {
            msg!(
                "❌ caller listing {} != queue head listing {}",
                target_listing,
                head.listing
            );
            return err!(ErrorCode::StaleFloorEntry);
        }

        // Liveness check: read live listing.
        let info = ctx.accounts.marketplace_listing.to_account_info();
        let live_ok = info.owner == &degenbtc_market::ID && info.lamports() > 0;
        let mut stale = !live_ok;
        if !stale {
            let data = info.try_borrow_data()?;
            let mut slice: &[u8] = &data;
            match degenbtc_market::state::Listing::try_deserialize(&mut slice) {
                Ok(live) => {
                    if live.price_lamports != head.price
                        || live.seller != head.seller
                        || live.asset != head.asset
                        || head.seller == inventory_key
                    {
                        stale = true;
                    }
                }
                Err(_) => stale = true,
            }
        }

        if stale {
            raw_remove_floor_entry_at(&floor_queue_info, 0)?;
            emit!(FloorEntryRemoved {
                listing: head.listing,
                asset: head.asset,
                queue_index: 0,
                reason: 3, // stale
                timestamp: Clock::get()?.unix_timestamp,
            });
            None
        } else {
            Some(head)
        }
    };

    let Some(chosen_entry) = maybe_chosen_entry else {
        pay_keeper(
            &ctx.accounts.inventory_sweep_vault.to_account_info(),
            &ctx.accounts.caller.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            ctx.bumps.inventory_sweep_vault,
            KEEPER_REWARD_LAMPORTS,
        )?;
        return Ok(());
    };

    require!(
        ctx.accounts.inventory_pool.total_count < MAX_INVENTORY,
        ErrorCode::InventoryFull
    );
    require_keys_eq!(
        chosen_entry.asset,
        ctx.accounts.hashbeast_asset.key(),
        ErrorCode::InvalidAccount
    );
    assert_listing_pda(
        chosen_entry.listing,
        ctx.accounts.marketplace_config.key(),
        chosen_entry.asset,
    )?;

    // Anchor for price ceiling.
    let floor_history_info = ctx.accounts.floor_history.to_account_info();
    let trend_bps = raw_floor_history_compute_trend_bps(&floor_history_info)?;
    let anchor = raw_floor_history_current_anchor(&floor_history_info)?;
    require!(
        anchor >= SWEEP_MIN_ANCHOR_LAMPORTS,
        ErrorCode::SweepAnchorTooLow
    );

    // Price ceiling vs anchor.
    let max_price =
        ((anchor as u128 * (10_000u128 + SWEEP_ATTRACTIVE_BPS as u128)) / 10_000u128) as u64;
    require!(
        chosen_entry.price <= max_price,
        ErrorCode::FloorPriceTooHigh
    );

    // Vault reserve + per-tx cap.
    let vault_lamports = ctx.accounts.inventory_sweep_vault.lamports();
    let needed = chosen_entry
        .price
        .checked_add(MIN_SWEEP_RESERVE_LAMPORTS)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_add(KEEPER_REWARD_LAMPORTS)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    require!(vault_lamports >= needed, ErrorCode::SweepVaultBelowReserve);

    let tx_cap = ((vault_lamports as u128 * SWEEP_MAX_PCT_BPS as u128) / 10_000u128) as u64;
    require!(chosen_entry.price <= tx_cap, ErrorCode::SweepTxCapExceeded);

    // Pre-fund inventory_pda with the buy amount.
    fund_inventory_from_sweep_vault(
        &ctx.accounts.inventory_sweep_vault.to_account_info(),
        &ctx.accounts.inventory_pda.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.inventory_sweep_vault,
        chosen_entry.price,
    )?;

    // CPI buy_listing as inventory_pda.
    let pool_bump = ctx.accounts.inventory_pool.bump;
    let inventory_seeds_inner: &[&[u8]] = &[INVENTORY_POOL_SEED, &[pool_bump]];
    let signers: &[&[&[u8]]] = &[inventory_seeds_inner];
    {
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.marketplace_program.to_account_info(),
            degenbtc_market::cpi::accounts::BuyListing {
                buyer: ctx.accounts.inventory_pda.to_account_info(),
                seller: ctx.accounts.seller.to_account_info(),
                marketplace_config: ctx.accounts.marketplace_config.to_account_info(),
                listing: ctx.accounts.marketplace_listing.to_account_info(),
                asset: ctx.accounts.hashbeast_asset.to_account_info(),
                collection: ctx.accounts.hashbeast_collection.to_account_info(),
                escrow: ctx.accounts.marketplace_escrow.to_account_info(),
                fee_recipient: ctx.accounts.fee_recipient.to_account_info(),
                mpl_core_program: ctx.accounts.mpl_core_program.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            },
            signers,
        );
        degenbtc_market::cpi::buy_listing(cpi_ctx, chosen_entry.price)?;
    }

    // Pop the swept entry from the queue.
    raw_remove_floor_entry_at(&floor_queue_info, 0)?;

    let asset_key = ctx.accounts.hashbeast_asset.key();
    let faction_id = ctx.accounts.hashbeast_metadata.faction_id;
    let now = Clock::get()?.unix_timestamp;

    // Disposition cascade.
    let queue_has_space = (ctx.accounts.lootbox_queue.filled_count as usize) < LOOTBOX_QUEUE_SIZE;

    if queue_has_space {
        // Path A: push to lootbox queue. Init RebornEntry as Lootbox.
        let quality_score = compute_quality_score(
            ctx.accounts.hashbeast_metadata.multiplier,
            ctx.accounts.hashbeast_metadata.xp,
            ctx.accounts.hashbeast_metadata.breed_count,
        );
        let entry_info = ctx.accounts.reborn_entry.to_account_info();
        let asset_bytes = asset_key.to_bytes();
        let bump = ctx.bumps.reborn_entry;
        let entry_seeds: &[&[u8]] = &[
            REBORN_ENTRY_SEED,
            asset_bytes.as_ref(),
            core::slice::from_ref(&bump),
        ];
        let blank = RebornEntry {
            bump,
            asset: asset_key,
            faction_id,
            quality_score,
            reborn_at: now,
            status: RebornStatus::Lootbox as u8,
            listing_price: 0,
            origin: RebornOrigin::Swept as u8,
            original_buy_price: chosen_entry.price,
            expire_count: 0,
        };
        crate::instructions::helper::init_pda_account_if_needed::<RebornEntry>(
            &ctx.accounts.caller.to_account_info(),
            &entry_info,
            &ctx.accounts.system_program.to_account_info(),
            entry_seeds,
            RebornEntry::LEN,
            &blank,
        )?;

        let depth_after = {
            let lootbox = &mut ctx.accounts.lootbox_queue;
            let idx = lootbox.filled_count as usize;
            lootbox.slots[idx] = asset_key;
            lootbox.filled_count = lootbox
                .filled_count
                .checked_add(1)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            lootbox.filled_count
        };
        ctx.accounts.inventory_pool.total_count = ctx
            .accounts
            .inventory_pool
            .total_count
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        emit!(LootboxQueuePush {
            faction_id,
            asset: asset_key,
            queue_depth_after: depth_after,
            source: 1, // sweep_buy
            timestamp: now,
        });
    } else if trend_bps < BURN_TREND_BPS_THRESHOLD {
        // Path B: deep bear, burn the asset. No RebornEntry, no counter bump.
        crate::mpl_core_helpers::burn_mpl_core_asset(
            &ctx.accounts.hashbeast_asset.to_account_info(),
            Some(&ctx.accounts.hashbeast_collection.to_account_info()),
            &ctx.accounts.caller.to_account_info(),
            &ctx.accounts.inventory_pda.to_account_info(),
            &ctx.accounts.mpl_core_program.to_account_info(),
            Some(signers),
        )?;
        emit!(InventoryAssetBurned {
            asset: asset_key,
            reason: 0, // trend crash
            trend_bps,
            expire_count: 0,
            timestamp: now,
        });
    } else {
        // Path C: relist at formula markup. Init RebornEntry as Listed.
        let markup_bps = compute_relist_markup_bps(trend_bps, 0);
        let new_price = apply_markup(chosen_entry.price, markup_bps);

        let quality_score = compute_quality_score(
            ctx.accounts.hashbeast_metadata.multiplier,
            ctx.accounts.hashbeast_metadata.xp,
            ctx.accounts.hashbeast_metadata.breed_count,
        );
        let entry_info = ctx.accounts.reborn_entry.to_account_info();
        let asset_bytes = asset_key.to_bytes();
        let bump = ctx.bumps.reborn_entry;
        let entry_seeds: &[&[u8]] = &[
            REBORN_ENTRY_SEED,
            asset_bytes.as_ref(),
            core::slice::from_ref(&bump),
        ];
        let blank = RebornEntry {
            bump,
            asset: asset_key,
            faction_id,
            quality_score,
            reborn_at: now,
            status: RebornStatus::Listed as u8,
            listing_price: new_price,
            origin: RebornOrigin::Swept as u8,
            original_buy_price: chosen_entry.price,
            expire_count: 0,
        };
        crate::instructions::helper::init_pda_account_if_needed::<RebornEntry>(
            &ctx.accounts.caller.to_account_info(),
            &entry_info,
            &ctx.accounts.system_program.to_account_info(),
            entry_seeds,
            RebornEntry::LEN,
            &blank,
        )?;

        // CPI list_nft as inventory_pda. The just-closed listing PDA is the
        // same address as the new listing PDA (same seeds), so re-init is OK.
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.marketplace_program.to_account_info(),
            degenbtc_market::cpi::accounts::ListNft {
                seller: ctx.accounts.inventory_pda.to_account_info(),
                marketplace_config: ctx.accounts.marketplace_config.to_account_info(),
                listing: ctx.accounts.marketplace_listing.to_account_info(),
                asset: ctx.accounts.hashbeast_asset.to_account_info(),
                collection: ctx.accounts.hashbeast_collection.to_account_info(),
                escrow: ctx.accounts.marketplace_escrow.to_account_info(),
                mpl_core_program: ctx.accounts.mpl_core_program.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            },
            signers,
        );
        degenbtc_market::cpi::list_nft(cpi_ctx, new_price)?;

        ctx.accounts.inventory_pool.total_count = ctx
            .accounts
            .inventory_pool
            .total_count
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        emit!(InventoryAssetRelisted {
            asset: asset_key,
            original_buy_price: chosen_entry.price,
            new_list_price: new_price,
            markup_bps,
            trend_bps,
            expire_count: 0,
            timestamp: now,
        });
    }

    // Pay keeper.
    pay_keeper(
        &ctx.accounts.inventory_sweep_vault.to_account_info(),
        &ctx.accounts.caller.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.inventory_sweep_vault,
        KEEPER_REWARD_LAMPORTS,
    )?;

    emit!(FloorSweepExecuted {
        asset: asset_key,
        buy_price: chosen_entry.price,
        seller: chosen_entry.seller,
        anchor_price: anchor,
        trend_bps,
        stale_skipped: 0,
        keeper: ctx.accounts.caller.key(),
        timestamp: now,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct SweepFloorLowest<'info> {
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    /// CHECK: Inventory custody PDA — same address as inventory_pool.
    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pda: UncheckedAccount<'info>,

    /// CHECK: SOL vault for sweep buys + keeper bounty.
    #[account(
        mut,
        seeds = [INVENTORY_SWEEP_VAULT_SEED],
        bump,
    )]
    pub inventory_sweep_vault: UncheckedAccount<'info>,

    #[account(mut, seeds = [FLOOR_QUEUE_SEED], bump)]
    /// CHECK: Seed-checked; read/written in-place to keep the validator stack small.
    pub floor_queue: UncheckedAccount<'info>,

    #[account(seeds = [FLOOR_HISTORY_SEED], bump)]
    /// CHECK: Seed-checked; read in-place to keep the validator stack small.
    pub floor_history: UncheckedAccount<'info>,

    /// New entry created on queue/relist paths. PDA seeds enforced; payload
    /// init'd manually inside the handler.
    /// CHECK: PDA validated by seeds; init via helper.
    #[account(
        mut,
        seeds = [REBORN_ENTRY_SEED, hashbeast_asset.key().as_ref()],
        bump,
    )]
    pub reborn_entry: UncheckedAccount<'info>,

    /// HashBeast metadata (read for faction_id / quality_score).
    #[account(
        seeds = [HASHBEAST_METADATA_SEED, hashbeast_asset.key().as_ref()],
        bump = hashbeast_metadata.bump,
        constraint = hashbeast_metadata.mint == hashbeast_asset.key() @ ErrorCode::InvalidAccount,
    )]
    pub hashbeast_metadata: Box<Account<'info, HashBeastMetadata>>,

    /// Country lootbox queue for this asset's faction.
    #[account(
        mut,
        seeds = [LOOTBOX_QUEUE_SEED, &[hashbeast_metadata.faction_id]],
        bump = lootbox_queue.bump,
    )]
    pub lootbox_queue: Box<Account<'info, LootboxQueue>>,

    /// CHECK: marketplace_config PDA.
    #[account(mut)]
    pub marketplace_config: UncheckedAccount<'info>,

    /// CHECK: Listing PDA — closed by buy CPI, re-init'd by relist CPI on
    /// path C. Address must match floor_queue.entries[0].listing.
    #[account(mut)]
    pub marketplace_listing: UncheckedAccount<'info>,

    /// CHECK: Escrow PDA.
    #[account(mut)]
    pub marketplace_escrow: UncheckedAccount<'info>,

    /// CHECK: mpl-core asset.
    #[account(mut)]
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// CHECK: HashBeast collection.
    #[account(mut)]
    pub hashbeast_collection: UncheckedAccount<'info>,

    /// CHECK: Listing seller — receives proceeds.
    #[account(mut)]
    pub seller: UncheckedAccount<'info>,

    /// CHECK: Marketplace fee recipient.
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
// ============================== expire_program_listing ==================================
// ========================================================================================

/// Permissionless. Cancels a stale program-owned listing and re-runs the
/// disposition cascade. Subject to a 7-day grace period since the listing
/// was created.
pub fn internal_expire_program_listing(ctx: Context<ExpireProgramListing>) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_expire_program_listing");

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

    let asset_key = ctx.accounts.hashbeast_asset.key();
    let inventory_key = ctx.accounts.inventory_pool.key();

    // Manually load reborn_entry (UncheckedAccount). Avoids the
    // Box<Account<T>> Drop-guard re-serialize-on-exit panic when we close the
    // account on burn paths.
    let entry_info = ctx.accounts.reborn_entry.to_account_info();
    require_keys_eq!(*entry_info.owner, crate::ID, ErrorCode::InvalidAccount);
    let mut entry: RebornEntry = {
        let data = entry_info.try_borrow_data()?;
        RebornEntry::try_deserialize(&mut &data[..])?
    };
    require!(
        entry.status == RebornStatus::Listed as u8,
        ErrorCode::InvalidRebornStatus
    );
    require_keys_eq!(entry.asset, asset_key, ErrorCode::InvalidAccount);
    require!(
        entry.faction_id == ctx.accounts.lootbox_queue.faction_id,
        ErrorCode::InvalidFactionId
    );
    let stored_bump = entry.bump;

    // Validate listing belongs to inventory_pda and is old enough.
    let (listing_seller, listing_asset, listing_age) = {
        let info = ctx.accounts.marketplace_listing.to_account_info();
        require!(
            info.owner == &degenbtc_market::ID,
            ErrorCode::InvalidAccount
        );
        let data = info.try_borrow_data()?;
        let mut slice: &[u8] = &data;
        let listing = degenbtc_market::state::Listing::try_deserialize(&mut slice)?;
        let now = Clock::get()?.unix_timestamp;
        (
            listing.seller,
            listing.asset,
            now.saturating_sub(listing.created_at),
        )
    };
    require_keys_eq!(listing_seller, inventory_key, ErrorCode::NotProgramListing);
    require_keys_eq!(listing_asset, asset_key, ErrorCode::InvalidAccount);
    assert_listing_pda(
        ctx.accounts.marketplace_listing.key(),
        ctx.accounts.marketplace_config.key(),
        asset_key,
    )?;
    require!(
        listing_age >= EXPIRE_GRACE_SECS,
        ErrorCode::ListingNotYetExpirable
    );

    let previous_price = entry.listing_price;
    let pool_bump = ctx.accounts.inventory_pool.bump;
    let inventory_seeds_inner: &[&[u8]] = &[INVENTORY_POOL_SEED, &[pool_bump]];
    let signers: &[&[&[u8]]] = &[inventory_seeds_inner];

    // Cancel the existing listing — asset returns to inventory_pda.
    {
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.marketplace_program.to_account_info(),
            degenbtc_market::cpi::accounts::CancelListing {
                seller: ctx.accounts.inventory_pda.to_account_info(),
                marketplace_config: ctx.accounts.marketplace_config.to_account_info(),
                listing: ctx.accounts.marketplace_listing.to_account_info(),
                asset: ctx.accounts.hashbeast_asset.to_account_info(),
                collection: ctx.accounts.hashbeast_collection.to_account_info(),
                escrow: ctx.accounts.marketplace_escrow.to_account_info(),
                mpl_core_program: ctx.accounts.mpl_core_program.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            },
            signers,
        );
        degenbtc_market::cpi::cancel_listing(cpi_ctx)?;
    }

    // Increment expire_count BEFORE deciding next path.
    entry.expire_count = entry.expire_count.saturating_add(1);
    entry.listing_price = 0;
    let expire_count_after = entry.expire_count;
    let original_buy_price = entry.original_buy_price;
    let faction_id = entry.faction_id;
    let now = Clock::get()?.unix_timestamp;
    let _ = stored_bump; // bump captured for symmetry; verified by Anchor seeds

    let trend_bps = ctx.accounts.floor_history.compute_trend_bps();

    emit!(ProgramListingExpired {
        asset: asset_key,
        previous_list_price: previous_price,
        expire_count_after,
        keeper: ctx.accounts.caller.key(),
        timestamp: now,
    });

    let force_burn = expire_count_after >= MAX_EXPIRES;
    let queue_has_space = (ctx.accounts.lootbox_queue.filled_count as usize) < LOOTBOX_QUEUE_SIZE;

    // Helper: persist mutated entry back to its account data.
    fn persist_entry(entry_info: &AccountInfo, entry: &RebornEntry) -> Result<()> {
        let mut data = entry_info.try_borrow_mut_data()?;
        let mut cursor = &mut data[..];
        entry.try_serialize(&mut cursor)?;
        Ok(())
    }

    // Helper: manually close the reborn_entry account, refunding lamports.
    fn manual_close(entry_info: &AccountInfo, dest: &AccountInfo) -> Result<()> {
        let dest_lamports = dest.lamports();
        **dest.lamports.borrow_mut() = dest_lamports
            .checked_add(entry_info.lamports())
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        **entry_info.lamports.borrow_mut() = 0;
        entry_info.assign(&anchor_lang::system_program::ID);
        entry_info.realloc(0, false)?;
        Ok(())
    }

    if force_burn {
        // Strikes exhausted → burn.
        crate::mpl_core_helpers::burn_mpl_core_asset(
            &ctx.accounts.hashbeast_asset.to_account_info(),
            Some(&ctx.accounts.hashbeast_collection.to_account_info()),
            &ctx.accounts.caller.to_account_info(),
            &ctx.accounts.inventory_pda.to_account_info(),
            &ctx.accounts.mpl_core_program.to_account_info(),
            Some(signers),
        )?;
        ctx.accounts.inventory_pool.total_count = ctx
            .accounts
            .inventory_pool
            .total_count
            .checked_sub(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        emit!(InventoryAssetBurned {
            asset: asset_key,
            reason: 1, // max expires
            trend_bps,
            expire_count: expire_count_after,
            timestamp: now,
        });
        manual_close(&entry_info, &ctx.accounts.caller.to_account_info())?;
    } else if queue_has_space {
        // Push to lootbox queue.
        let depth_after = {
            let lootbox = &mut ctx.accounts.lootbox_queue;
            let idx = lootbox.filled_count as usize;
            lootbox.slots[idx] = asset_key;
            lootbox.filled_count = lootbox
                .filled_count
                .checked_add(1)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            lootbox.filled_count
        };
        entry.status = RebornStatus::Lootbox as u8;
        persist_entry(&entry_info, &entry)?;
        emit!(LootboxQueuePush {
            faction_id,
            asset: asset_key,
            queue_depth_after: depth_after,
            source: 2, // expire-cascade
            timestamp: now,
        });
    } else if trend_bps < BURN_TREND_BPS_THRESHOLD {
        // Trend crashed below burn threshold.
        crate::mpl_core_helpers::burn_mpl_core_asset(
            &ctx.accounts.hashbeast_asset.to_account_info(),
            Some(&ctx.accounts.hashbeast_collection.to_account_info()),
            &ctx.accounts.caller.to_account_info(),
            &ctx.accounts.inventory_pda.to_account_info(),
            &ctx.accounts.mpl_core_program.to_account_info(),
            Some(signers),
        )?;
        ctx.accounts.inventory_pool.total_count = ctx
            .accounts
            .inventory_pool
            .total_count
            .checked_sub(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        emit!(InventoryAssetBurned {
            asset: asset_key,
            reason: 0, // trend crash
            trend_bps,
            expire_count: expire_count_after,
            timestamp: now,
        });
        manual_close(&entry_info, &ctx.accounts.caller.to_account_info())?;
    } else {
        // Relist at progressively-discounted markup.
        let markup_bps = compute_relist_markup_bps(trend_bps, expire_count_after);
        let new_price = apply_markup(original_buy_price, markup_bps);

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.marketplace_program.to_account_info(),
            degenbtc_market::cpi::accounts::ListNft {
                seller: ctx.accounts.inventory_pda.to_account_info(),
                marketplace_config: ctx.accounts.marketplace_config.to_account_info(),
                listing: ctx.accounts.marketplace_listing.to_account_info(),
                asset: ctx.accounts.hashbeast_asset.to_account_info(),
                collection: ctx.accounts.hashbeast_collection.to_account_info(),
                escrow: ctx.accounts.marketplace_escrow.to_account_info(),
                mpl_core_program: ctx.accounts.mpl_core_program.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            },
            signers,
        );
        degenbtc_market::cpi::list_nft(cpi_ctx, new_price)?;

        entry.status = RebornStatus::Listed as u8;
        entry.listing_price = new_price;
        persist_entry(&entry_info, &entry)?;

        emit!(InventoryAssetRelisted {
            asset: asset_key,
            original_buy_price,
            new_list_price: new_price,
            markup_bps,
            trend_bps,
            expire_count: expire_count_after,
            timestamp: now,
        });
    }

    // Keeper bounty.
    pay_keeper(
        &ctx.accounts.inventory_sweep_vault.to_account_info(),
        &ctx.accounts.caller.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.inventory_sweep_vault,
        KEEPER_REWARD_LAMPORTS,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct ExpireProgramListing<'info> {
    #[account(mut)]
    pub caller: Signer<'info>,

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

    /// CHECK: SOL vault — keeper bounty source.
    #[account(
        mut,
        seeds = [INVENTORY_SWEEP_VAULT_SEED],
        bump,
    )]
    pub inventory_sweep_vault: UncheckedAccount<'info>,

    /// CHECK: PDA-validated by seeds + program owner check in handler. Manual
    /// load/store/close avoids the `Account<T>` Drop-guard re-serialize panic
    /// on the burn paths (where we close the account before exit).
    #[account(
        mut,
        seeds = [REBORN_ENTRY_SEED, hashbeast_asset.key().as_ref()],
        bump,
    )]
    pub reborn_entry: UncheckedAccount<'info>,

    #[account(
        seeds = [FLOOR_HISTORY_SEED],
        bump = floor_history.bump,
    )]
    pub floor_history: Box<Account<'info, FloorHistory>>,

    /// Country lootbox queue for the asset's faction. Seed validated against
    /// the entry's faction_id inside the handler (we deserialize manually).
    #[account(
        mut,
        seeds = [LOOTBOX_QUEUE_SEED, &[lootbox_queue.faction_id]],
        bump = lootbox_queue.bump,
    )]
    pub lootbox_queue: Box<Account<'info, LootboxQueue>>,

    /// CHECK: marketplace_config PDA.
    #[account(mut)]
    pub marketplace_config: UncheckedAccount<'info>,

    /// CHECK: Listing PDA — closed by cancel CPI, possibly re-init'd by list CPI.
    #[account(mut)]
    pub marketplace_listing: UncheckedAccount<'info>,

    /// CHECK: Escrow PDA.
    #[account(mut)]
    pub marketplace_escrow: UncheckedAccount<'info>,

    /// CHECK: mpl-core asset.
    #[account(mut)]
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// CHECK: HashBeast collection.
    #[account(mut)]
    pub hashbeast_collection: UncheckedAccount<'info>,

    /// CHECK: standalone marketplace program.
    pub marketplace_program: UncheckedAccount<'info>,

    /// CHECK: mpl-core program.
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

// ========================================================================================
// ============================== handle_inventory_proceeds ===============================
// ========================================================================================

/// Permissionless. Drains lamports accumulated on inventory_pda from sale
/// proceeds, splitting `INVENTORY_SWEEP_RESERVE_BPS` to the sweep vault and
/// the rest to sol_treasury.
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

    emit!(InventoryProceedsRouted {
        to_sweep,
        to_protocol,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct HandleInventoryProceeds<'info> {
    pub caller: Signer<'info>,

    #[account(
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    /// CHECK: Inventory PDA — same address as inventory_pool.
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

    /// CHECK: SOL treasury PDA.
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED],
        bump,
    )]
    pub sol_treasury: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

// ========================================================================================
// ============================== inventory_finalize_sale =================================
// ========================================================================================

/// Permissionless. Verifies an inventory listing's asset is no longer owned
/// by `inventory_pda` (i.e., a real buyer purchased it), then closes the
/// `RebornEntry` and decrements `total_count`.
pub fn internal_inventory_finalize_sale(ctx: Context<InventoryFinalizeSale>) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_inventory_finalize_sale");

    require!(
        ctx.accounts.reborn_entry.status == RebornStatus::Listed as u8,
        ErrorCode::InvalidRebornStatus
    );
    require_keys_eq!(
        ctx.accounts.hashbeast_asset.key(),
        ctx.accounts.reborn_entry.asset,
        ErrorCode::InvalidAccount
    );

    let inventory_key = ctx.accounts.inventory_pool.key();
    let owner = read_asset_owner(&ctx.accounts.hashbeast_asset.to_account_info())?;
    require!(
        owner != inventory_key,
        ErrorCode::AssetStillOwnedByInventory
    );

    let asset = ctx.accounts.reborn_entry.asset;

    ctx.accounts.inventory_pool.total_count = ctx
        .accounts
        .inventory_pool
        .total_count
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    emit!(InventorySaleFinalized {
        asset,
        keeper: ctx.accounts.caller.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    // Anchor closes reborn_entry to caller via `close = caller` at exit.
    Ok(())
}

#[derive(Accounts)]
pub struct InventoryFinalizeSale<'info> {
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    #[account(
        mut,
        close = caller,
        seeds = [REBORN_ENTRY_SEED, reborn_entry.asset.as_ref()],
        bump = reborn_entry.bump,
    )]
    pub reborn_entry: Box<Account<'info, RebornEntry>>,

    /// CHECK: mpl-core asset — read-only here, owner field inspected.
    pub hashbeast_asset: UncheckedAccount<'info>,
}

// ========================================================================================
// ============================== claim_lootbox_nft ========================================
// ========================================================================================

/// Permissionless delivery for a reserved loser-roll hashbeast. The signer may be
/// the user or any cranker bot. The recipient is fixed by `LootboxClaim.user`,
/// so the cranker can only pay to deliver the NFT to the recorded winner.
pub fn internal_claim_lootbox_nft(ctx: Context<ClaimLootboxNft>) -> Result<()> {
    crate::log_fn!("marketplace_cpi", "internal_claim_lootbox_nft");

    let now = Clock::get()?.unix_timestamp;
    let user_key = ctx.accounts.user.key();
    let cranker_key = ctx.accounts.cranker.key();
    let asset_key = ctx.accounts.hashbeast_asset.key();
    let claim = &ctx.accounts.lootbox_claim;
    let entry = &ctx.accounts.reborn_entry;

    // Reservation validity.
    require_keys_eq!(claim.user, user_key, ErrorCode::InvalidOwner);
    require_keys_eq!(claim.asset, asset_key, ErrorCode::InvalidAccount);
    require!(asset_key != Pubkey::default(), ErrorCode::InvalidAccount);
    require!(
        entry.status == RebornStatus::Lootbox as u8,
        ErrorCode::InvalidRebornStatus
    );
    require_keys_eq!(entry.asset, asset_key, ErrorCode::InvalidAccount);

    let faction_id = claim.faction_id;

    // Transfer asset inventory_pda → user (mpl-core, signed by inventory PDA seeds).
    let pool_bump = ctx.accounts.inventory_pool.bump;
    let inventory_seeds_inner: &[&[u8]] = &[INVENTORY_POOL_SEED, &[pool_bump]];
    let inventory_signers: &[&[&[u8]]] = &[inventory_seeds_inner];
    crate::mpl_core_helpers::transfer_mpl_core_asset(
        &ctx.accounts.hashbeast_asset.to_account_info(),
        ctx.accounts
            .hashbeast_collection
            .as_ref()
            .map(|c| c.to_account_info())
            .as_ref(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.inventory_pda.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        Some(inventory_signers),
    )?;

    // Delivery does not mutate HashBeastMetadata. User-reborn assets were already
    // reborn inside `rebirth_hashbeast`; market-maker assets keep their existing DNA.
    let rebirth_count = {
        let metadata = &ctx.accounts.hashbeast_metadata;
        current_rebirth_count(metadata)
    };

    // Decrement total_count.
    ctx.accounts.inventory_pool.total_count = ctx
        .accounts
        .inventory_pool
        .total_count
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    emit!(LootboxNftClaimed {
        user: user_key,
        cranker: cranker_key,
        faction_id,
        asset: asset_key,
        rebirth_count,
        timestamp: now,
    });

    msg!(
        "🎁 [claim_lootbox_nft] {} → {} (faction {}, cranker {})",
        asset_key,
        user_key,
        faction_id,
        cranker_key
    );

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimLootboxNft<'info> {
    #[account(mut)]
    pub cranker: Signer<'info>,

    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    /// CHECK: Inventory custody PDA — same address as `inventory_pool`.
    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pda: UncheckedAccount<'info>,

    /// Reservation PDA, closed on success. Rent goes to the cranker as the
    /// delivery incentive; the NFT recipient remains fixed by `lootbox_claim.user`.
    #[account(
        mut,
        close = cranker,
        seeds = [LOOTBOX_CLAIM_SEED, lootbox_claim.user.as_ref()],
        bump = lootbox_claim.bump,
    )]
    pub lootbox_claim: Box<Account<'info, LootboxClaim>>,

    /// CHECK: Recorded winner. Does not sign; receives the NFT.
    #[account(
        mut,
        address = lootbox_claim.user @ ErrorCode::InvalidOwner,
    )]
    pub user: UncheckedAccount<'info>,

    /// RebornEntry for the dropped asset, closed on success.
    #[account(
        mut,
        close = cranker,
        seeds = [REBORN_ENTRY_SEED, hashbeast_asset.key().as_ref()],
        bump = reborn_entry.bump,
    )]
    pub reborn_entry: Box<Account<'info, RebornEntry>>,

    /// CHECK: mpl-core asset; current owner is `inventory_pda`.
    #[account(mut)]
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// HashBeast metadata, read for rebirth generation emitted to indexers.
    #[account(
        seeds = [HASHBEAST_METADATA_SEED, hashbeast_asset.key().as_ref()],
        bump = hashbeast_metadata.bump,
        constraint = hashbeast_metadata.mint == hashbeast_asset.key() @ ErrorCode::InvalidAccount,
    )]
    pub hashbeast_metadata: Account<'info, HashBeastMetadata>,

    /// CHECK: HashBeast collection (required by mpl-core on transfer).
    #[account(mut)]
    pub hashbeast_collection: Option<UncheckedAccount<'info>>,

    /// CHECK: Metaplex Core program.
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
