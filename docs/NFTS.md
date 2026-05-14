<p align="center">
  <a href="./Readme.md">Overview</a> |
  <a href="./GAMEPLAY.md">Gameplay</a> |
  <a href="./ECONOMY.md">Economy</a> |
  <a href="./NFTS.md">HashBeasts</a>
</p>

# HashBeasts And NFT Market Logic

HashBeasts are MineBTC's dynamic NFT layer: Metaplex Core assets with game-state PDAs that can fight, mutate, breed, rebirth, boost staking, enter lootboxes, relist through inventory, or burn.

The key idea:

> **A HashBeast is not just art. It is a playable operator with a balance sheet and a lifecycle.**

## What A HashBeast Is

| Component | Role |
|---|---|
| Metaplex Core asset | Ownership, collection identity, royalties, transfer surface. |
| `HashBeastMetadata` PDA | Game state: DNA, XP, faction, breed count, rebirth count, multipliers, value, locks. |
| Active gameplay lock | Lets one HashBeast operate during round claims and mutations. |
| Passive staking custody | Lets up to 3 HashBeasts boost staking hashpower. |
| Inventory / marketplace state | Lets protocol-owned assets move through lootbox, relist, or burn paths. |

The Core asset is the source of truth for ownership. The metadata PDA is the source of truth for gameplay identity.

## Three Jobs

| Job | What the user feels | What the contract tracks |
|---|---|---|
| Gameplay operator | "This is my fighter." | `active_hashbeast`, `active_multiplier`, XP, DNA mutation, home-country score. |
| Passive staking booster | "These support my country yield." | Up to 3 staked HashBeasts contribute passive multiplier capped at 3x. |
| Economic asset | "This can mint, breed, cash out, recycle, sell, drop, or burn." | Breed count, rebirth count, accumulated value, inventory status, floor queue state. |

## Lifecycle

| Phase | Entry path | Exit path |
|---|---|---|
| Genesis | Public mint, whitelist mint, admin mint | User owns canonical HashBeast. |
| Active gameplay | User deploys as operator | User withdraws after unlock rules. |
| Passive staking | User stakes into custody | User unstakes after reward sync. |
| Breeding | Two eligible parents create offspring | Parents increment breed count; offspring starts new metadata. |
| Rebirth | Owner cashes out accumulated dBTC value | Asset resets and enters inventory, or burns at cap/guardrail. |
| Lootbox | Inventory asset reserved for winner | `claim_lootbox_nft` delivers to fixed user. |
| Program listing | Inventory relists at markup | Sale finalized or listing expires into next disposition. |
| Burn | Cap, bearish trend, or no viable inventory path | Asset exits supply. |

## Metadata

`HashBeastMetadata` tracks the game state that makes each NFT dynamic:

| Field group | Purpose |
|---|---|
| Identity | mint/asset, owner path, faction ID, collection binding. |
| DNA | Gene state for traits, evolution, power, and future species layers. |
| Progression | XP, active multiplier, passive multiplier, evolution stage. |
| Economy | accumulated dBTC value, breed count, rebirth count. |
| Locks | gameplay use, staking custody, incubated / lifecycle flags. |

Rebirth count is encoded into the gene logic. That leaves room for species shifts by generation:

| Rebirth generation | Art/product direction |
|---:|---|
| 0 | Genesis HashBeasts. |
| 1 | First recycled species layer. |
| 2-6 | Future species / faction / rarity experiments. |
| 7 | Final allowed rebirth generation. |
| >7 | Burn only. |

## Mint Paths

All mint paths must bind to the canonical HashBeast collection.

| Path | Use |
|---|---|
| `batch_mint_hashbeasts` | Public bonding-curve genesis mint. |
| `admin_mint_hashbeast` | Treasury, grants, admin allocations. |
| `whitelist_mint_hashbeast` | Pre-list free mints from allowance state. |
| `breed_hashbeasts` | Two parents create one offspring after genesis is sold out. |

Shared invariant:

```text
official Core collection
  -> create asset
  -> initialize metadata PDA
  -> assign faction/DNA
  -> increment total_hashbeasts_minted
```

Never mint outside the official collection. Marketplace gating, royalties, identity, and floor support all assume canonical collection membership.

## Genesis

Current setup:

| Parameter | Value |
|---|---:|
| Genesis cap | 36,000 |
| Per-faction genesis cap | 3,000 |
| Base mint price | 1 SOL |
| Pricing | Bonding curve |
| Royalties | 4.2% |

