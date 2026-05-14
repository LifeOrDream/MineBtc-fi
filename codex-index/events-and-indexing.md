# Events and Indexing

Canonical source: `programs/mineBTC/src/events.rs`.

The backend should index contract events and also reconcile PDA state where event-only history may be insufficient after restarts. Sockets should publish compact deltas for hot views and API endpoints should serve cached snapshots for page loads.

## User and Referral Events

- `PlayerInitialized` - user signup, country/faction, referral state.
- `PlayerRecruited` - referral relationship and same-faction context.
- `ReferralRewardsClaimed` - claimed SOL/degenBTC referral rewards.
- `SolFeesWithdrawn` - SOL fee withdrawal split.

## Round and Betting Events

- `RoundStarted` - new active round with timing.
- `BetsPlaced` - user positions and bet totals.
- `RoundEnded` - winner/entropy result.
- `RewardsDistributedForRound` - reward pool/index finalization.
- `RoundRewardsClaimed` - per-user round reward claim.
- `JackpotHit`, `JackpotNearMiss`, `JackpotRolledOver` - jackpot state changes.

## Autominer Events

- `AutominerInitialized`, `AutominerUpdated`, `AutominerStopped`, `AutominerReloaded`.
- Backend should maintain autominer status, remaining rounds, reserve balance, last execution, and claimability.

## HashBeast NFT Events

- `HashBeastMinted` - mint, owner, metadata account, URI, DNA, multiplier, faction, price, ticket tier/count.
- `HashBeastFreeMintAllowanceUpdated`.
- `HashBeastCollectionCreated`, `CollectionDelegateAdded`, `CollectionInfoUpdated`.
- `HashBeastStaked`, `HashBeastUnstaked` - passive staking multiplier/hashpower updates.
- `HashBeastReborn`, `HashBeastRebirthBurned`.
- `HashBeastUsedForGameplay`, `HashBeastGameplayUnlockRequested`, `HashBeastWithdrawnFromGameplay`.
- Story events:
  - `StoryEventTriggered`
  - `HashBeastEvolution`
  - `HashBeastPowerMutation`
  - `HashBeastVisualMutation`
- Gameplay scoring:
  - `GameplayScoreAccumulated`

## Staking Events

- `MineBtcStaked`, `MineBtcUnstaked`.
- `LiquidityStaked`, `LiquidityUnstaked`.
- `SolRewardsClaimed`, `DbtcRewardsClaimed`.
- `MinebtcClaimableAccrued`.
- `HodlTaxRedistributed`, `PaperHandBurned`.
- `DegenBtcStakingRewardsDistributed`, `LpStakingRewardsDistributed`.

## Economy Events

- `PriceSnapshotTaken`
  - snapshot number, SOL swapped, degenBTC received, current price, weighted average, SOL earmarked for POL, total POL balance, history count, timestamp.
- `DistributionRateUpdated`
  - old/new emission rate, price change %, current/average/track/recent price, rate changed flag, new faction-war multiplier, timestamp.
- `FactionWarMultiplierUpdated`
  - multiplier bps movement.
- `LiquidityAdded`
  - SOL, degenBTC, LP minted, LP token price.
- `LpTokensBurned`
  - LP burned, cumulative LP burned, liquidity amounts, LP token price.
- `TaxDistributed` (now reports faction_treasury_amount + burn_amount only — no NFT floor sweep field).
- `FactionTreasuryRewardsClaimed`.
- `NftMarketMakingFunded` — `distribute_sol_fees` peeled off `nft_market_making_pct` into `inventory_sweep_vault`.
- `SolFeesWithdrawn` — top-line SOL distribute event.

## NFT Marketplace Events (`marketplace_cpi.rs`)

- `InventoryPoolInitialized` — one-time init telemetry.
- `FloorEntryRegistered` / `FloorEntryRemoved` — floor queue lifecycle (insertions, sweeps, cancels, price updates, stale pops).
- `FloorSweepExecuted` — `sweep_floor_lowest` succeeded; reports buy_price, seller, anchor, trend, stale_skipped, keeper.
- `InventoryAssetRelisted` — sweep or expire relisted at formula markup; reports original_buy_price, new_list_price, markup_bps, trend_bps, expire_count.
- `InventoryAssetBurned` — burn reason (0=trend crash, 1=max_expires, 2=recycle queue full).
- `UserSaleRecorded` — qualifying user-to-user sale entered `SaleHistory`.
- `FloorSnapshotRecorded` — daily snapshot wrote anchor + sample count. Source: 0=sale median, 1=queue fallback, 2=sale capped by queue, 3=first snapshot capped to marketplace min, 4=capped by prior anchor.
- `ProgramListingExpired` — 7-day TTL fired on a stuck program listing.
- `LootboxQueuePush` — recycle / sweep / expire-cascade pushed an asset into a country lootbox queue.
- `LootboxNftClaimed` — reserved drop delivered to the recorded winner.
- `InventoryProceedsRouted` — 50/50 split of accrued inventory_pda lamports between sweep_vault and sol_treasury.
- `InventorySaleFinalized` — `RebornEntry` cleanup after on-chain owner check confirmed sale.

## Faction War Events

- `FactionWarAutoStarted`, `FactionWarAutoSettled`.
- `FactionWarSettled` - final ranks, rewards, resolved directions.
- `FactionWarMvp`.
- `FactionWarRewardsClaimed`.

## Admin/Config Events

- `MiningTokenVaultSet`.
- `FactionAdded`.
- `EvolutionUnlockStageUpdated`.
- `GameplayTuningUpdated`.
- `GamePauseToggled`.

## Indexing Priorities

Hot-path tables/caches:

- Latest global config, game state, current round, current faction war, mining/economy state.
- Per-round session summary and faction-direction bet matrix.
- Per-user player state, current round bet, claimable rounds, claimable faction-war rewards.
- Per-faction aggregate state and cycle leaderboard.
- HashBeast inventory/metadata, staked HashBeasts, gameplay HashBeast.
- Staking positions and rewards.
- Referral leaderboard and user referral rewards.
- Investor/data-room aggregates: DAU/repeat/new users, round volume, SOL volume, HashBeast mints, referrals, retention cohorts, faction distribution, autominer usage.

Socket topics should be derived from these same cached snapshots to avoid duplicate API fetches on the frontend.
