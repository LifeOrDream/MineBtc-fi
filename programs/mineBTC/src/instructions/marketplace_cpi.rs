//! # Marketplace CPI + Permissionless On-Chain Market Maker
//!
//! This module is the bridge between the `mineBTC` program and the
//! standalone `degenbtc_market` program, **and** the on-chain market maker
//! that defends the HashBeast floor price by buying cheap listings into a
//! protocol-owned inventory. There is no admin crank — every path is either
//! user-signed (the asset owner) or fully permissionless (anyone can call,
//! with a small keeper bounty paid from `inventory_sweep_vault`).
//!
//! ## Why this module exists
//!
//! Without floor support, secondary-market panic sells could spiral the
//! HashBeast floor below the breed price, breaking the breeding bonding
//! curve and trapping new mints. To prevent that, the protocol earmarks a
//! slice of every distributed SOL fee into `inventory_sweep_vault`, then
//! lets permissionless keepers spend it on cheap floor listings. The
//! protocol absorbs those NFTs into `inventory_pda` and disposes them via:
//!   - dropping into a faction's `LootboxQueue` (rewards round losers), or
//!   - relisting at a markup (resells back into the market), or
//!   - burning (only on deep bearish trend).
//!
//! ## State
//!
//! ```text
//!   InventoryPool        seeds [INVENTORY_POOL]
//!                        — cached marketplace_program / marketplace_config
//!                        — total_count (cap MAX_INVENTORY)
//!                        — the account *itself* IS the inventory custody
//!                          PDA (same address as `inventory_pda` in every
//!                          struct). Sale proceeds land here as lamports.
//!
//!   inventory_sweep_vault   seeds [INVENTORY_SWEEP_VAULT]
//!                           system-owned SOL vault, no data. Pays for
//!                           sweep buys + keeper bounties.
//!
//!   FloorQueue           seeds [FLOOR_QUEUE]
//!                        sorted-ascending fixed-size buffer of the
//!                        cheapest live user listings (size FLOOR_QUEUE_SIZE)
//!
//!   SaleHistory          seeds [SALE_HISTORY]
//!                        ringbuffer of last SALE_HISTORY_SIZE qualifying
//!                        user-to-user sales (price + timestamps + parties).
//!
//!   FloorHistory         seeds [FLOOR_HISTORY]
//!                        7-day rolling ringbuffer of (timestamp,
//!                        anchor_price) snapshots. `current_anchor()` is the
//!                        floor reference used by breed + sweep pricing.
//!                        Snapshots are pushed at most once per 24h.
//!
//!   RebornEntry          seeds [REBORN_ENTRY, asset]
//!                        one per asset currently held by inventory_pda.
//!                        Tracks faction / quality / status (Listed |
//!                        Lootbox) / original_buy_price / expire_count.
//!
//!   LootboxClaim         seeds [LOOTBOX_CLAIM, user]
//!                        per-user reservation populated by a winning
//!                        loser-roll. Delivery via `claim_lootbox_nft`.
//! ```
//!
//! ## Money flow
//!
//! ```text
//!   protocol SOL fees (economy.rs)
//!         │
//!         ▼ DistributeSolFees splits `nft_market_making_pct`
//!   inventory_sweep_vault  ◀──┐
//!         │                    │ 50% (handle_inventory_proceeds)
//!         │ pays sweep buy ≤   │
//!         │ min(5%×vault,      │
//!         │     1.05×anchor)   │
//!         ▼                    │
//!   marketplace seller/fee     │
//!         │                    │
//!         ▼                    │
//!   inventory_pda receives NFT │
//!   inventory_pda receives sale proceeds when protocol listings sell
//!         │                    │
//!         │ 50% to sweep_vault │
//!         │ 50% to sol_treasury (handle_inventory_proceeds)
//!         └────────────────────┘
//!
//!   keeper bounties (out of inventory_sweep_vault):
//!     KEEPER_REWARD_LAMPORTS         = 500k  ; real sweep / snapshot / expire
//!     STALE_PURGE_KEEPER_REWARD…     = 20k   ; queue head cleanup only
//! ```
//!
//! ## Caller surface
//!
//! **User-signed (asset owner is signer):**
//! - `list_user_nft` / `cancel_user_listing` / `update_user_listing_price`
//!   wrap the marketplace ix and keep `FloorQueue` in sync atomically.
//! - `buy_user_listing` wraps `degenbtc_market::buy_listing`, pops the
//!   listing from `FloorQueue` if present, and records the sale into
//!   `SaleHistory` if it qualifies as a real-demand signal
//!   (user-to-user, ≥5min listing age).
//!
//! **Permissionless (anyone can crank):**
//! - `register_floor_listing` — pushes an existing live marketplace listing
//!   into the sorted `FloorQueue` (deduped, address-bound to the canonical
//!   HashBeast collection, escrow-owner checked, min-price gated). No direct
//!   reward; bots register to keep the queue accurate so they can win sweep
//!   races.
//! - `sweep_floor_lowest` — buys `floor_queue.entries[0]` from
//!   `inventory_sweep_vault`, disposes the swept asset (lootbox-push /
//!   relist / burn), pays full `KEEPER_REWARD_LAMPORTS`. If the head is
//!   stale (listing canceled out from under us), purges that one entry and
//!   pays the *reduced* `STALE_PURGE_KEEPER_REWARD_LAMPORTS` — this lower
//!   reward exists specifically to defuse a list→raw-cancel→purge spam
//!   attack that could otherwise drain the vault via keeper bounties. See
//!   the constant docs.
//! - `record_floor_snapshot` — once per 24h, computes a new anchor (sale
//!   median with queue/prior-anchor caps, or conservative queue fallback)
//!   and pushes it to `FloorHistory`.
//! - `expire_program_listing` — after a 7-day TTL, cancels a stuck
//!   inventory-owned listing and re-runs the disposition cascade. After
//!   `MAX_EXPIRES` strikes the asset is burned.
//! - `handle_inventory_proceeds` — drains accumulated sale lamports from
//!   inventory_pda, splitting 50/50 between sweep_vault and sol_treasury.
//! - `inventory_finalize_sale` — closes a sold inventory `RebornEntry`
//!   after verifying the on-chain asset owner is neither inventory_pda nor
//!   the canonical marketplace escrow PDA.
//! - `claim_lootbox_nft` — delivers a reserved loser-roll HashBeast to its
//!   recorded winner. Cranker can be anyone; recipient is fixed by the
//!   `LootboxClaim.user` address constraint on the `user` field.
//!
//! ## Oracle (anchor) design
//!
//! The "floor anchor" the protocol prices against is the head of
//! `FloorHistory`. Snapshots:
//!   - fire at most once per 24h (`FLOOR_SNAPSHOT_INTERVAL_SECS`),
//!   - prefer the median of recent user-to-user sales (≥17 samples in the
//!     last 24h, with ≥5min listing-age qualifier),
//!   - cap sale anchors to the registered queue median if cheaper sell-side
//!     supply exists,
//!   - bootstrap the first anchor at the marketplace minimum even if early
//!     sale/listing samples are higher,
//!   - cap upward sale-driven jumps to `FLOOR_ANCHOR_MAX_UPWARD_MOVE_BPS`
//!     per snapshot once an anchor exists,
//!   - fall back to `FloorQueue` median only for downward moves / day-zero
//!     bootstrap. Listing-only data can lower the anchor immediately, but
//!     cannot raise an existing anchor.
//!
//! Trend is computed across all populated history slots, clamped to ±100%
//! per 7d.
//!
//! **Wash-trade resistance:** the 5-min listing-age qualifier means each
//! manipulation cycle takes ≥5min; `MIN_SALES_FOR_ANCHOR = 17` means a
//! manipulator must own a majority of the 32-slot sale ringbuffer before the
//! sale median is used; queue-median and previous-anchor caps stop high-price
//! wash trades from immediately raising the vault's buy ceiling while cheaper
//! registered supply exists. Listing-only fallback is deliberately
//! conservative: it can only bootstrap from marketplace min price or move the
//! anchor down.
//!
//! ## Disposition cascade (sweep_floor_lowest + expire_program_listing)
//!
//! Both flows end in the same three-way decision for what to do with an
//! asset that's now owned by `inventory_pda`:
//!
//! 1. **Lootbox queue has space** → push to faction's `LootboxQueue`. The
//!    asset becomes a reward for a future round-losing player in that
//!    faction. `RebornEntry.status = Lootbox`.
//! 2. **Trend < `BURN_TREND_BPS_THRESHOLD` (-30%)** → burn the asset and
//!    close `RebornEntry`. Deep-bear deflationary lever.
//! 3. **Otherwise** → relist at `apply_markup(buy_price, markup_bps)` where
//!    `markup_bps = compute_relist_markup_bps(trend, expire_count)`. Each
//!    expire strike chips the markup down; final price is clamped at the
//!    marketplace minimum so bearish discounts cannot create unlistable
//!    program inventory. `RebornEntry.status = Listed`.
//!
//! ## Key invariants
//!
//! - `inventory_pool.total_count` ≤ `MAX_INVENTORY` (200). Enforced on every
//!   intake path.
//! - `inventory_sweep_vault` lamports always ≥ `MIN_SWEEP_RESERVE_LAMPORTS`
//!   after any payout (the floor inside `pay_keeper`).
//! - Per-sweep cost ≤ `min(vault × 5%, anchor × 1.05)`.
//! - Sweep/relist decisions require a fresh floor anchor
//!   (`FLOOR_ANCHOR_MAX_AGE_SECS`) before spending against trend/anchor data.
//! - `FloorQueue` is sorted ascending by price; `insert_floor_entry` enforces.
//! - No duplicate assets in `FloorQueue` (rejected at insert).
//! - Queue entries are accepted only while the underlying listing account is
//!   canonical and the asset is still owned by the marketplace escrow PDA.
//! - All program-owned NFT moves use the `inventory_pool` PDA seeds; no other
//!   PDA can authorize a transfer from inventory.
//! - Claim recipient is always seed-bound — `claim_lootbox_nft` enforces
//!   `user.key() == lootbox_claim.user` via `address = …` constraint.
//!
//! ## Future-AI-agent notes
//!
//! - **Never** add a permissionless path that pays from
//!   `inventory_sweep_vault` without:
//!   1. Rate-limiting how often a single attacker can invoke it.
//!   2. Tying the payout to bounded value (not just "anyone can call").
//!   3. Reviewing the attack-vector analogue of the list→raw-cancel→purge
//!      drain that motivated `STALE_PURGE_KEEPER_REWARD_LAMPORTS`.
//! - The `caller: Signer<'info>` field on permissionless ix is there so the
//!   tx has a fee-payer and a stable identity for keeper rewards. It is NOT
//!   an authority check — never assume the caller has any privilege.
//! - `inventory_pool` and `inventory_pda` share the same address; the
//!   former is the typed `Account<InventoryPool>` view, the latter the raw
//!   custody view. Both seed-pinned. Keep them mutually consistent in every
//!   Accounts struct that uses both.
//! - The `raw_*` helpers (raw_floor_entry / raw_record_sale etc.) deserialize
//!   in-place to keep the validator stack small. If you change any state-struct
//!   field offsets, update the corresponding `*_OFFSET` constants here.

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