There is no lifetime supply cap. Instead, post-genesis supply is controlled by breeding rules, breed pricing, floor anchor, breed count, and rebirth/burn mechanics.

## Gameplay Mutations

Mutations happen during reward claims, not at bet placement.

| Mutation | Score weight | Product meaning |
|---|---:|---|
| Evolution | 4x | Major visual/progression jump. |
| Power | 2x | Strength or multiplier-oriented upgrade. |
| Trait | 1x | Trait/visual/secondary update. |

The roll can update:

| State | Effect |
|---|---|
| DNA | Trait/power/evolution bits change. |
| XP | Accumulates or is consumed by mutation. |
| Active multiplier | Grows toward the 4.2x cap. |
| Accumulated value | NFT can store dBTC value for later rebirth. |
| Faction-war score | Home-country mutations count toward HashBeast/MVP lanes. |

Mercenary rule:

| Mutation context | Leaderboard | User HB/MVP reward credit |
|---|---:|---:|
| Home-country win | Full score | Yes |
| Foreign-country win | 50% score | No |

This preserves the reward invariant while still letting foreign bets matter on the visible leaderboard.

## Breeding

Breeding is controlled supply expansion, not free inflation.

The economic lesson from earlier breedable NFT games is simple: breeding can be a great token sink, but cheap repeat breeding can crush the floor if new supply arrives faster than demand. MineBTC therefore makes breeding dual-resource, parent-limited, same-faction, same-rebirth, and floor-aware.

Rules:

| Rule | Why it exists |
|---|---|
| Genesis must be sold out first. | Avoids competing with primary supply. |
| Parents must be canonical HashBeasts. | Prevents fake collection assets. |
| Parents must be same faction. | Preserves country identity. |
| Parents must be same rebirth generation. | Keeps species/recycle levels coherent. |
| Each parent has max breed count. | Prevents infinite output from best parents. |
| Floor snapshot must be fresh. | Prevents stale oracle pricing. |
| Price must be at least 1.5x floor anchor. | Breeding cannot undercut the secondary floor. |

Payment split:

| Leg | Split |
|---|---|
| SOL half | 25% fee recipient, 75% SOL treasury. |
| dBTC half | 50% burn, 50% mining vault. |

This makes breeding simultaneously:

- a dBTC sink;
- a dBTC emission recycle;
- a SOL treasury source;
- a floor-protected supply valve.

## Rebirth

Rebirth is the recycling mechanic.

When a user rebirths:

| Step | What happens |
|---:|---|
| 1 | User cashes out the HashBeast's accumulated dBTC value. |
| 2 | Asset transfers into protocol inventory. |
| 3 | If cap/queue/inventory checks pass, metadata resets in place. |
| 4 | DNA is renewed, XP resets, gameplay state resets, multiplier state resets, breed state resets. |
| 5 | Rebirth count increments. |
| 6 | Asset becomes future lootbox / relist / burn inventory. |

Rebirth cap:

| Condition | Result |
|---|---|
| `rebirth_count < 7` and inventory path available | Reset and recycle. |
| `rebirth_count >= 7` | Burn. |
| Queue/inventory path cannot accept asset | Burn. |

Important distinction:

| Asset source | Reset? |
|---|---:|
| User explicitly rebirths and cashes out accumulated dBTC | Yes |
| Market maker buys listing into lootbox inventory | No |

Marketplace sweep buys do not magically wipe user history. Explicit rebirth does, because the owner extracted the stored value and the asset starts a new lifecycle.

## Lootboxes

Each country has a 10-slot `LootboxQueue`.

Inventory sources:

| Source | Enters queue? |
|---|---|
| Reborn HashBeasts | Yes, if queue has room. |
| Market-maker sweep buys | Yes, if queue has room. |
| Expired program inventory | Re-enters disposition cascade. |

Loser-roll drop chance:

| Depth | Chance | Depth | Chance |
|---:|---:|---:|---:|
| 0 | 0% | 6 | 0.58% |
| 1 | 0.03% | 7 | 0.78% |
| 2 | 0.08% | 8 | 1.00% |
| 3 | 0.15% | 9 | 1.25% |
| 4 | 0.25% | 10 | 1.50% |
| 5 | 0.40% |  |  |

The queue creates consolation excitement without creating a guaranteed drain at full inventory.

## Marketplace Architecture

