# NFT Marketplace + MineBTC Market Maker Spec

This document describes the current contract implementation. It intentionally
matches the permissionless floor-queue design in `programs/mineBTC/src`.

## Principles

- HashBeast trading happens through the standalone `degenbtc_market` program.
- MineBTC wraps user marketplace actions only when it needs protocol accounting.
- Inventory maintenance is permissionless. There is no privileged cranker key.
- Program-owned HashBeasts have exactly two persistent states: lootbox inventory or
  listed inventory. There is no stranded pending state.
- Floor anchors are built from on-chain sales and queue data, not off-chain
  demand-index writes.

## Marketplace Program

### Accounts

`MarketplaceConfig` is a PDA at `[b"marketplace-config", collection_mint]`.
It stores the admin, enabled flag, collection mint, fee bps, fee recipient,
minimum listing price, and MPL Core program id.

`Listing` is a PDA at `[b"listing", marketplace_config, asset]`.
The account exists only while a listing is live. Listing cancel and buy close it.

### Escrow

The marketplace does not create a separate escrow account with data. When a
listing is created, the MPL Core asset owner is set to the deterministic escrow
PDA `[b"escrow", marketplace_config, asset]`. Cancel and buy sign as that PDA.

### Instructions

- `initialize_marketplace`
- `update_marketplace_config`
- `list_nft`
- `cancel_listing`
- `update_listing_price`
- `buy_listing`

The marketplace verifies collection membership, minimum price, fee recipient,
seller ownership, and MPL Core program id. MineBTC must still bind all wrapper
calls to the cached marketplace config so unrelated collections cannot affect
the HashBeast floor queue.

## MineBTC State

### `InventoryPool`

PDA: `[b"inventory-pool"]`.

Fields:

- `bump`
- `marketplace_program`
- `marketplace_config`
- `total_count`

The PDA also acts as the inventory custody address for MPL Core assets.
`total_count` tracks live program inventory across lootbox and listed states.

### `RebornEntry`

PDA: `[b"reborn-entry", asset]`.

Fields:

- `asset`
- `faction_id`
- `quality_score`
- `reborn_at`
- `status`
- `listing_price`
- `origin`
- `original_buy_price`
- `expire_count`

`status` values:

- `Lootbox = 0`
- `Listed = 1`

Closed accounts are terminal sold, dropped, or burned states.

### Floor Data

`FloorQueue` is a sorted 20-entry queue of the cheapest user listings. Program
inventory listings are excluded.

`SaleHistory` is a 32-entry ringbuffer of qualifying user-to-user sales.

`FloorHistory` is a 7-entry daily anchor ringbuffer. Snapshots use the median of
recent qualifying sales when at least three samples exist; otherwise they fall
back to the median of the live floor queue. Zero-anchor snapshots are rejected.

### Lootbox Data

Each country has a `LootboxQueue` PDA with five packed asset slots.

Each winner has a `LootboxClaim` PDA at `[b"lootbox-claim", user]`. Reward
claiming can reserve an asset, and `claim_lootbox_nft` can be called later by
the user or by any bot. The NFT always goes to the recorded user.

### HashBeast Rebirth Data

`HashBeastMetadata` stores `rebirth_count`, capped at `MAX_REBIRTH_COUNT = 7`.
The same value is also encoded into DNA as a 3-bit rebirth generation at bit
offset 174. Faction remains in the low four DNA bits; the rebirth count is not
stored in the first bits because those already identify the country/faction.

Only explicit user `rebirth_hashbeast` calls increment this count and reroll DNA.
Market-maker sweeps or expiry cascades that put an NFT into a lootbox do not
reset DNA, multiplier, XP, breed state, or rebirth count.

Breeding also treats `rebirth_count` as a real generation/species lane: both
parents must have the same country and the same rebirth count, and offspring
inherits that count in both `HashBeastMetadata.rebirth_count` and DNA bits at offset
174.

## MineBTC Instructions

### User-Callable Marketplace Wrappers

- `list_user_nft(price)`
- `cancel_user_listing()`
- `update_user_listing_price(new_price)`
- `buy_user_listing()`

All wrappers check the cached marketplace program and config. Listing, update,
cancel, and buy stay in sync with `FloorQueue` when the listing is competitive
enough to be tracked.

### Permissionless Market Maker

`register_floor_listing`

Registers an existing marketplace listing into `FloorQueue`. The instruction
binds the listing to the cached marketplace config, verifies the listing PDA,
requires the listed asset to match a real HashBeast metadata PDA, and rejects
program-owned listings.

`record_floor_snapshot`

Records one daily floor anchor. It requires at least one usable sample and an
anchor at or above the sweep minimum before paying a keeper reward.

`sweep_floor_lowest`

Acts on the cheapest queue entry. If the head is stale, the instruction removes
that one stale entry and exits successfully. If the head is live, the protocol
buys it only when:

- the latest anchor is nonzero and above the sweep minimum,
- price is no more than `anchor * 1.05`,
- the buy fits the per-transaction vault cap,
- the sweep vault keeps its minimum reserve after the buy and keeper reward.

After the buy, the asset is handled in a single transaction:

