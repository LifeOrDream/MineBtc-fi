# NFT Marketplace + Recycle Pool — Implementation Spec

> **Status:** implementation-ready. Pre-launch — no migration shims, break what needs breaking.

## 0. Principles

- **Separate program**: `degenbtc_market` is a standalone Solana program. mineBTC CPIs into it. Marketplace can be upgraded independently of mineBTC.
- **Minimal surface**: `list_nft`, `cancel_listing`, `update_listing_price`, `buy_listing`. No bids, no expiry, no auctions, no on-chain decay.
- **3% flat fee**, no royalty plugin. Fee routes to mineBTC `fee_recipient`.
- **No backward compatibility**. We are pre-launch.
- **Recycle replaces burn**. `send_to_heaven` becomes `recycle_doge` — NFT goes to inventory custody instead of burning.

---

## 1. Program Layout

```
programs/
├── mineBTC/                            # existing program — modified
│   └── src/
│       ├── instructions/
│       │   ├── doges.rs                # send_to_heaven → recycle_doge
│       │   ├── economy.rs              # unchanged; protocol-fee router stays as-is
│       │   ├── tax.rs                  # MODIFIED: deprecate degenBTC nft_floor_sweep_vault path
│       │   ├── faction_war.rs          # MODIFIED: emit lootbox eligibility flag in claim
│       │   ├── marketplace_cpi.rs      # NEW: CPI wrappers + lootbox drop + proceeds split
│       │   └── ...
│       ├── state.rs                    # NEW accounts: InventoryPool, RecycledEntry, MarketMetrics
│       ├── lib.rs                      # add modules + dispatch
│       └── mpl_core_helpers.rs         # already has transfer + burn helpers — reuse
│
└── degenbtc_market/                    # NEW standalone program
    ├── Cargo.toml
    └── src/
        ├── lib.rs                      # entrypoints
        ├── state.rs                    # MarketplaceConfig, Listing
        ├── errors.rs
        ├── events.rs
        └── instructions/
            ├── initialize.rs
            ├── update_config.rs        # set fee, recipient, min_price, enabled
            ├── list_nft.rs
            ├── cancel_listing.rs
            ├── update_listing_price.rs
            └── buy_listing.rs
```

---

## 2. `degenbtc_market` Program

### 2.1 State

#### `MarketplaceConfig` (PDA: `[b"marketplace-config", collection_mint]`)
```rust
#[account]
pub struct MarketplaceConfig {
    pub bump: u8,
    pub admin: Pubkey,
    pub enabled: bool,
    /// Verified Doge collection mint (mpl-core CollectionV1)
    pub collection_mint: Pubkey,
    /// 300 = 3.00%
    pub fee_bps: u16,
    /// SOL recipient for marketplace fees
    pub fee_recipient: Pubkey,
    /// Hard floor on listing prices (lamports)
    pub min_price_lamports: u64,
    /// Cached MPL Core program ID for validation
    pub mpl_core_program: Pubkey,
}
```
LEN = 8 + 1 + 32 + 1 + 32 + 2 + 32 + 8 + 32 = **148 bytes**.

#### `Listing` (PDA: `[b"listing", marketplace_config, asset]`)
```rust
#[account]
pub struct Listing {
    pub bump: u8,
    pub seller: Pubkey,
    pub asset: Pubkey,
    pub price_lamports: u64,
    pub created_at: i64,
}
```
LEN = 8 + 1 + 32 + 32 + 8 + 8 = **89 bytes**. No `is_active` flag — listing exists ⇔ active. Cancelling or buying closes the account.

#### Escrow ownership
**There is no separate escrow account to init.** When a listing is created, the mpl-core asset's `owner` field is set to the deterministic PDA `[b"escrow", marketplace_config, asset]`. We never `init` that address — we just use it as the new owner via mpl-core `TransferV1`. On cancel/buy, the program signs as that PDA to transfer the asset back out.

Escrow PDA bump is **re-derived** in cancel/buy (cheap one-time computation per ix). We don't store it in `Listing` to keep that account compact.

### 2.2 Errors

```rust
#[error_code]
pub enum MarketError {
    #[msg("Marketplace is disabled")]
    MarketplaceDisabled,
    #[msg("Price below minimum")]
    PriceTooLow,
    #[msg("Fee exceeds max 10%")]
    FeeTooHigh,
    #[msg("Asset not in registered collection")]
    NotCollectionMember,
    #[msg("Seller mismatch")]
    SellerMismatch,
    #[msg("Insufficient buyer funds")]
    InsufficientFunds,
    #[msg("Invalid MPL Core program")]
    InvalidMplCoreProgram,
    #[msg("Admin only")]
    Unauthorized,
    #[msg("Asset has unsupported plugin")]
    UnsupportedPlugin,
    #[msg("Math overflow")]
    MathOverflow,
}
```

### 2.3 Events

```rust
#[event] pub struct MarketplaceInitialized { pub config: Pubkey, pub collection_mint: Pubkey, pub fee_bps: u16 }
#[event] pub struct MarketplaceConfigUpdated { pub config: Pubkey, pub fee_bps: u16, pub fee_recipient: Pubkey, pub enabled: bool, pub min_price_lamports: u64 }
#[event] pub struct NftListed { pub asset: Pubkey, pub seller: Pubkey, pub price_lamports: u64, pub timestamp: i64 }
#[event] pub struct ListingCancelled { pub asset: Pubkey, pub seller: Pubkey, pub timestamp: i64 }
#[event] pub struct ListingPriceUpdated { pub asset: Pubkey, pub seller: Pubkey, pub new_price_lamports: u64, pub timestamp: i64 }
#[event] pub struct NftSold { pub asset: Pubkey, pub buyer: Pubkey, pub seller: Pubkey, pub price_lamports: u64, pub fee_lamports: u64, pub timestamp: i64 }
```

### 2.4 Instructions