HashBeast trading uses a standalone `degenbtc_market` program. MineBTC wraps the market when it needs game accounting, floor tracking, or inventory ownership.

| Surface | Who calls | What it does |
|---|---|---|
| `list_user_nft` | Asset owner | Lists through marketplace and inserts listing into floor queue. |
| `cancel_user_listing` | Asset owner | Cancels and removes from floor queue. |
| `update_user_listing_price` | Asset owner | Updates price and re-sorts queue. |
| `buy_user_listing` | Buyer | Buys listing, removes queue entry, records qualifying sale. |
| `register_floor_listing` | Anyone | Registers an existing live listing into the floor queue. |
| `sweep_floor_lowest` | Anyone | Buys the cheapest attractive listing into protocol inventory. |
| `record_floor_snapshot` | Anyone | Updates conservative floor anchor. |
| `expire_program_listing` | Anyone | Cancels stale inventory listing and re-disposes asset. |
| `handle_inventory_proceeds` | Anyone | Splits inventory sale lamports to sweep vault and treasury. |
| `inventory_finalize_sale` | Anyone | Closes sold inventory entry after owner/escrow checks. |
| `claim_lootbox_nft` | Anyone | Delivers reserved NFT to fixed user recipient. |

There is no privileged market-maker cranker. The caller can earn a bounty, but cannot redirect the NFT recipient.

## Floor Oracle

The floor anchor is intentionally conservative.

| State | Role |
|---|---|
| `FloorQueue` | Top 20 cheapest registered live user listings. |
| `SaleHistory` | 32-slot ringbuffer of qualifying user-to-user sales. |
| `FloorHistory` | 7-day rolling snapshot buffer used as the current floor anchor. |

Anti-manipulation rules:

| Guard | Why it matters |
|---|---|
| Sale must have at least 5 minutes listing age. | Makes wash-trade cycles slower and more expensive. |
| Sale median requires at least 17 qualifying samples. | Manipulator must dominate the ringbuffer before sales drive anchor. |
| Queue median caps high sale anchors. | Cheap sell-side supply prevents high wash trades from raising buy ceiling. |
| First anchor capped by marketplace minimum. | Day-zero cannot pump anchor with one high listing. |
| Existing anchor has upward move cap. | Floor cannot jump violently from sales alone. |
| Listing-only fallback can move anchor down, not pump up. | Cheap supply matters immediately; expensive listings do not fake demand. |

## Sweep Guardrails

Per-sweep limits:

| Guardrail | Value |
|---|---:|
| Max price | floor anchor x 1.05 |
| Max vault spend | 5% of sweep vault |
| Min floor anchor | 0.01 SOL |
| Min sweep vault reserve | 0.05 SOL |
| Inventory cap | 200 assets |

Keeper rewards:

| Action | Reward |
|---|---:|
| Real sweep / snapshot / expire | 0.0005 SOL |
| Stale queue purge only | 0.00002 SOL |

The stale-purge reward is intentionally tiny. It removes the list -> raw-cancel -> purge-reward farming attack while still compensating honest cleanup.

## Inventory Disposition

When the protocol owns an asset, the same cascade decides what happens next:

| Condition | Action |
|---|---|
| Country lootbox queue has room | Push into lootbox queue. |
| Market trend is deeply bearish | Burn. |
| Otherwise | Relist at markup. |
| Program listing expires repeatedly | Discount over time, then burn at max expires. |

Relist formula:

```text
markup = base_markup + trend_modifier - expiry_penalty
```

Current constants:

| Parameter | Value |
|---|---:|
| Base markup | +15% |
| Trend modifier range | -10% to +30% |
| Expiry penalty | -5% per strike |
| Total markup clamp | -20% to +60% |
| Expire grace | 7 days |
| Max expires | 3 |
| Burn trend threshold | -30% |

## Why This NFT System Matters

HashBeasts connect every major part of MineBTC:

| NFT action | System touched |
|---|---|
| Mutate | Gameplay, country score, MVP, NFT metadata. |
| Stake passively | Staking hashpower and country loyalty. |
| Breed | dBTC burn, mining vault recycle, SOL treasury, floor anchor. |
| Rebirth | Accumulated value, supply recycling, lootboxes, burn cap. |
| Sell | Marketplace, floor queue, sale history, floor anchor. |
| Sweep | Treasury-funded market maker, lootboxes, relists, burns. |

The collection is designed to feel alive because the contracts treat it as alive: every serious gameplay and economy loop can leave a mark on the NFT layer.
