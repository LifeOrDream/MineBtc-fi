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

## Doge NFT Events

- `DogeMinted` - mint, owner, metadata account, URI, DNA, multiplier, faction, price, ticket tier/count.
- `DogeFreeMintAllowanceUpdated`.
- `DogeCollectionCreated`, `CollectionDelegateAdded`, `CollectionInfoUpdated`.
- `DogeStaked`, `DogeUnstaked` - passive staking multiplier/hashpower updates.
- `DogeSentToHeaven`.
- `DogeUsedForGameplay`, `DogeGameplayUnlockRequested`, `DogeWithdrawnFromGameplay`.
- Story events:
  - `StoryEventTriggered`
  - `DogeEvolution`
  - `DogePowerMutation`
  - `DogeVisualMutation`
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
- `TaxDistributed`, `NftFloorSweepFundsWithdrawn`, `FactionTreasuryRewardsClaimed`.

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
- Doge inventory/metadata, staked Doges, gameplay Doge.
- Staking positions and rewards.
- Referral leaderboard and user referral rewards.
- Investor/data-room aggregates: DAU/repeat/new users, round volume, SOL volume, Doge mints, referrals, retention cohorts, faction distribution, autominer usage.

Socket topics should be derived from these same cached snapshots to avoid duplicate API fetches on the frontend.