#### 2.4.1 `initialize_marketplace`
```rust
pub fn initialize_marketplace(
    ctx: Context<InitializeMarketplace>,
    fee_bps: u16,
    fee_recipient: Pubkey,
    min_price_lamports: u64,
    mpl_core_program: Pubkey,
) -> Result<()>
```
Accounts:
| Account | Mut | Signer | Notes |
|---|---|---|---|
| `payer` | ✓ | ✓ | rent payer |
| `admin` | | ✓ | becomes config admin |
| `marketplace_config` | ✓ | | PDA init `[b"marketplace-config", collection_mint]` |
| `collection_mint` | | | mpl-core `CollectionV1`, validated |
| `system_program` | | | |

Constraints: `fee_bps <= 1000`. `collection_mint` must deserialize as `CollectionV1`. Initial `enabled = true`.

#### 2.4.2 `update_marketplace_config`
```rust
pub fn update_marketplace_config(
    ctx: Context<UpdateMarketplaceConfig>,
    fee_bps: Option<u16>,
    fee_recipient: Option<Pubkey>,
    min_price_lamports: Option<u64>,
    enabled: Option<bool>,
) -> Result<()>
```
Accounts: `admin: Signer`, `marketplace_config: mut`. Constraint: `admin == config.admin`. Each `Some` field overwrites; `None` leaves alone. Emits `MarketplaceConfigUpdated`.

#### 2.4.3 `list_nft`
```rust
pub fn list_nft(ctx: Context<ListNft>, price_lamports: u64) -> Result<()>
```
Accounts:
| Account | Mut | Signer | Notes |
|---|---|---|---|
| `seller` | ✓ | ✓ | pays listing rent; current asset owner |
| `marketplace_config` | | | |
| `listing` | ✓ | | `init` PDA `[b"listing", config, asset]`, payer = seller |
| `asset` | ✓ | | mpl-core `BaseAssetV1` |
| `collection` | ✓ | | mpl-core `CollectionV1`, must match `config.collection_mint` |
| `mpl_core_program` | | | must match `config.mpl_core_program` |
| `system_program` | | | |

Logic:
1. `require!(config.enabled)`.
2. `require!(price_lamports >= config.min_price_lamports)`.
3. Deserialize `asset` as `BaseAssetV1`. `require!(asset.owner == seller.key())`.
4. `require!(asset.update_authority == UpdateAuthority::Collection(config.collection_mint))`.
5. **Plugin gate**: reject assets with a `Royalties` or `FreezeDelegate` plugin attached at asset level (defensive — prevents transfer surprises). Plugins like `Attributes` are fine.
6. mpl-core `TransferV1` from `seller` → escrow PDA `[b"escrow", config, asset]` (signer = seller).
7. Init `Listing { seller, asset, price_lamports, created_at: now }`.
8. Emit `NftListed`.

**Why we don't gate "is gameplay-locked"**: a Doge that's locked for gameplay isn't owned by the user — it's owned by mineBTC's `doge_custody` PDA. Step 3's owner check naturally blocks it. We document this rather than re-checking.

**P2P sale of an NFT with `accumulated_val > 0`**: the new owner inherits the `DogeMetadata` (it's per-mint and the marketplace doesn't touch mineBTC state). This is intentional — it's the seller's choice whether to claim/zero out via `recycle_doge` first. Document on the FE.

#### 2.4.4 `cancel_listing`
```rust
pub fn cancel_listing(ctx: Context<CancelListing>) -> Result<()>
```
Accounts:
| Account | Mut | Signer | Notes |
|---|---|---|---|
| `seller` | ✓ | ✓ | receives rent refund + asset |
| `marketplace_config` | | | |
| `listing` | ✓ | | `close = seller`, must match seeds |
| `asset` | ✓ | | |
| `collection` | ✓ | | |
| `mpl_core_program` | | | |
| `system_program` | | | |

Logic:
1. `require!(listing.seller == seller.key())`.
2. Re-derive escrow PDA bump.
3. mpl-core `TransferV1` from escrow PDA → `seller` (signer = escrow PDA seeds).
4. Anchor closes `listing` to `seller` (rent refund).
5. Emit `ListingCancelled`.

#### 2.4.5 `update_listing_price`
```rust
pub fn update_listing_price(ctx: Context<UpdateListingPrice>, new_price_lamports: u64) -> Result<()>
```
Accounts: `seller: Signer`, `marketplace_config`, `listing: mut`.

Logic: gate on `seller == listing.seller` and `new_price >= config.min_price_lamports`. Mutate `listing.price_lamports`. Emit `ListingPriceUpdated`. Trivial.

#### 2.4.6 `buy_listing`
```rust
pub fn buy_listing(ctx: Context<BuyListing>) -> Result<()>
```
Accounts:
| Account | Mut | Signer | Notes |
|---|---|---|---|
| `buyer` | ✓ | ✓ | pays SOL, receives asset |
| `seller` | ✓ | | receives proceeds + listing rent refund |
| `marketplace_config` | | | |
| `listing` | ✓ | | `close = seller` |
| `asset` | ✓ | | currently owned by escrow PDA |
| `collection` | ✓ | | |
| `fee_recipient` | ✓ | | must match `config.fee_recipient` |
| `mpl_core_program` | | | |
| `system_program` | | | |

Logic:
```
require!(config.enabled);
let price = listing.price_lamports;
let fee = price.checked_mul(config.fee_bps as u64)? / 10_000;
let to_seller = price.checked_sub(fee)?;

// 1. Buyer pays SOL (system_program transfer, no PDA signer needed for buyer)
system::transfer(buyer -> fee_recipient, fee);
system::transfer(buyer -> seller, to_seller);

// 2. Asset escrow -> buyer (signer: escrow PDA seeds)
mpl_core::transfer_v1(asset, escrow_pda -> buyer);

// 3. Listing closes to seller (Anchor handles via close = seller)

emit!(NftSold { asset, buyer, seller, price, fee, timestamp });
```

Notes:
- **Listing rent refund** goes to `seller` regardless of buyer. For inventory listings, seller IS `inventory_pda`, so rent stays in mineBTC's custody — handled by `handle_inventory_proceeds` later.
- **Front-running** is acceptable for v1: cheapest-listing griefing exists on every NFT marketplace and bots already do it on Tensor/ME.

---

## 3. `mineBTC` Program Changes