fn assert_marketplace_escrow_pda(
    escrow: Pubkey,
    marketplace_config: Pubkey,
    asset: Pubkey,
) -> Result<()> {
    let (expected_escrow, _) = Pubkey::find_program_address(
        &[
            degenbtc_market::state::ESCROW_SEED,
            marketplace_config.as_ref(),
            asset.as_ref(),
        ],
        &degenbtc_market::ID,
    );
    require_keys_eq!(escrow, expected_escrow, ErrorCode::InvalidAccount);
    Ok(())
}

fn read_marketplace_listing(info: &AccountInfo) -> Result<degenbtc_market::state::Listing> {
    require!(
        info.owner == &degenbtc_market::ID,
        ErrorCode::InvalidAccount
    );
    require!(info.lamports() > 0, ErrorCode::InvalidAccount);
    let data = info.try_borrow_data()?;
    let mut slice: &[u8] = &data;
    degenbtc_market::state::Listing::try_deserialize(&mut slice).map_err(Into::into)
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

fn raw_floor_queue_median_entry(queue_info: &AccountInfo<'_>) -> Result<(FloorEntry, u32)> {
    require_raw_account::<FloorQueue>(queue_info, FloorQueue::LEN)?;
    let data = queue_info.try_borrow_data()?;
    let count = read_u8_at(&data, FLOOR_QUEUE_COUNT_OFFSET)? as usize;
    require!(count <= FLOOR_QUEUE_SIZE, ErrorCode::InvalidAccount);
    if count == 0 {
        return Ok((FloorEntry::default(), 0));
    }
    Ok((read_raw_floor_entry(&data, count / 2)?, count as u32))
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
    let (entry, count) = raw_floor_queue_median_entry(queue_info)?;
    Ok((entry.price, count))
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
    previous_anchor: u64,
    marketplace_min_price: u64,
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

    let (queue_median, queue_count) = raw_floor_queue_median(queue_info)?;

    if n >= MIN_SALES_FOR_ANCHOR {
        prices[..n].sort_unstable();
        let mut anchor = prices[n / 2];
        let mut source = 0u8; // sale median

        // If live registered listings are cheaper than the sale median, cap
        // the anchor to the listing median. This prevents a wash-sale burst
        // from raising the vault buy ceiling while cheaper sell-side supply
        // exists in the queue.
        if queue_median > 0 && queue_median < anchor {
            anchor = queue_median;
            source = 2; // sale median capped by queue median
        }

        // Day-zero anchor bootstrap is conservative for all sources, not
        // just queue fallback. Without prior history, even a 17-sale burst
        // could be collusive; start at the marketplace minimum and let
        // later qualified sales walk the anchor up through the daily cap.
        if previous_anchor == 0 && anchor > marketplace_min_price {
            anchor = marketplace_min_price;
            source = 3; // first snapshot capped to min price
        } else if previous_anchor > 0 {
            // If there is already an anchor, never allow a single snapshot to
            // raise it by more than the configured daily move cap.
            let capped = previous_anchor.saturating_add(
                ((previous_anchor as u128 * FLOOR_ANCHOR_MAX_UPWARD_MOVE_BPS as u128) / 10_000)
                    as u64,
            );
            if anchor > capped {
                anchor = capped;
                source = 4; // capped by prior anchor
            }
        }

        return Ok((anchor, source, n as u32));
    }

    if queue_median == 0 {
        return Ok((0, 1, queue_count));
    }

    // Thin-volume fallback is intentionally one-way:
    // - it may lower the anchor immediately if listings are cheaper,
    // - it may bootstrap from the marketplace min price on day zero,
    // - it may NOT raise an existing anchor from listings alone.
    // Upward moves require enough qualified sales.
    let (anchor, source) = if previous_anchor > 0 {
        if queue_median > previous_anchor {
            (previous_anchor, 4) // listing-only upward move capped to previous anchor
        } else {
            (queue_median, 1) // queue median
        }
    } else if queue_median > marketplace_min_price {
        (marketplace_min_price, 3) // initial queue fallback capped to min price
    } else {
        (queue_median, 1)
    };

    Ok((anchor, source, queue_count))
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

fn require_fresh_floor_anchor(history_info: &AccountInfo<'_>, now: i64) -> Result<()> {
    let last_snapshot_at = raw_floor_history_last_snapshot_at(history_info)?;
    require!(
        last_snapshot_at > 0 && now.saturating_sub(last_snapshot_at) <= FLOOR_ANCHOR_MAX_AGE_SECS,
        ErrorCode::FloorAnchorStale
    );
    Ok(())
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

fn assert_live_floor_entry(
    entry: FloorEntry,
    listing_info: &AccountInfo<'_>,
    asset_info: &AccountInfo<'_>,
    escrow_info: &AccountInfo<'_>,
    marketplace_config: Pubkey,
    inventory_key: Pubkey,
    collection: Pubkey,
) -> Result<()> {
    require_keys_eq!(listing_info.key(), entry.listing, ErrorCode::InvalidAccount);
    require_keys_eq!(asset_info.key(), entry.asset, ErrorCode::InvalidAccount);
    assert_listing_pda(entry.listing, marketplace_config, entry.asset)?;
    assert_marketplace_escrow_pda(escrow_info.key(), marketplace_config, entry.asset)?;

    let listing = read_marketplace_listing(listing_info)?;
    require_keys_eq!(listing.asset, entry.asset, ErrorCode::StaleFloorEntry);
    require_keys_eq!(listing.seller, entry.seller, ErrorCode::StaleFloorEntry);
    require!(
        listing.price_lamports == entry.price,
        ErrorCode::StaleFloorEntry
    );
    require!(
        listing.seller != inventory_key,
        ErrorCode::ProgramListingNotAllowed
    );
    require_keys_eq!(
        read_asset_owner(asset_info)?,
        escrow_info.key(),
        ErrorCode::StaleFloorEntry
    );
    assert_asset_collection(asset_info, collection)?;
    Ok(())
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
    assert_marketplace_config(
        &ctx.accounts.marketplace_config.to_account_info(),
        ctx.accounts.inventory_pool.marketplace_config,
    )?;

    // Caller must not be inventory_pda — that path is internal-only.
    require!(
        ctx.accounts.seller.key() != ctx.accounts.inventory_pool.key(),
        ErrorCode::ProgramListingNotAllowed
    );

    let cpi_ctx = CpiContext::new(
        ctx.accounts.marketplace_program.to_account_info(),
        degenbtc_market::cpi::accounts::ListNft {
            payer: ctx.accounts.seller.to_account_info(),
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
    let full_before_replace = (queue.entries_count as usize) >= FLOOR_QUEUE_SIZE;
    let previous_worst_price = if full_before_replace {
        Some(queue.entries[FLOOR_QUEUE_SIZE - 1].price)
    } else {
        None
    };
    let mut replaced_existing = false;
    if let Some(idx) = find_floor_entry_by_asset(queue, entry.asset) {
        replaced_existing = true;
        let old_listing = queue.entries[idx as usize].listing;
        remove_floor_entry_at(queue, idx);
        emit!(FloorEntryRemoved {
            listing: old_listing,
            asset: entry.asset,
            queue_index: idx,
            reason: 2, // relist / price refresh
            timestamp: now,
        });
    }
    if replaced_existing
        && previous_worst_price.is_some_and(|worst_price| entry.price >= worst_price)
    {
        msg!("ℹ️  Re-listed asset no longer beats the tracked queue; dropping floor entry");
        return Ok(());
    }
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
    assert_marketplace_config(
        &ctx.accounts.marketplace_config.to_account_info(),
        ctx.accounts.inventory_pool.marketplace_config,
    )?;
    let asset_key = ctx.accounts.hashbeast_asset.key();

    let cpi_ctx = CpiContext::new(
        ctx.accounts.marketplace_program.to_account_info(),
        degenbtc_market::cpi::accounts::CancelListing {
            payer: ctx.accounts.seller.to_account_info(),
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
    assert_marketplace_config(
        &ctx.accounts.marketplace_config.to_account_info(),
        ctx.accounts.inventory_pool.marketplace_config,
    )?;
    let collection = read_marketplace_config_collection(&ctx.accounts.marketplace_config)?;

    let listing_key = ctx.accounts.marketplace_listing.key();
    let seller_key = ctx.accounts.seller.key();
    let listing = read_marketplace_listing(&ctx.accounts.marketplace_listing.to_account_info())?;
    let asset_key = listing.asset;
    require_keys_eq!(listing.seller, seller_key, ErrorCode::InvalidAccount);
    require_keys_eq!(
        ctx.accounts.hashbeast_asset.key(),
        asset_key,
        ErrorCode::InvalidAccount
    );
    assert_listing_pda(
        listing_key,
        ctx.accounts.marketplace_config.key(),
        asset_key,
    )?;
    assert_asset_collection(&ctx.accounts.hashbeast_asset.to_account_info(), collection)?;

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
    let full_before_replace = (queue.entries_count as usize) >= FLOOR_QUEUE_SIZE;
    let previous_worst_price = if full_before_replace {
        Some(queue.entries[FLOOR_QUEUE_SIZE - 1].price)
    } else {
        None
    };

    // Pop existing entry (if any) and re-insert at the new sort position.
    let replaced_existing = if let Some(idx) = find_floor_entry_by_asset(queue, asset_key) {
        remove_floor_entry_at(queue, idx);
        emit!(FloorEntryRemoved {
            listing: listing_key,
            asset: asset_key,
            queue_index: idx,
            reason: 2, // price-update
            timestamp: now,
        });
        true
    } else {
        false
    };
    if replaced_existing
        && previous_worst_price.is_some_and(|worst_price| new_price_lamports >= worst_price)
    {
        msg!("ℹ️  Updated listing no longer beats the tracked queue; dropping floor entry");
        return Ok(());
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

    /// CHECK: mpl-core asset referenced by the marketplace listing. The
    /// handler reads the listing first and requires listing.asset == this key
    /// before using it as the queue lookup key.
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
    assert_marketplace_config(
        &ctx.accounts.marketplace_config.to_account_info(),
        ctx.accounts.inventory_pool.marketplace_config,
    )?;

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
    assert_listing_pda(
        ctx.accounts.marketplace_listing.key(),
        ctx.accounts.marketplace_config.key(),
        listing_asset,
    )?;
    assert_marketplace_escrow_pda(
        ctx.accounts.marketplace_escrow.key(),
        ctx.accounts.marketplace_config.key(),
        listing_asset,
    )?;
    let collection = read_marketplace_config_collection(&ctx.accounts.marketplace_config)?;
    assert_asset_collection(&ctx.accounts.hashbeast_asset.to_account_info(), collection)?;
    require!(
        listing_price <= max_price_lamports,
        ErrorCode::ListingPriceExceedsMax
    );

    let cpi_ctx = CpiContext::new(
        ctx.accounts.marketplace_program.to_account_info(),
        degenbtc_market::cpi::accounts::BuyListing {
            payer: ctx.accounts.buyer.to_account_info(),
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
    assert_marketplace_escrow_pda(
        ctx.accounts.marketplace_escrow.key(),
        ctx.accounts.marketplace_config.key(),
        asset_key,
    )?;
    require_keys_eq!(
        read_asset_owner(&ctx.accounts.hashbeast_asset.to_account_info())?,
        ctx.accounts.marketplace_escrow.key(),
        ErrorCode::StaleFloorEntry
    );

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

    /// CHECK: Marketplace escrow PDA for `hashbeast_asset`. Registration only
    /// accepts listings whose asset is still escrow-owned, so stale raw
    /// listing accounts cannot enter the floor queue.
    pub marketplace_escrow: UncheckedAccount<'info>,

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
    assert_marketplace_config(
        &ctx.accounts.marketplace_config.to_account_info(),
        ctx.accounts.inventory_pool.marketplace_config,
    )?;
    let marketplace_min_price =
        read_marketplace_min_price(&ctx.accounts.marketplace_config.to_account_info())?;
    let previous_anchor = raw_floor_history_current_anchor(&floor_history_info)?;
    let (median_entry, queue_count) =
        raw_floor_queue_median_entry(&ctx.accounts.floor_queue.to_account_info())?;
    if queue_count > 0 {
        let listing = ctx
            .accounts
            .queue_median_listing
            .as_ref()
            .ok_or(ErrorCode::InvalidAccount)?;
        let asset = ctx
            .accounts
            .queue_median_asset
            .as_ref()
            .ok_or(ErrorCode::InvalidAccount)?;
        let escrow = ctx
            .accounts
            .queue_median_escrow
            .as_ref()
            .ok_or(ErrorCode::InvalidAccount)?;
        let collection = read_marketplace_config_collection(&ctx.accounts.marketplace_config)?;
        assert_live_floor_entry(
            median_entry,
            &listing.to_account_info(),
            &asset.to_account_info(),
            &escrow.to_account_info(),
            ctx.accounts.marketplace_config.key(),
            ctx.accounts.inventory_pool.key(),
            collection,
        )?;
    }

    let (anchor, source, samples) = raw_compute_snapshot_anchor(
        &ctx.accounts.sale_history.to_account_info(),
        &ctx.accounts.floor_queue.to_account_info(),
        previous_anchor,
        marketplace_min_price,
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

    /// CHECK: marketplace_config PDA. Address and owner are checked against
    /// InventoryPool so the first thin-volume snapshot can cap itself at the
    /// canonical marketplace minimum price.
    pub marketplace_config: UncheckedAccount<'info>,

    /// CHECK: Optional live-check account for the current floor queue median.
    /// Required when FloorQueue has at least one entry.
    pub queue_median_listing: Option<UncheckedAccount<'info>>,

    /// CHECK: Optional mpl-core asset for the current floor queue median.
    pub queue_median_asset: Option<UncheckedAccount<'info>>,

    /// CHECK: Optional marketplace escrow PDA for the current floor queue median.
    pub queue_median_escrow: Option<UncheckedAccount<'info>>,

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
///     Pays a keeper bounty out of `inventory_sweep_vault`.
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
    assert_marketplace_config(
        &ctx.accounts.marketplace_config.to_account_info(),
        ctx.accounts.inventory_pool.marketplace_config,
    )?;

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
        require_keys_eq!(
            ctx.accounts.hashbeast_asset.key(),
            head.asset,
            ErrorCode::InvalidAccount
        );
        require_keys_eq!(
            ctx.accounts.seller.key(),
            head.seller,
            ErrorCode::InvalidAccount
        );
        assert_marketplace_escrow_pda(
            ctx.accounts.marketplace_escrow.key(),
            ctx.accounts.marketplace_config.key(),
            head.asset,
        )?;

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
            if !stale {
                match read_asset_owner(&ctx.accounts.hashbeast_asset.to_account_info()) {
                    Ok(owner) if owner == ctx.accounts.marketplace_escrow.key() => {}
                    _ => stale = true,
                }
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
        // Stale-head purge: pay a *reduced* keeper bounty so the cleanup is
        // still economically viable for honest bots (covers ~14k tx gas) but
        // can't be farmed via list → raw-cancel → purge cycles, which would
        // otherwise turn the cleanup path into a vault drain. See the docs
        // on `STALE_PURGE_KEEPER_REWARD_LAMPORTS` for full attack math.
        pay_keeper(
            &ctx.accounts.inventory_sweep_vault.to_account_info(),
            &ctx.accounts.caller.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            ctx.bumps.inventory_sweep_vault,
            STALE_PURGE_KEEPER_REWARD_LAMPORTS,
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
    // Defense-in-depth: caller must pass the correct seller account.
    require_keys_eq!(
        ctx.accounts.seller.key(),
        chosen_entry.seller,
        ErrorCode::InvalidAccount
    );
    assert_listing_pda(
        chosen_entry.listing,
        ctx.accounts.marketplace_config.key(),
        chosen_entry.asset,
    )?;
    let marketplace_min_price =
        read_marketplace_min_price(&ctx.accounts.marketplace_config.to_account_info())?;

    // Anchor for price ceiling.
    let floor_history_info = ctx.accounts.floor_history.to_account_info();
    require_fresh_floor_anchor(&floor_history_info, Clock::get()?.unix_timestamp)?;
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

    let queue_has_space = (ctx.accounts.lootbox_queue.filled_count as usize) < LOOTBOX_QUEUE_SIZE;
    let will_relist_after_sweep = !queue_has_space && trend_bps >= BURN_TREND_BPS_THRESHOLD;
    let relist_rent = if will_relist_after_sweep {
        Rent::get()?.minimum_balance(degenbtc_market::state::Listing::LEN)
    } else {
        0
    };

    // Vault reserve + per-tx cap. If this sweep will immediately relist the
    // acquired asset, the sweep vault also pays listing rent in the downstream
    // CPI, so include it in the solvency check.
    let vault_lamports = ctx.accounts.inventory_sweep_vault.lamports();
    let needed = chosen_entry
        .price
        .checked_add(MIN_SWEEP_RESERVE_LAMPORTS)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_add(KEEPER_REWARD_LAMPORTS)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_add(relist_rent)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    require!(vault_lamports >= needed, ErrorCode::SweepVaultBelowReserve);

    let tx_cap = ((vault_lamports as u128 * SWEEP_MAX_PCT_BPS as u128) / 10_000u128) as u64;
    require!(chosen_entry.price <= tx_cap, ErrorCode::SweepTxCapExceeded);

    // CPI buy_listing with the system-owned sweep vault as SOL payer and
    // inventory_pda as NFT recipient. Do not pre-fund inventory_pda: it is a
    // program-owned data account and cannot pay via System Program transfer.
    let pool_bump = ctx.accounts.inventory_pool.bump;
    let inventory_seeds_inner: &[&[u8]] = &[INVENTORY_POOL_SEED, &[pool_bump]];
    let sweep_bump = ctx.bumps.inventory_sweep_vault;
    let sweep_seeds_inner: &[&[u8]] = &[INVENTORY_SWEEP_VAULT_SEED, &[sweep_bump]];
    let inventory_signers: &[&[&[u8]]] = &[inventory_seeds_inner];
    let sweep_signers: &[&[&[u8]]] = &[sweep_seeds_inner];
    let inventory_and_sweep_signers: &[&[&[u8]]] = &[inventory_seeds_inner, sweep_seeds_inner];
    {
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.marketplace_program.to_account_info(),
            degenbtc_market::cpi::accounts::BuyListing {
                payer: ctx.accounts.inventory_sweep_vault.to_account_info(),
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
            sweep_signers,
        );
        degenbtc_market::cpi::buy_listing(cpi_ctx, chosen_entry.price)?;
    }

    // Pop the swept entry from the queue.
    raw_remove_floor_entry_at(&floor_queue_info, 0)?;

    let asset_key = ctx.accounts.hashbeast_asset.key();
    let faction_id = ctx.accounts.hashbeast_metadata.faction_id;
    let now = Clock::get()?.unix_timestamp;

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
        let was_created = crate::instructions::helper::init_pda_account_if_needed::<RebornEntry>(
            &ctx.accounts.caller.to_account_info(),
            &entry_info,
            &ctx.accounts.system_program.to_account_info(),
            entry_seeds,
            RebornEntry::LEN,
            &blank,
        )?;
        // A pre-existing RebornEntry means stale state (e.g. finalize_sale not
        // called after a previous sale). Refuse to overwrite — force cleanup first.
        require!(was_created, ErrorCode::InvalidState);

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
            Some(inventory_signers),
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
        let new_price = apply_markup(chosen_entry.price, markup_bps).max(marketplace_min_price);

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
        let was_created = crate::instructions::helper::init_pda_account_if_needed::<RebornEntry>(
            &ctx.accounts.caller.to_account_info(),
            &entry_info,
            &ctx.accounts.system_program.to_account_info(),
            entry_seeds,
            RebornEntry::LEN,
            &blank,
        )?;
        require!(was_created, ErrorCode::InvalidState);

        // CPI list_nft as inventory_pda. The just-closed listing PDA is the
        // same address as the new listing PDA (same seeds), so re-init is OK.
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.marketplace_program.to_account_info(),
            degenbtc_market::cpi::accounts::ListNft {
                payer: ctx.accounts.inventory_sweep_vault.to_account_info(),
                seller: ctx.accounts.inventory_pda.to_account_info(),
                marketplace_config: ctx.accounts.marketplace_config.to_account_info(),
                listing: ctx.accounts.marketplace_listing.to_account_info(),
                asset: ctx.accounts.hashbeast_asset.to_account_info(),
                collection: ctx.accounts.hashbeast_collection.to_account_info(),
                escrow: ctx.accounts.marketplace_escrow.to_account_info(),
                mpl_core_program: ctx.accounts.mpl_core_program.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            },
            inventory_and_sweep_signers,
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
    assert_marketplace_config(
        &ctx.accounts.marketplace_config.to_account_info(),
        ctx.accounts.inventory_pool.marketplace_config,
    )?;

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
    assert_marketplace_escrow_pda(
        ctx.accounts.marketplace_escrow.key(),
        ctx.accounts.marketplace_config.key(),
        asset_key,
    )?;
    require_keys_eq!(
        read_asset_owner(&ctx.accounts.hashbeast_asset.to_account_info())?,
        ctx.accounts.marketplace_escrow.key(),
        ErrorCode::StaleFloorEntry
    );
    require!(
        listing_age >= EXPIRE_GRACE_SECS,
        ErrorCode::ListingNotYetExpirable
    );

    let previous_price = entry.listing_price;
    let marketplace_min_price =
        read_marketplace_min_price(&ctx.accounts.marketplace_config.to_account_info())?;
    let planned_expire_count = entry.expire_count.saturating_add(1);
    let queue_has_space = (ctx.accounts.lootbox_queue.filled_count as usize) < LOOTBOX_QUEUE_SIZE;
    let trend_bps = ctx.accounts.floor_history.compute_trend_bps();
    if !queue_has_space && planned_expire_count < MAX_EXPIRES {
        require!(
            ctx.accounts.floor_history.current_anchor() >= SWEEP_MIN_ANCHOR_LAMPORTS,
            ErrorCode::SweepAnchorTooLow
        );
        require!(
            ctx.accounts.floor_history.last_snapshot_at > 0
                && Clock::get()?
                    .unix_timestamp
                    .saturating_sub(ctx.accounts.floor_history.last_snapshot_at)
                    <= FLOOR_ANCHOR_MAX_AGE_SECS,
            ErrorCode::FloorAnchorStale
        );
    }
    let will_relist_after_expire = !queue_has_space
        && planned_expire_count < MAX_EXPIRES
        && trend_bps >= BURN_TREND_BPS_THRESHOLD;
    if will_relist_after_expire {
        let relist_rent = Rent::get()?.minimum_balance(degenbtc_market::state::Listing::LEN);
        let needed = relist_rent
            .checked_add(MIN_SWEEP_RESERVE_LAMPORTS)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_add(KEEPER_REWARD_LAMPORTS)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        require!(
            ctx.accounts.inventory_sweep_vault.lamports() >= needed,
            ErrorCode::SweepVaultBelowReserve
        );
    }
    let pool_bump = ctx.accounts.inventory_pool.bump;
    let inventory_seeds_inner: &[&[u8]] = &[INVENTORY_POOL_SEED, &[pool_bump]];
    let sweep_bump = ctx.bumps.inventory_sweep_vault;
    let sweep_seeds_inner: &[&[u8]] = &[INVENTORY_SWEEP_VAULT_SEED, &[sweep_bump]];
    let inventory_signers: &[&[&[u8]]] = &[inventory_seeds_inner];
    let inventory_and_sweep_signers: &[&[&[u8]]] = &[inventory_seeds_inner, sweep_seeds_inner];

    // Cancel the existing listing — asset returns to inventory_pda. The
    // inventory PDA authorizes the asset, while the system-owned sweep vault
    // pays any mpl-core reallocation rent.
    {
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.marketplace_program.to_account_info(),
            degenbtc_market::cpi::accounts::CancelListing {
                payer: ctx.accounts.inventory_sweep_vault.to_account_info(),
                seller: ctx.accounts.inventory_pda.to_account_info(),
                marketplace_config: ctx.accounts.marketplace_config.to_account_info(),
                listing: ctx.accounts.marketplace_listing.to_account_info(),
                asset: ctx.accounts.hashbeast_asset.to_account_info(),
                collection: ctx.accounts.hashbeast_collection.to_account_info(),
                escrow: ctx.accounts.marketplace_escrow.to_account_info(),
                mpl_core_program: ctx.accounts.mpl_core_program.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            },
            inventory_and_sweep_signers,
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

    emit!(ProgramListingExpired {
        asset: asset_key,
        previous_list_price: previous_price,
        expire_count_after,
        keeper: ctx.accounts.caller.key(),
        timestamp: now,
    });

    let force_burn = expire_count_after >= MAX_EXPIRES;

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
            Some(inventory_signers),
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
            Some(inventory_signers),
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
        let new_price = apply_markup(original_buy_price, markup_bps).max(marketplace_min_price);

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.marketplace_program.to_account_info(),
            degenbtc_market::cpi::accounts::ListNft {
                payer: ctx.accounts.inventory_sweep_vault.to_account_info(),
                seller: ctx.accounts.inventory_pda.to_account_info(),
                marketplace_config: ctx.accounts.marketplace_config.to_account_info(),
                listing: ctx.accounts.marketplace_listing.to_account_info(),
                asset: ctx.accounts.hashbeast_asset.to_account_info(),
                collection: ctx.accounts.hashbeast_collection.to_account_info(),
                escrow: ctx.accounts.marketplace_escrow.to_account_info(),
                mpl_core_program: ctx.accounts.mpl_core_program.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            },
            inventory_and_sweep_signers,
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

    // inventory_pda is the program-owned InventoryPool data account. The
    // System Program cannot debit it like a wallet, so route proceeds by
    // directly moving lamports while preserving rent exemption.
    {
        let mut inventory_lamports = pool_info.lamports.borrow_mut();
        **inventory_lamports = inventory_lamports
            .checked_sub(available)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    if to_sweep > 0 {
        let sweep_lamports = ctx.accounts.inventory_sweep_vault.lamports();
        **ctx
            .accounts
            .inventory_sweep_vault
            .to_account_info()
            .lamports
            .borrow_mut() = sweep_lamports
            .checked_add(to_sweep)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    if to_protocol > 0 {
        let treasury_lamports = ctx.accounts.sol_treasury.lamports();
        **ctx
            .accounts
            .sol_treasury
            .to_account_info()
            .lamports
            .borrow_mut() = treasury_lamports
            .checked_add(to_protocol)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
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
    let (expected_escrow, _) = Pubkey::find_program_address(
        &[
            degenbtc_market::state::ESCROW_SEED,
            ctx.accounts.inventory_pool.marketplace_config.as_ref(),
            ctx.accounts.reborn_entry.asset.as_ref(),
        ],
        &degenbtc_market::ID,
    );
    let owner = read_asset_owner(&ctx.accounts.hashbeast_asset.to_account_info())?;
    require!(
        owner != inventory_key,
        ErrorCode::AssetStillOwnedByInventory
    );
    require!(owner != expected_escrow, ErrorCode::AssetStillListed);

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
    // The asset's intake faction (recorded on `RebornEntry`) must match the
    // winner's faction recorded at loser-roll time. This is invariant under
    // current flow — each lootbox_queue PDA is keyed by faction, and the
    // loser-roll only ever pops from the player's home queue. Asserting it
    // here makes the invariant explicit so a future cross-faction reroll
    // path can't quietly route an asset to the wrong indexer bucket.
    require!(
        entry.faction_id == claim.faction_id,
        ErrorCode::InvalidFactionId
    );
    require!(
        ctx.accounts.hashbeast_collection.is_some(),
        ErrorCode::InvalidAccount
    );
    require_keys_eq!(
        read_asset_owner(&ctx.accounts.hashbeast_asset.to_account_info())?,
        ctx.accounts.inventory_pda.key(),
        ErrorCode::AssetNotInInventory
    );

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

    /// Read-only config that pins the canonical HashBeast collection.
    #[account(
        seeds = [HASHBEAST_CONFIG_SEED.as_ref()],
        bump = hashbeast_config.bump,
    )]
    pub hashbeast_config: Box<Account<'info, HashBeastConfig>>,

    /// CHECK: HashBeast collection (required by mpl-core on transfer). The
    /// Option wrapper is only for mpl-core builder compatibility; handler
    /// requires Some and Anchor address-checks it against HashBeastConfig.
    #[account(
        mut,
        address = hashbeast_config.hashbeast_collection @ ErrorCode::InvalidAccount,
    )]
    pub hashbeast_collection: Option<UncheckedAccount<'info>>,

    /// CHECK: Metaplex Core program.
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