- country lootbox queue has space: create a lootbox `RebornEntry` and push it
  without resetting the NFT,
- deep bearish floor trend: burn it,
- otherwise: relist from inventory at formula price.

`expire_program_listing`

Cancels a program-owned listing after the 7-day grace period and runs the same
queue/relist/burn cascade. Each expiry increments `expire_count`. When
`expire_count >= MAX_EXPIRES`, the asset is burned.

`inventory_finalize_sale`

Closes a listed `RebornEntry` after verifying the exact recorded asset is no
longer owned by `inventory_pda`.

`handle_inventory_proceeds`

Routes SOL sitting on the inventory PDA: 50% to `inventory_sweep_vault`, 50% to
the SOL treasury, preserving rent exemption.

`claim_lootbox_nft`

Permissionless delivery for a reserved `LootboxClaim`. The cranker receives the
closed account rent, while the NFT recipient is fixed by the claim account.
Delivery does not mutate HashBeast metadata.

`rebirth_hashbeast`

User-owned HashBeasts with accumulated degenBTC can be reborn into the lootbox
inventory. The instruction first pays the user's locked `accumulated_val`, then:

- if `rebirth_count >= MAX_REBIRTH_COUNT`, burns the asset,
- if the country lootbox queue or inventory is full, burns the asset,
- otherwise increments `rebirth_count`, rerolls fresh DNA with that count encoded,
  resets multiplier, XP, breed count, cooldown, accumulated value, gameplay lock,
  and parent lineage, transfers the asset into inventory, and queues it for a
  future lootbox winner.

## Pricing And Guardrails

Current constants:

- `RELIST_BASE_MARKUP_BPS = 1500`
- `RELIST_TREND_DIVIDER = 2`
- trend modifier clamped to `[-1000, +3000]`
- `RELIST_EXPIRE_PENALTY_BPS = 500`
- total markup clamped to `[-2000, +6000]`
- `BURN_TREND_BPS_THRESHOLD = -3000`
- `MAX_EXPIRES = 3`
- `EXPIRE_GRACE_SECS = 7 days`
- `SWEEP_ATTRACTIVE_BPS = 500`
- `SWEEP_MAX_PCT_BPS = 500`
- `SWEEP_MIN_ANCHOR_LAMPORTS = 0.01 SOL`
- `MIN_SWEEP_RESERVE_LAMPORTS = 0.05 SOL`
- `KEEPER_REWARD_LAMPORTS = 0.0005 SOL`

Keeper payments always preserve `MIN_SWEEP_RESERVE_LAMPORTS`.

## SOL Flow

User listing purchase:

```text
buyer SOL -> seller
buyer SOL -> marketplace fee recipient
escrow PDA -> buyer receives NFT
```

Inventory sweep:

```text
inventory_sweep_vault -> inventory_pda -> seller + marketplace fee recipient
escrow PDA -> inventory_pda receives NFT
inventory_pda -> lootbox queue, relist, or burn
```

Inventory listing sale:

```text
buyer SOL -> inventory_pda
buyer SOL -> marketplace fee recipient
inventory_pda proceeds -> inventory_sweep_vault + sol_treasury
```

HashBeast breeding:

```text
requires HashBeastMintConfig.genesis_mints >= genesis_mint_limit

total breed price = max(breeding curve, current floor anchor * 1.5)

SOL half:
  user -> fee_recipient (25% of SOL leg)
  user -> sol_treasury  (75% of SOL leg)

dbTC half:
  user token account -> burn             (50% of dbTC leg)
  user token account -> minebtc vault    (50% of dbTC leg)
```

## HashBeast Supply

`HashBeastConfig` stores non-sale HashBeast state: collection, total minted count, and
breeding config. `HashBeastMintConfig` stores genesis-sale-only limits: genesis
mint cap, per-country cap, sale curve, and ticket tiers.

There is no lifetime supply cap field in `HashBeastConfig`. Genesis mints are capped
by `HashBeastMintConfig`; post-genesis breeding uses the breeding bonding curve and
breed-count limits. Breeding has no separate birth throttle; the hard economic
guards are that the genesis sale must be fully sold out, the total breed price
must be at least 1.5x the current floor anchor, and half of that value is paid
in dbTC.

## Test Plan

Marketplace:

- list rejects non-collection assets and prices below minimum,
- cancel only works for seller and returns the asset,
- update price only works for seller and respects minimum price,
- buy transfers SOL/fees correctly, transfers the asset, and closes listing.

MineBTC market maker:

- register rejects listings not derived from cached marketplace config,
- register rejects non-HashBeast/non-metadata assets,
- user list/cancel/update/buy keep `FloorQueue` sorted and synchronized,
- snapshot rejects empty/zero anchors and accepts sale/queue medians,
- sweep purges one stale head without rolling back,
- sweep buys only within anchor, reserve, and tx-cap limits,
- sweep disposition creates lootbox entry, relists, or burns correctly,
- expire relists with progressive discount and burns on the third expiry,
- finalize sale cannot close an entry using a different asset,
- keeper rewards never reduce the sweep vault below reserve,
- lootbox claim can be cranked by a bot but always transfers to the recorded user.