### 3.1 New State Accounts

#### `InventoryPool` (PDA: `[b"inventory-pool"]`)
```rust
#[account]
pub struct InventoryPool {
    pub bump: u8,
    pub crank_authority: Pubkey,
    /// Cached pubkey of the marketplace program for CPI validation
    pub marketplace_program: Pubkey,
    /// Cached marketplace config PDA
    pub marketplace_config: Pubkey,

    /// Live inventory counts
    pub total_count: u32,         // pending + lootbox + listed
    pub pending_count: u32,       // recycled, awaiting disposition
    pub lootbox_count: u32,       // earmarked for drops
    pub listed_count: u32,        // currently listed on marketplace

    /// Lifetime counters (analytics)
    pub total_recycled: u64,
    pub total_listed: u64,
    pub total_sold: u64,
    pub total_dropped: u64,
    pub total_swept: u64,         // bought from market into inventory
}
```
LEN = 8 + 1 + 32 + 32 + 32 + 4*4 + 5*8 = **161 bytes**.

#### `RecycledEntry` (PDA: `[b"recycled-entry", asset]`)
One per Doge currently held by inventory.
```rust
#[account]
pub struct RecycledEntry {
    pub bump: u8,
    pub asset: Pubkey,
    pub faction_id: u8,
    /// Snapshot at intake — used for listing-price modeling and lootbox priority
    pub quality_score: u16,           // 0..=10_000
    pub recycled_at: i64,
    pub status: u8,                   // see RecycledStatus
    pub listing_price: u64,           // 0 if not listed
    /// 0=recycled (send_to_heaven), 1=swept (bought from market)
    pub origin: u8,
}
```
LEN = 8 + 1 + 32 + 1 + 2 + 8 + 1 + 8 + 1 = **62 bytes**.

```rust
pub enum RecycledStatus {
    Pending  = 0,
    Listed   = 1,
    Lootbox  = 2,
    // Sold and Dropped close the account; never persist these on chain
}
```

#### `MarketMetrics` (PDA: `[b"market-metrics"]`)
```rust
#[account]
pub struct MarketMetrics {
    pub bump: u8,
    pub demand_index: i16,            // [-100, +100]
    pub last_updated: i64,
    pub floor_price_lamports: u64,
    pub avg_sell_price_24h: u64,
    pub listings_count: u32,          // mirrored from indexer
    pub sales_count_24h: u32,
    pub crank_authority: Pubkey,
    pub _reserved: [u8; 32],
}
```

### 3.2 New Instructions in `mineBTC`

#### 3.2.1 `init_inventory_pool` (admin-only, one-time)
Inits `InventoryPool`, `MarketMetrics`. Caches `marketplace_program` and `marketplace_config` Pubkeys. Sets `crank_authority`.

#### 3.2.2 `update_market_metrics` (crank-only)
```rust
pub fn update_market_metrics(
    ctx: Context<UpdateMarketMetrics>,
    demand_index: i16,
    floor_price_lamports: u64,
    avg_sell_price_24h: u64,
    listings_count: u32,
    sales_count_24h: u32,
) -> Result<()>
```
Constraints: `signer == metrics.crank_authority`, `demand_index ∈ [-100, 100]`, `last_updated` set to `Clock::get()?.unix_timestamp`.

#### 3.2.3 `recycle_doge` (replaces `int_send_to_heaven`)

Replaces the burn in `send_to_heaven`. The user still receives `accumulated_val` degenBTC. NFT transfers to `inventory_pda` and a `RecycledEntry` is created.

```rust
pub fn recycle_doge(ctx: Context<RecycleDoge>) -> Result<()> {
    let metadata = &mut ctx.accounts.doge_metadata;
    require!(
        metadata.incubated_player_data == Pubkey::default(),
        ErrorCode::DogeAlreadyAtGuard
    );
    let accumulated = metadata.accumulated_val;
    let asset = metadata.mint;

    // 1. Pay user accumulated_val (same logic as old send_to_heaven)
    if accumulated > 0 {
        token_2022::transfer_checked(
            cpi_ctx_with_vault_authority(...),
            accumulated, MINEBTC_DECIMALS,
        )?;
        ctx.accounts.mine_btc_mining.total_tokens_distributed = ...;
    }

    // 2. Transfer NFT user -> inventory_pda (mpl-core)
    transfer_mpl_core_asset(
        &ctx.accounts.doge_asset,
        Some(&ctx.accounts.doge_collection),
        &ctx.accounts.user,                  // payer
        &ctx.accounts.user,                  // authority (current owner)
        &ctx.accounts.inventory_pda,         // new owner
        &ctx.accounts.mpl_core_program,
        None,
    )?;

    // 3. Reset metadata for rebirth
    let quality = compute_quality_score(metadata);
    metadata.accumulated_val = 0;
    metadata.multiplier = BASE_MULTIPLIER;          // 1.0x
    metadata.xp = 0;                                // also reset XP — see §3.5
    metadata.incubated_player_data = Pubkey::default();
    // breed_count, dna, mom, dad, faction_id PRESERVED

    // 4. Init RecycledEntry
    let entry = &mut ctx.accounts.recycled_entry;
    entry.bump = ctx.bumps.recycled_entry;
    entry.asset = asset;
    entry.faction_id = metadata.faction_id;
    entry.quality_score = quality;
    entry.recycled_at = Clock::get()?.unix_timestamp;
    entry.status = RecycledStatus::Pending as u8;
    entry.listing_price = 0;
    entry.origin = 0; // recycled

    // 5. Update pool
    let pool = &mut ctx.accounts.inventory_pool;
    pool.total_count = pool.total_count.checked_add(1)?;
    pool.pending_count = pool.pending_count.checked_add(1)?;
    pool.total_recycled = pool.total_recycled.checked_add(1)?;

    emit!(DogeRecycled {
        asset,
        former_owner: ctx.accounts.user.key(),
        accumulated_val: accumulated,
        quality_score: quality,
        timestamp: Clock::get()?.unix_timestamp,
    });
    Ok(())
}
```

Accounts:
| Account | Mut | Signer | Notes |
|---|---|---|---|
| `user` | ✓ | ✓ | rent payer + receives degenBTC |
| `inventory_pool` | ✓ | | counters |
| `inventory_pda` | ✓ | | new asset owner; PDA `[b"inventory-pool"]` (same as pool, dual-purpose: state account + custody) |
| `recycled_entry` | ✓ | | `init` PDA `[b"recycled-entry", asset]`, payer = user |
| `doge_metadata` | ✓ | | existing |
| `doge_asset` | ✓ | | mpl-core asset |
| `doge_collection` | ✓ | | |
| `minebtc_token_vault` | ✓ | | source of accumulated_val payout |
| `user_token_account` | ✓ | | dest |
| `vault_authority` | | | PDA, signs token transfer |
| `token_mint` | | | |
| `mine_btc_mining` | ✓ | | total_tokens_distributed bump |
| `mpl_core_program` | | | |
| `token_program` | | | token-2022 |
| `system_program` | | | |

**Quality score formula** (deterministic, used by off-chain disposition + lootbox priority):
```
quality_score =
      ((multiplier - BASE_MULTIPLIER).min(BASE_MULTIPLIER * 4) * 6_000) / (BASE_MULTIPLIER * 4)
    + (xp.min(MAX_XP_FOR_QUALITY) * 3_000) / MAX_XP_FOR_QUALITY
    + (breed_count.min(5) as u16 * 200);          // 0..=1000
// clamped to [0, 10_000]
```
Tunable constants live in `state.rs`. Anyone reading the on-chain `quality_score` can sort/threshold without re-fetching `DogeMetadata`.

#### 3.2.4 CPI wrappers in `marketplace_cpi.rs`

##### `inventory_list_nft`
```rust
pub fn inventory_list_nft(
    ctx: Context<InventoryListNft>,
    price_lamports: u64,
) -> Result<()>
```
Accounts (extends with all `degenbtc_market::list_nft` accounts):
| Account | Mut | Signer | Notes |
|---|---|---|---|
| `crank_authority` | ✓ | ✓ | rent payer for marketplace listing |
| `inventory_pool` | ✓ | | |
| `recycled_entry` | ✓ | | status Pending or Lootbox → Listed |
| `inventory_pda` | ✓ | | signs as marketplace `seller` via PDA seeds |
| (all `list_nft` accounts) | | | passed through |

Logic:
1. `require!(crank_authority == pool.crank_authority)`.
2. `require!(entry.status ∈ {Pending, Lootbox})`.
3. CPI `degenbtc_market::list_nft` with signer seeds for `inventory_pda` (`[b"inventory-pool", &[pool.bump]]`).
4. `entry.status = Listed; entry.listing_price = price`.
5. Update pool counters; `total_listed += 1`.
6. Emit `InventoryListed { asset, price, source_status }`.

##### `inventory_cancel_listing`
Mirrors above but CPI to `cancel_listing`. Status flips back to `Pending`. Decrement `listed_count`, increment `pending_count`.

##### `inventory_update_price`
CPI to `update_listing_price`. Mutate `entry.listing_price`.

##### `inventory_buy_listing`  (sweep path)
```rust
pub fn inventory_buy_listing(ctx: Context<InventoryBuyListing>) -> Result<()>
```
Accounts:
| Account | Mut | Signer | Notes |
|---|---|---|---|
| `crank_authority` | ✓ | ✓ | |
| `inventory_pool` | ✓ | | |
| `inventory_sweep_vault` | ✓ | | SOL source — see §3.3 |
| `inventory_pda` | ✓ | | becomes new asset owner |
| `recycled_entry` | ✓ | | `init` PDA `[b"recycled-entry", asset]`, payer = inventory_sweep_vault (or crank_authority — see note) |
| (all `buy_listing` accounts where buyer = inventory_pda) | | | |

Critical: `buy_listing` requires the buyer to sign for system::transfer. So `inventory_pda` must be the `buyer` and signer (PDA seeds).

But `buyer` pays SOL. PDAs can pay SOL via `invoke_signed` system::transfer. We do **not** want SOL coming directly from `inventory_pda` (it's a mineBTC state account holding NFTs, not a SOL pool). Instead, **before** the CPI, we transfer SOL `inventory_sweep_vault → inventory_pda` (using `inventory_sweep_vault` PDA seeds), then the CPI pulls from `inventory_pda` to seller + fee_recipient.

Logic:
```
require!(crank == pool.crank_authority);
require!(price <= di_max_sweep_price);  // optional, also enforced off-chain

// Pre-fund inventory_pda
system::transfer_signed(inventory_sweep_vault -> inventory_pda, listing.price_lamports);

// CPI buy
degenbtc_market::buy_listing { buyer: inventory_pda (signer via seeds), ... };

// Init RecycledEntry for swept asset
entry.origin = 1;
entry.status = Pending;

// Pool counters
pool.total_count += 1; pool.pending_count += 1; pool.total_swept += 1;
```

Note on rent payer for new `recycled_entry`: pay from `crank_authority` (it's recovered when the entry closes on drop/sale). Cheaper than drawing from sweep vault.

#### 3.2.5 `process_lootbox_drops` (separate cranker ix — NOT inside claim)

Why separate: drop NFT is selected by VRF at runtime; can't pre-declare its accounts in the user's claim tx. Instead, `claim_faction_war_rewards_internal` flags eligibility on `PlayerData`, and a cranker bot processes pending drops in its own tx.

##### Eligibility flag added to claim
In `faction_war.rs::claim_faction_war_rewards_internal`, after rewards:
```rust
if user_eligible_for_lootbox(player_data, user_faction_war_bets, faction_war_state) {
    player_data.pending_lootbox_roll = Some(LootboxRollClaim {
        faction_war_id,
        faction_id: player_data.faction_id,
        cycle_seed: faction_war_state.entropy_seed,  // already exists
    });
}
```
Add `pending_lootbox_roll: Option<LootboxRollClaim>` to `PlayerData` (~1 + 8 + 1 + 32 = 42 bytes new). LEN bump in state.rs.

Eligibility: `user_bet.has_correct_call && player_data.gameplay_doge != default && pool.lootbox_count >= MIN_LOOTBOX_POOL && (now - player_data.last_lootbox_drop_at) >= LOOTBOX_COOLDOWN_SECONDS && !user_already_owns_max_doges`.

##### `process_lootbox_drops`
```rust
pub fn process_lootbox_drops(ctx: Context<ProcessLootboxDrops>) -> Result<()>
```
Accounts:
| Account | Mut | Signer | Notes |
|---|---|---|---|
| `crank_authority` | ✓ | ✓ | |
| `inventory_pool` | ✓ | | |
| `market_metrics` | | | for DI-driven roll |
| `winner_player_data` | ✓ | | the recipient |
| `winner_wallet` | ✓ | | mpl-core new owner |
| `winner_doge_token_account` | ✓ | | (n/a for mpl-core; placeholder if we add token-side later) |
| `recycled_entry` | ✓ | | `close = winner_wallet` (rent refund to winner — small UX win) |
| `doge_asset` | ✓ | | mpl-core, currently owned by inventory_pda |
| `doge_metadata` | ✓ | | reset to fresh state for new owner |
| `doge_collection` | ✓ | | |
| `inventory_pda` | ✓ | | signs transfer |
| `mpl_core_program` | | | |
| `system_program` | | | |
| `slot_hashes` | | | for entropy |

Cranker selection algorithm (off-chain) — picks the (player, asset) tuple to drop:
1. Read `pending_lootbox_roll` for the player.
2. Filter inventory `RecycledEntry`s with `status == Lootbox` and `faction_id == player.faction_id`.
3. VRF pick weighted by `quality_score` × player's correct-direction SOL bet share.
4. Submit `process_lootbox_drops` with selected accounts.

On-chain logic:
```
require!(crank == pool.crank_authority);
let roll = player_data.pending_lootbox_roll.ok_or(NoLootboxRoll)?;

// Re-derive entropy on-chain to defeat cranker manipulation
let entropy = keccak256(&[
    &roll.cycle_seed,
    &winner_wallet.key().to_bytes(),
    &recent_slothash,
]);
let drop_chance_bps = compute_drop_chance(metrics, pool, player_quality_factor);
let roll_value = u16::from_le_bytes([entropy[0], entropy[1]]) % 10_000;

require!(entry.status == Lootbox);
require!(entry.faction_id == roll.faction_id);

if roll_value < drop_chance_bps {
    // Transfer asset inventory_pda -> winner_wallet
    transfer_mpl_core_asset(asset, ..., authority = inventory_pda (signed), new_owner = winner_wallet, ...)?;

    // Reset DogeMetadata for new owner
    doge_metadata.accumulated_val = 0;
    doge_metadata.multiplier = BASE_MULTIPLIER;
    doge_metadata.xp = 0;
    doge_metadata.incubated_player_data = Pubkey::default();
    // dna, mom, dad, breed_count, faction_id preserved

    pool.lootbox_count -= 1;
    pool.total_count -= 1;
    pool.total_dropped += 1;

    player_data.last_lootbox_drop_at = now;
    emit!(LootboxNftWon { asset, winner, faction_id });
} else {
    // No drop this time — no state change to entry; entry stays in pool
    emit!(LootboxRollMissed { winner: winner_wallet.key(), roll: roll_value, threshold: drop_chance_bps });
}

player_data.pending_lootbox_roll = None;  // single-use, win or lose
```

**Drop chance formula** (basis points):
```
base = 1500;                                            // 15%
di_adjust = match metrics.demand_index {
    di if di < -60 => +1500,                            // crisis: drop more (don't list)
    di if di < -20 =>  +500,
    di if di <  20 =>     0,
    di if di <  60 =>  -500,
    _              => -1000,
};
pool_pressure = ((pool.lootbox_count as i32 - 50).max(0) * 50).min(2000);
quality_factor = ((entry.quality_score as i32 - 5_000) / 5).clamp(-300, +300);

drop_chance_bps = (base + di_adjust + pool_pressure + quality_factor)
                  .clamp(500, 8500);                    // 5%..85%
```
All inputs are read on-chain; no off-chain trust.

#### 3.2.6 `handle_inventory_proceeds`

When a program listing sells, marketplace sends `to_seller` lamports to `inventory_pda` (the seller). This ix splits accumulated proceeds 50/50 between sweep reserve and protocol pipeline, callable by anyone (idempotent — keeper-friendly).

**Important**: SOL lands in `inventory_pda`. We need to keep enough lamports for rent exemption of the InventoryPool account. Compute available the same way `distribute_sol_fees_internal` does.

```rust
pub fn handle_inventory_proceeds(ctx: Context<HandleInventoryProceeds>) -> Result<()> {
    let pool_info = ctx.accounts.inventory_pda.to_account_info();
    let rent_exempt = Rent::get()?.minimum_balance(pool_info.data_len());
    let available = pool_info.lamports().saturating_sub(rent_exempt);
    if available == 0 { return Ok(()); }

    let to_sweep = available / 2;
    let to_protocol = available - to_sweep;

    // 50% -> inventory_sweep_vault
    invoke_signed_transfer(inventory_pda -> inventory_sweep_vault, to_sweep, pool_seeds);

    // 50% -> sol_treasury (then routed by economy.rs::distribute_sol_fees_internal as usual)
    invoke_signed_transfer(inventory_pda -> sol_treasury, to_protocol, pool_seeds);

    emit!(InventoryProceedsRouted { to_sweep, to_protocol });
    Ok(())
}
```

Accounts:
| Account | Mut | Signer | Notes |
|---|---|---|---|
| `crank_authority` | ✓ | ✓ | |
| `inventory_pda` | ✓ | | source |
| `inventory_sweep_vault` | ✓ | | dest 50% |
| `sol_treasury` | ✓ | | dest 50%; existing PDA |
| `system_program` | | | |

### 3.3 New SOL vault: `inventory_sweep_vault`

PDA: `[b"inventory-sweep-vault"]`, system-owned, no data. Holds SOL reserved for sweep buys.

**Why a new vault**, not reusing `nft_floor_sweep_vault`: existing one is a **degenBTC token account**, wrong asset class. Reusing would force a swap each sweep — clunky.

**Migration**: existing degenBTC sitting in `nft_floor_sweep_vault` from past tax flows is **drained one-time via a Raydium swap to SOL**, deposited into `inventory_sweep_vault`. Implemented as `migrate_floor_sweep_to_sol` admin ix (one-shot, callable once). After migration, `tax.rs` is updated to route the floor-sweep tax bps directly to `inventory_sweep_vault` via swap-on-distribute. (Or simplest: reroute the bps to `nft_sale_sol_vault` / similar SOL accumulator, swap is a separate cranker ix.)

For the v1 implementation, the simplest clean path is:
- `nft_floor_sweep_vault` (degenBTC) is **deprecated and emptied** by admin migration.
- `tax.rs` floor-sweep bps is repointed to mint extra burn or to a fresh degenBTC accumulator that gets periodically swapped to SOL by a separate cranker (out of v1 scope — simply zero the bps for now and revisit).

**v1 simplification**: zero out the `nft_floor_sweep_vault` allocation in `TaxConfig` for now. Sweep reserve fills purely from listing proceeds. We can re-introduce tax-funded sweeps in a later phase.

### 3.4 Modified: `tax.rs`

- Mark `nft_floor_sweep_vault` as deprecated in comments; add admin `drain_legacy_nft_floor_sweep_vault` to swap-and-route remaining balance.
- Set the floor-sweep allocation `fee_split.nft_floor_sweep_pct = 0` at config update time (don't drop the field — just zero it).
- Existing `nft_sale_sol_vault` keeps working as-is for primary sale revenue.

### 3.5 Modified: `doges.rs`

- `int_send_to_heaven` is **renamed** to `int_recycle_doge` and rewritten per §3.2.3. **Burn CPI removed.**
- `SendToHeaven` accounts struct → `RecycleDoge`.
- `lib.rs` ix entrypoint renamed from `send_to_heaven` to `recycle_doge`.
- `events.rs`: `DogeSentToHeaven` event renamed to `DogeRecycled` (drop `accumulated_val`-only payload, add `quality_score`).
- **XP reset on recycle**: spec previously said XP stays. Reversed — XP also resets to 0 to prevent compounding (otherwise a recycled Doge entering inventory carries free-XP forward). DNA, breed_count, mom/dad, faction_id stay (preserve identity).

### 3.6 Modified: `state.rs`

New constants:
```rust
// Inventory
pub const MAX_INVENTORY: u32 = 200;
pub const MAX_LISTED: u32 = 25;
pub const MIN_LOOTBOX_POOL: u32 = 10;

// Lootbox
pub const LOOTBOX_COOLDOWN_SECONDS: i64 = 24 * 60 * 60 * 3;  // 3 days
pub const MAX_DOGES_PER_WALLET_FOR_DROP: u8 = 5;
pub const MAX_XP_FOR_QUALITY: u32 = 100_000;

// Demand Index thresholds
pub const DI_HOT: i16 = 60;
pub const DI_FIRM: i16 = 20;
pub const DI_NEUTRAL: i16 = -20;
pub const DI_SOFT: i16 = -60;

// Marketplace seeds (mineBTC side)
pub const INVENTORY_POOL_SEED: &[u8] = b"inventory-pool";
pub const RECYCLED_ENTRY_SEED: &[u8] = b"recycled-entry";
pub const MARKET_METRICS_SEED: &[u8] = b"market-metrics";
pub const INVENTORY_SWEEP_VAULT_SEED: &[u8] = b"inventory-sweep-vault";
```

Add to `PlayerData`: `pub pending_lootbox_roll: Option<LootboxRollClaim>` and `pub last_lootbox_drop_at: i64`. Bump `PlayerData::LEN`.

### 3.7 Modified: `events.rs`

Add:
```rust
#[event] pub struct DogeRecycled { pub asset: Pubkey, pub former_owner: Pubkey, pub accumulated_val: u64, pub quality_score: u16, pub timestamp: i64 }
#[event] pub struct InventoryListed { pub asset: Pubkey, pub price_lamports: u64, pub source_status: u8 }
#[event] pub struct InventoryCancelled { pub asset: Pubkey }
#[event] pub struct InventoryPriceUpdated { pub asset: Pubkey, pub price_lamports: u64 }
#[event] pub struct InventorySwept { pub asset: Pubkey, pub price_lamports: u64, pub seller: Pubkey }
#[event] pub struct LootboxNftWon { pub asset: Pubkey, pub winner: Pubkey, pub faction_id: u8, pub timestamp: i64 }
#[event] pub struct LootboxRollMissed { pub winner: Pubkey, pub roll: u16, pub threshold: u16 }
#[event] pub struct InventoryProceedsRouted { pub to_sweep: u64, pub to_protocol: u64 }
#[event] pub struct MarketMetricsUpdated { pub demand_index: i16, pub floor_price_lamports: u64 }
```

Remove `DogeSentToHeaven`.

---

## 4. SOL Flow Diagram

```
                     ┌────────────────── degenbtc_market ──────────────────┐
                     │                                                      │
P2P listing sells:   │  buyer.SOL --> 3% --> mineBTC.fee_recipient         │
                     │  buyer.SOL --> 97% -> seller_user.SOL                │
                     │                                                      │
Inventory listing    │  buyer.SOL --> 3% --> mineBTC.fee_recipient         │
sells:               │  buyer.SOL --> 97% -> mineBTC.inventory_pda         │
                     └──────────────────────────────────────────────────────┘
                                                │
                                                ▼ handle_inventory_proceeds
                     ┌────────────────── mineBTC.inventory_pda ────────────┐
                     │   50% --> inventory_sweep_vault (future sweeps)     │
                     │   50% --> sol_treasury                              │
                     │                                                      │
                     │              sol_treasury (existing flow)            │
                     │   distribute_sol_fees_internal                      │
                     │     --> buybacks_sol_vault (buyback_pct)            │
                     │     --> dev/multisig WSOL (1 - buyback_pct)         │
                     └──────────────────────────────────────────────────────┘

Inventory sweeps:    inventory_sweep_vault --> inventory_pda --> system::transfer
                     to seller via degenbtc_market::buy_listing (fee carved by mkt)
                     NFT --> inventory_pda
```

---

## 5. Off-Chain Crankers

All run as Node.js services subscribing to indexer + on-chain accounts.

### 5.1 DI Cranker (every 30 min)
Reads listings, recent sales, mint events; computes DI; calls `update_market_metrics`. Component formula (each in [-1, +1] before weights):

| Component | Source | Weight |
|---|---|---|
| Floor trend (24h slope of cheapest active listing price) | indexer | 0.25 |
| Avg time-to-fill (TTF) of last 50 sales | indexer | 0.30 |
| Sell-through rate (filled vs cancelled, last 7d) | indexer | 0.15 |
| Mint velocity Δ (current 24h vs prior 24h) | DogeMintConfig events | 0.15 |
| Gameplay-doge lock ratio (locked / circulating) | PlayerData scan | 0.10 |
| SOL bet volume Δ | game session events | 0.05 |

Final: weighted sum × 100, clamped to `[-100, 100]`.

### 5.2 Disposition Cranker (every 6h or on `DogeRecycled`)
For each `RecycledEntry` with `status == Pending`:

| DI band | Action |
|---|---|
| > +60 | `inventory_list_nft` at `floor × (1.10 + di_premium + quality_premium)` |
| +20..+60 | `inventory_list_nft` at `floor × (1.00 + quality_premium)` |
| -20..+20 | random 50/50: `list_nft` at `floor` OR mark `Lootbox` |
| -60..-20 | mark `Lootbox` |
| < -60 | mark `Lootbox` (crisis) |

Marking lootbox is just an on-chain status flip; add `inventory_set_lootbox(entry)` cranker ix (status: Pending → Lootbox, increments `lootbox_count`, decrements `pending_count`).

Auto-relist: any listing older than 24h with no fill, decrement price by 5% via `inventory_update_price` until it hits floor or sells. Cap at 6 decrements (then cancel and shift to lootbox if DI didn't recover).

### 5.3 Sweep Cranker (every 1-6h)
- If `DI > +60`: skip (hot market — let listings run).
- Compute `max_sweep_price = floor × sweep_max_pct(DI)` (e.g., 0.80x at DI=-100, 1.00x at DI=0).
- Compute `budget = sweep_vault_balance × sweep_aggressiveness(DI)` (e.g., 0% at DI=+60, 60% at DI=-60).
- Walk active P2P listings sorted ascending by price. For each ≤ `max_sweep_price`, call `inventory_buy_listing` until budget exhausted.
- Skip listings where seller is `inventory_pda` (don't sweep our own).

### 5.4 Lootbox Drop Cranker (run on each cycle settle, then drain queue)
Iterate `PlayerData` accounts with `pending_lootbox_roll != None`. For each:
1. Pick a candidate `RecycledEntry` (status Lootbox, same faction).
2. Call `process_lootbox_drops` with that pair.
3. Win or miss, the `pending_lootbox_roll` clears.

### 5.5 Proceeds Sweeper (every 1h)
Calls `handle_inventory_proceeds` if `inventory_pda` has > rent_exempt + 0.05 SOL. Idempotent.

---

## 6. Phased Implementation

| Phase | What ships | Calendar | Dev-days |
|---|---|---|---|
| **3.1** | mineBTC: `InventoryPool` + `RecycledEntry` + `MarketMetrics` accounts. `recycle_doge` replaces `send_to_heaven`. Tests. *No marketplace yet — recycled NFTs accumulate in inventory.* | wk 1 | ~5 |
| **3.2** | mineBTC: `process_lootbox_drops` + claim-side eligibility flag on `PlayerData` + drop chance formula + `last_lootbox_drop_at`. **Lootbox feature is live with no marketplace.** | wks 2-3 | ~7 |
| **3.3** | New program `degenbtc_market`: `init`, `update_config`, `list_nft`, `cancel_listing`, `update_listing_price`, `buy_listing`. Tests. **P2P trading live, no inventory hookup.** | wks 3-5 | ~14 |
| **3.4** | mineBTC `marketplace_cpi.rs`: `inventory_list_nft`, `inventory_cancel_listing`, `inventory_update_price`, `inventory_buy_listing`, `inventory_set_lootbox`, `handle_inventory_proceeds`, `update_market_metrics`. **Algorithm endpoints live.** | wk 6 | ~7 |
| **3.5** | Off-chain: DI cranker, disposition cranker, sweep cranker, drop cranker, proceeds sweeper. Indexer for marketplace events. | wks 7-8 | ~12 |
| **3.6** | Frontend: marketplace browse/list/buy, "Doge Reborn" tab, admin panel + DI dashboard. | wks 7-9 (parallel) | ~10 |
| **3.7** | External audit (mandatory — marketplace + lootbox both attractive attack surfaces). | wks 10-13 | calendar; $20k–$40k |
| **3.8** | Tune DI weights/thresholds against real volumes; iterate. | post-audit | ongoing |

**Total dev**: ~55 days ≈ 11 weeks solo, ~6 weeks with two devs in parallel (one on contracts, one on FE+off-chain). Plus 2-4 calendar weeks for audit.

---

## 7. Constants Reference

```rust
// degenbtc_market
pub const MARKETPLACE_FEE_BPS: u16 = 300;                  // 3.00%
pub const MAX_FEE_BPS: u16 = 1000;                         // hard cap 10%
pub const MIN_LISTING_PRICE_LAMPORTS: u64 = 10_000_000;    // 0.01 SOL

// mineBTC inventory
pub const MAX_INVENTORY: u32 = 200;
pub const MAX_LISTED: u32 = 25;
pub const MIN_LOOTBOX_POOL: u32 = 10;
pub const MAX_XP_FOR_QUALITY: u32 = 100_000;

// Lootbox
pub const LOOTBOX_COOLDOWN_SECONDS: i64 = 259_200;         // 3 days
pub const MAX_DOGES_PER_WALLET_FOR_DROP: u8 = 5;
pub const LOOTBOX_BASE_DROP_BPS: u16 = 1500;               // 15% before adjustments
pub const LOOTBOX_MIN_BPS: u16 = 500;                      // 5% floor
pub const LOOTBOX_MAX_BPS: u16 = 8500;                     // 85% ceiling

// Demand Index bands
pub const DI_HOT: i16 = 60;
pub const DI_FIRM: i16 = 20;
pub const DI_NEUTRAL: i16 = -20;
pub const DI_SOFT: i16 = -60;
```

---

## 8. Locked Decisions

| Question | Decision | Rationale |
|---|---|---|
| Marketplace fee | 3% flat, no royalty | Simple. Royalty enforcement happens via the fee since *we* control the marketplace. |
| Royalty plugin on Doges | **Not added** at mint | Avoids transfer interception by mpl-core plugin; fee captures the same value. |
| P2P sale of accumulator | Buyer inherits `accumulated_val`, `multiplier`, `xp` | Per-mint state follows the asset; documented as a feature, not a bug. |
| Inventory listing rent | Refunded to `inventory_pda` on close | Captured later by `handle_inventory_proceeds`. |
| Lootbox drop selection | Cranker picks candidate, on-chain VRF gates yes/no | Cranker can't bias the win/lose roll; only candidate selection (which is bounded by faction + lootbox-status filter). |
| Drop XP/multiplier | Reset to 0 / 1.0× on entry to inventory | Prevents free progression handouts to recipients. |
| DNA / faction / breed_count on recycle | Preserved | Identity continuity. |
| Inventory custody | Single `inventory_pda` (= `InventoryPool` account, dual-purpose) | One PDA, one set of seeds, less surface area. |
| Sweep funding source | New `inventory_sweep_vault` (system account, SOL) | Existing `nft_floor_sweep_vault` is degenBTC, wrong asset class. |
| Existing `nft_floor_sweep_vault` balance | Drained via one-shot admin migration (swap to SOL → `inventory_sweep_vault`); `tax_config.nft_floor_sweep_pct` zeroed | Pre-launch, no users to honor. |
| Update listing price | Included | Trivial UX win, ~10 lines. |
| Listing expiry | Excluded | Cranker decay logic covers stale listings off-chain via auto-relist. |
| Bids / accept-bid | Excluded | Sweep walks listing book directly. No orderbook. |
| Authority on `update_market_metrics` | mineBTC `crank_authority` (separate from admin) | Crank can be rotated without admin handover. |

---

## 9. Test Plan (gating on each phase)

### `degenbtc_market` unit tests
- Init + config update: only admin can update; `fee_bps > 1000` rejected.
- List: rejects non-collection asset, rejects price below min, rejects when disabled, rejects with royalty plugin attached.
- Cancel: only seller, asset returned to seller, listing closed, rent refunded.
- Update price: only seller, rejects below min.
- Buy: SOL math correct (3% fee), asset transferred, listing closed, rent to seller, fee to fee_recipient.
- Buy: fails if buyer underfunded.
- Buy: re-listing closed listing succeeds (PDA reuse via close).

### mineBTC integration tests
- `recycle_doge`: user gets accumulated_val, NFT goes to inventory_pda, RecycledEntry created with correct quality, metadata reset (multiplier=BASE, xp=0, accum=0), pool counters bump.
- `inventory_list_nft`: only crank_authority, status transitions Pending→Listed, listed_count++.
- Full cycle: recycle → list → buy (by user) → handle_inventory_proceeds → 50/50 split lands.
- `inventory_buy_listing` (sweep): pre-funds inventory_pda, buys, RecycledEntry with origin=1, total_swept++.
- `process_lootbox_drops`: win path resets metadata + transfers asset; miss path leaves entry in pool, clears `pending_lootbox_roll`.
- Pending roll accumulates only on correct-direction claims with active gameplay doge.
- Eligibility: cooldown enforced, MAX_DOGES_PER_WALLET enforced.
- Faction match enforced for drops.
- `handle_inventory_proceeds`: rent-exempt protection, exact 50/50 with rounding to sweep on odd lamport.

### Adversarial / edge cases
- Cranker passes a `RecycledEntry` with wrong faction → reject.
- Cranker passes a `recycled_entry` for an asset not actually owned by `inventory_pda` → reject (asset.owner check).
- User tries to list a Doge currently locked for gameplay → owner check fails (asset owner is `doge_custody`, not user).
- User lists, decreases price below `min_price` → reject.
- Buy a listing whose seller is no longer the recorded seller (shouldn't be possible — escrow holds it) → impossible by construction; assert via test.
- Buyer = seller (self-buy) → not blocked, but emit event normally (no exploit since fee is paid).
- Concurrent cancel + buy → serialized by Solana; whichever lands first wins, the other fails on closed listing.
- Inventory hits `MAX_INVENTORY` → recycle reverts with explicit error (don't silently drop the user's `accumulated_val` payout — verify ordering).

---

## 10. Open Items (to nail before code)

1. **Audit firm and slot.** Marketplace + lootbox attract auditors; book early. Suggest 2-3 firms to compare.
2. **Indexer infra.** Helius webhooks vs custom geyser? Indexer is critical for DI inputs.
3. **Frontend marketplace UX patterns.** "List my Doge" flow needs a confirm-and-sign with clear price/fee preview.
4. **Drop cooldown calibration.** 3 days is a guess. Tune post-launch to target ~1 drop per 50-100 active player-cycles.
5. **Migration timing of `nft_floor_sweep_vault` drain.** Pre-launch is fine to drop balance entirely (it's test-only) — confirm with team.
6. **Whether to ship 3.2 (lootbox) before 3.3 (marketplace).** Recommended yes — fastest engagement win, isolated risk.

---

## 11. Out of Scope (v1)

- Bids / offers / sweep-via-bids
- Listing expiry (off-chain auto-relist replaces this)
- User-set royalties on resale
- Multi-collection support (`MarketplaceConfig` is per-collection — fine, just one for now)
- Bulk list / bulk buy
- Tensor / ME compatibility (deliberately decoupled — could be added later if liquidity demands)
- On-chain DI computation (off-chain cranker pushes; cheaper)
