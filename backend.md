# MineBTC Backend Architecture & Readiness Assessment

> **Context:** This document maps the `MineBtcBackend` sister codebase, identifies what exists, what's working, and what must be updated to support the contract changes made in `MineBtc-fi` (jackpot rebrand, staking visibility, faction war MVP bonuses, and the pending mutation overhaul).

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Tech Stack](#2-tech-stack)
3. [Project Structure](#3-project-structure)
4. [Indexing & Event Pipeline](#4-indexing--event-pipeline)
5. [Database Schema (DynamoDB)](#5-database-schema-dynamodb)
6. [GraphQL API Surface](#6-graphql-api-surface)
7. [Orchestrator & Crankers](#7-orchestrator--crankers)
8. [NFT Minting Pipeline](#8-nft-minting-pipeline)
9. [Real-Time Layer](#9-real-time-layer)
10. [Critical Backend Updates Needed](#10-critical-backend-updates-needed)
11. [Priority Matrix](#11-priority-matrix)

---

## 1. Architecture Overview

The backend is a **hybrid monolith** — single TypeScript codebase that spawns multiple specialized Node.js processes. It is **NOT** microservices; processes share code and DynamoDB tables but have distinct runtime roles.

### Core Data Flow

```
┌─────────────────┐     ┌──────────────┐     ┌─────────────┐     ┌──────────────┐
│  Helius Webhook │────▶│  Express     │────▶│  BullMQ     │────▶│  Worker      │
│  (enhanced)     │     │  (port 3003) │     │  (Redis)    │     │  (decode +   │
└─────────────────┘     └──────────────┘     └─────────────┘     │   persist)   │
                                                                  └──────┬───────┘
                                                                         │
                              ┌──────────────────────────────────────────┘
                              ▼
┌─────────────────┐     ┌──────────────┐     ┌─────────────┐     ┌──────────────┐
│  Frontend       │◀────│  Socket.io   │◀────│  Redis Pub/ │◀────│  DynamoDB    │
│  (React/Web)    │     │  (port 8080) │     │    Sub      │     │  (Valkey)    │
└─────────────────┘     └──────────────┘     └─────────────┘     └──────────────┘
                              ▲
                              │
┌─────────────────┐     ┌──────────────┐
│  GraphQL API    │────▶│  AWS Lambda  │
│  (Apollo 4)     │     │  (Serverless)│
└─────────────────┘     └──────────────┘
```

### Key Runtime Modes

| Mode | Deployment | Purpose |
|------|-----------|---------|
| **Serverless** | AWS Lambda via Serverless Framework v4 | GraphQL API only |
| **Long-running** | EC2 + PM2 or custom orchestrator | Everything else (indexers, crankers, workers, sockets) |

### 11 Managed Processes

| Process | Script | Role |
|---------|--------|------|
| `webhook-server` | `startWebhookServerQueue.ts` | Receives Helius webhooks, enqueues events |
| `worker` | `startWorker.ts` | BullMQ consumer: decodes Anchor events, persists to DynamoDB |
| `socket-server` | `startSocketServerPubsub.ts` | Socket.io + raw WS, subscribes to Redis pub/sub |
| `game-loop` | `game_loop.ts` | Cranker: starts/ends/distributes rounds |
| `economy-cycle-loop` | `snapshots_loop.ts` | Unified cranker: snapshots, rate updates, LP burn, tax, claims |
| `autominer-worker` | `autominer_loop.ts` | Executes autominer bets & claims on behalf of users |
| `rebase-claim-loop` | `rebase_claim_loop.ts` | Permissionless auto-claimer for faction war rewards |
| `tax-loop` | `tax_loop.ts` | Legacy tax harvest + distribute (may overlap with economy cycle) |
| `price-fetcher` | `startPriceFetcher.ts` | Fetches SOL/degenBTC price data |
| `pool-state-updater` | `startPoolStateUpdater.ts` | Tracks Raydium CP-Swap pool state |
| `asset-worker` | `startAssetWorker.ts` | Generates NFT assets via FAL.ai + Google GenAI |
| `admin-server` | `startAdminServer.ts` | Internal dashboard API (health, metrics, queues) |

---

## 2. Tech Stack

| Layer | Technology |
|-------|-----------|
| **Runtime** | Node.js 22+ |
| **Language** | TypeScript (ES2018 target, CommonJS output) |
| **Web Framework** | Express.js (webhook/admin), Apollo Server 4 (GraphQL), Socket.io + raw WS (realtime) |
| **Blockchain** | Solana Web3.js + Anchor 0.32.1, Metaplex UMI + MPL Core |
| **Database** | AWS DynamoDB (via Dynamoose ODM) |
| **Cache / Pub-Sub** | Valkey (Redis-compatible) via ioredis — AWS ElastiCache in prod |
| **Queue** | BullMQ |
| **Queue UI** | Bull Board (Express dashboard on port 3005) |
| **Object Storage** | AWS S3 (NFT metadata + images) |
| **Process Mgmt** | PM2 (`ecosystem.config.js`) OR custom orchestrator (`orchestrator.ts`) |
| **Notifications** | Telegram Bot API |
| **AI** | FAL.ai (Nano Banana Pro), Google Gemini 2.0 Flash (validation) |
| **Deployment** | AWS Lambda (GraphQL) + EC2/PM2 (all other services) |

---

## 3. Project Structure

```
MineBtcBackend/
├── src/
│   ├── api/                    # HTTP entry points
│   │   ├── graphQl.ts          # Apollo Server + AWS Lambda handler
│   │   ├── webhook.queue.ts    # Helius webhook receiver → BullMQ
│   │   ├── nftTransferWebhook.ts
│   │   └── admin.ts            # Express router for admin dashboard
│   ├── config/
│   │   └── default.ts          # Central config: tables, RPC, Redis, sockets
│   ├── graphql/
│   │   ├── resolvers/          # 24 TypeGraphQL resolvers
│   │   │   ├── round.resolver.ts
│   │   │   ├── rebase.resolver.ts
│   │   │   ├── playerData.resolver.ts
│   │   │   ├── stakedPositions.resolver.ts
│   │   │   ├── economyDashboard.resolver.ts
│   │   │   ├── frontendData.resolver.ts
│   │   │   ├── btcDoge.resolver.ts
│   │   │   ├── assetGeneration.resolver.ts
│   │   │   └── ... (16 more)
│   │   └── utils/converters.ts
│   ├── model/                  # Dynamoose models (DynamoDB schemas)
│   │   ├── Round.model.ts
│   │   ├── PlayerData.model.ts
│   │   ├── PlayerRoundBet.model.ts
│   │   ├── StakedPosition.model.ts
│   │   ├── Rebase.model.ts
│   │   ├── UserRebaseReward.model.ts
│   │   ├── TaxDistribution.model.ts
│   │   ├── BtcDoge.model.ts
│   │   ├── Faction.model.ts
│   │   ├── AutominerVault.model.ts
│   │   ├── ChatMessage.model.ts
│   │   └── ... (10 more)
│   ├── queues/
│   │   ├── connection.ts       # ioredis/Valkey connection factory
│   │   ├── eventQueue.ts       # BullMQ queue for Solana events
│   │   ├── assetGenerationQueue.ts
│   │   ├── autominerQueue.ts
│   │   └── index.ts
│   ├── scripts/                # Executable entry points (one per process)
│   │   ├── startWebhookServerQueue.ts
│   │   ├── startWorker.ts
│   │   ├── startSocketServerPubsub.ts
│   │   ├── game_loop.ts
│   │   ├── snapshots_loop.ts
│   │   ├── tax_loop.ts
│   │   ├── autominer_loop.ts
│   │   ├── rebase_claim_loop.ts
│   │   ├── orchestrator.ts     # Master process manager
│   │   ├── startAdminServer.ts
│   │   ├── startPriceFetcher.ts
│   │   ├── startAssetWorker.ts
│   │   └── sim/                # Simulation/devnet test scripts
│   ├── services/               # Business logic
│   │   ├── worker.service.ts          # BullMQ worker setup
│   │   ├── socket.service.ts          # Socket.io server logic
│   │   ├── processEvents/             # Event handlers by domain
│   │   │   ├── index.ts               # Main event router (big switch)
│   │   │   ├── game.ts
│   │   │   ├── user.ts
│   │   │   ├── stake.ts
│   │   │   ├── doges.ts
│   │   │   ├── economy.ts
│   │   │   ├── tax.ts
│   │   │   ├── rebase.ts
│   │   │   └── admin.ts
│   │   ├── apiCache.service.ts
│   │   ├── liveFeed.service.ts
│   │   ├── dashboardBundle.service.ts
│   │   ├── autominerWorker.service.ts
│   │   ├── rebaseAutoClaim.service.ts
│   │   ├── economyV2.service.ts
│   │   ├── redisState.service.ts
│   │   ├── arenaDashboard.service.ts
│   │   ├── frontendDashboard.service.ts
│   │   ├── nftMetadata.service.ts
│   │   ├── assetGenerationWorker.service.ts
│   │   └── ... (many more)
│   ├── utils/
│   │   ├── deploymentConfig.ts      # Reads deployment.json per env
│   │   ├── dynamoose.ts             # DynamoDB connection setup
│   │   ├── eventDecoder.ts          # Parses Anchor events from tx logs
│   │   ├── socketEmitter.pubsub.ts  # Redis pub/sub wrapper
│   │   ├── logger.ts
│   │   └── solana.ts
│   └── prompts/                # AI image generation prompts by faction
├── idl/
│   ├── minebtc.json            # Anchor IDL (currently STALE)
│   └── raydium_cp_swap.json
├── deployment.json             # Contract addresses per env
├── serverless.yml              # AWS Lambda deployment (GraphQL only)
├── ecosystem.config.js         # PM2 process definitions
├── Dockerfile
├── package.json
└── .env / .env.example
```

---

## 4. Indexing & Event Pipeline

### 4.1 Dual-Mode Ingestion

**Primary: Helius Webhooks → BullMQ → Workers**
- `POST /webhook/solana` receives enhanced webhooks from Helius
- `handleHeliusWebhookWithQueue()` validates auth, parses Anchor events from tx logs, enqueues to BullMQ
- Workers (`startWorker.ts`) dequeue and process via `processEvent()`

**Fallback: RPC Polling**
- `startIndexer.ts` runs `ModuleIndexer` polling `getSignaturesForAddress` every **1 hour**
- Startup catch-up fetches all signatures since last checkpoint

**Checkpointing**
- `IndexerCheckpoint.model.ts` stores `lastProcessedSlot`, `lastProcessedSignature`, `lastProcessedBlockTime`
- Debounced saves every 5s

### 4.2 Event Decoder

`src/utils/eventDecoder.ts` uses Anchor's `Program.coder.events.decode()` with the local IDL (`idl/minebtc.json`).

### 4.3 Queue Architecture (BullMQ)

**Queue:** `solana-events`
- **Priority levels:**
  - CRITICAL (1): `roundStarted`, `roundEnded`
  - HIGH (2): `rewardsDistributedForRound`, `factionWarSettled`, `taxDistributed`
  - NORMAL (3): `betsPlaced`
  - LOW (4): everything else
- Retry: 3 attempts, exponential backoff (1s, 2s, 4s)
- Retention: 1000 completed (1h), 5000 failed (24h)

**Worker concurrency:** 5 default (tunable via `WORKER_CONCURRENCY`)
- Rate limiter: 100 jobs / second

### 4.4 Event Router

`src/services/processEvents/index.ts` — large `switch` statement routes to typed handlers:

| Handler File | Events Handled |
|-------------|----------------|
| `game.ts` | `RoundStarted`, `RoundEnded`, `MotherlodeHit`, `RewardsDistributedForRound`, `DegenBtcStakingRewardsDistributed`, `LpStakingRewardsDistributed` |
| `user.ts` | `PlayerInitialized`, `FactionChanged`, `BetsPlaced`, `RoundRewardsClaimed`, `Autominer*`, `ReferralRewardsClaimed`, `DogeUsedForGameplay`, `DogeWithdrawnFromGameplay`, `MutationTriggered`, `DogeEvolution`, `DogePowerMutation`, `DogeVisualMutation`, `DogeGameplayUnlockRequested` |
| `stake.ts` | `MineBtcStaked`, `MineBtcUnstaked`, `LiquidityStaked`, `LiquidityUnstaked`, `EmergencyWithdrawal`, `SolRewardsClaimed`, `DbtcRewardsClaimed`, `MinebtcClaimableAccrued`, `RefiningFeeRedistributed` |
| `doges.ts` | `DogeMinted`, `DogeStaked`, `DogeUnstaked`, `DogeSentToHeaven`, `DogeSynced` |
| `rebase.ts` | `FactionWarAutoStarted`, `FactionWarSettled`, `FactionWarAutoSettled`, `FactionWarRewardsClaimed`, `MutationScoreAccumulated` |
| `tax.ts` | `TaxDistributed`, `NftFloorSweepFundsWithdrawn`, `FactionTreasuryRewardsClaimed` |
| `economy.ts` | `SolFeesWithdrawn`, `PriceSnapshotTaken`, `LiquidityAdded`, `DistributionRateUpdated`, `LpTokensBurned` |
| `admin.ts` | `FactionAdded`, `CollectionDelegateAdded`, `CollectionInfoUpdated`, `MiningTokenVaultSet`, `DogeCollectionCreated`, `DogeFreeMintAllowanceUpdated` |

### 4.5 IDL Events (55 Total — Currently Stale)

The backend IDL (`idl/minebtc.json`) still uses **OLD event names** that do not match the refactored contract:

| Backend IDL (Current) | Contract (New) | Status |
|----------------------|----------------|--------|
| `MotherlodeHit` | `JackpotHit` | ❌ Stale |
| `EmergencyWithdrawal` | `PaperHandBurned` | ❌ Stale |
| `RefiningFeeRedistributed` | `HodlTaxRedistributed` | ❌ Stale |
| *(missing)* | `JackpotNearMiss` | ❌ Missing |
| *(missing)* | `JackpotRolledOver` | ❌ Missing |
| *(missing)* | `FactionWarMvp` | ❌ Missing |

**This means the backend will NOT index any jackpot events, paper hand burns, HODL tax, near-misses, rollovers, or MVP bonuses after the contract is redeployed.**

---

## 5. Database Schema (DynamoDB)

All tables use `ON_DEMAND` throughput. Table names are prefixed `mineBTC_` and shared across environments.

### 5.1 Core Game Tables

| Table | Hash Key | Range Key | GSIs | Purpose |
|-------|----------|-----------|------|---------|
| `mineBTC_rounds` | `round_id` (N) | — | `gsi-pk-round_id`, `gsi-status-round_id` | Round state & outcomes |
| `mineBTC_player_data` | `owner` (S) | — | — | User profile, staking aggregates, claimables |
| `mineBTC_player_round_bets` | `owner` (S) | `round_id` (N) | — | Per-user per-round betting history |
| `mineBTC_rebases` | `faction_war_id` (N) | — | `gsi-pk-faction_war_id`, `gsi-status-faction_war_id` | Faction war cycles |
| `mineBTC_user_rebase_rewards` | `owner` (S) | `faction_war_id` (N) | `gsi-faction_war_id-owner`, `gsi-status-faction_war_id` | Per-user rebase reward eligibility |
| `mineBTC_factions` | `faction_id` (N) | — | — | Faction staking stats, APR windows |

### 5.2 Staking Tables

| Table | Hash Key | Purpose |
|-------|----------|---------|
| `mineBTC_staked_positions` | `owner` (S) + `position_key` (S) | Individual staking positions (DBTC/LP) |
| `mineBTC_unrefined_rewards` | single row | Refining reward pool state |
| `mineBTC_emissions_vault` | single row | Emissions vault principal/realized |

### 5.3 NFT Tables

| Table | Hash Key | GSIs | Purpose |
|-------|----------|------|---------|
| `mineBTC_doges` | `mint` (S) | `gsi-owner` | NFT metadata, DNA, evolution, mutations, asset generation status |
| `mineBTC_doge_round_participation` | composite | — | Doge participation per round |

### 5.4 Economy & Tax Tables

| Table | Hash Key | Purpose |
|-------|----------|---------|
| `mineBTC_tax_distributions` | `transaction_hash` (S) | Tax burn/treasury/recycle events |
| `mineBTC_faction_treasury_claims` | composite | Faction treasury reward claims |
| `mineBTC_game_fee_aggregated` | time-bucketed | Fee/refining fee aggregation |
| `mineBTC_price_cycles` | time-series | Price cycle data |
| `mineBTC_lp_pool_state` | single row | Raydium LP pool snapshots |
| `mineBTC_onchain_accounts` | named keys | Cached on-chain account data |

### 5.5 System Tables

| Table | Hash Key | Purpose |
|-------|----------|---------|
| `mineBTC_indexer_checkpoints` | `checkpoint_key` (S) | Indexer resume state |
| `mineBTC_autominer_vaults` | per-user | Autominer state |
| `mineBTC_game_metrics` | date key | Aggregated analytics (DAU, MAU, etc.) |
| `mineBTC_chat_messages` | composite | In-game chat |
| `mineBTC_referral_rewards` | composite | Referral tracking |

### 5.6 Schema Issues

1. **`mineBTC_rounds` has NO GSI for `faction_war_id`** — `aggregateRebaseVolumeFromRounds()` falls back to full table scan. Won't scale.
2. **Live bet data lives only in Redis** — `BetsPlaced` updates Redis but NOT DynamoDB until `RoundEnded`. Redis loss = unrecoverable bet history.
3. **`PlayerData` has no `current_faction_war_score` field** — needed for MVP tracking.
4. **`Rebase` model has no MVP fields** — `faction_mvp_user`, `faction_mvp_score`, `faction_mvp_bonus` missing.
5. **No `jackpot_*` fields in `Round` or `Faction`** — still using `motherlode_*` naming everywhere.

---

## 6. GraphQL API Surface

### 6.1 Resolvers (24 total)

| Resolver | Key Queries | Key Mutations | Notes |
|----------|-------------|---------------|-------|
| `round.resolver.ts` | `round(id)`, `rounds(limit, cursor)`, `roundsByStatus`, `currentArenaState`, `playerRoundOutcomes`, `claimableRoundSummary` | — | Returns `RoundResponseType` with `motherlode_*` fields |
| `rebase.resolver.ts` | `rebase(id)`, `rebases(limit, cursor)`, `currentActiveRebase`, `userRebaseRewards(wallet)`, `rebaseFactionWarCycles` | — | Returns `RebaseResponseType`, `RebaseFactionWarCycleType` |
| `playerData.resolver.ts` | `playerData(wallet)`, `playerLeaderboard` | — | Player profile + claimables |
| `stakedPositions.resolver.ts` | `stakedPositions(wallet)`, `stakeBoard` | — | Staking dashboard |
| `frontendData.resolver.ts` | `frontendOverview`, `stakeFactionBoard`, `roundLedger`, `rebaseFactionWarCycles` | — | Bundled dashboard data |
| `economyDashboard.resolver.ts` | `currentEconomyState`, `currentTaxState`, `liveFeed`, `taxFlowWindow` | — | Economy metrics |
| `btcDoge.resolver.ts` | `doge(mint)`, `dogesByOwner`, `doges(limit, cursor, sort)`, `dogeLeaderboard` | — | NFT data |
| `assetGeneration.resolver.ts` | `assetGenerationStatus(mint)`, `assetQueueStats`, `assetQueueOverview` | `retryAssetGeneration(mint)` | NFT pipeline monitoring |
| `autominerVault.resolver.ts` | `autominerVaults(wallet)`, `autominerVault(wallet, index)` | — | Autominer state |
| `factions.resolver.ts` | `factions`, `faction(id)` | — | Faction stats |
| `gameAnalytics.resolver.ts` | `gameAnalytics` | — | Aggregated game metrics |
| `gameFeeHistory.resolver.ts` | `gameFeeHistory` | — | Fee/refining history |
| `lpPoolState.resolver.ts` | `lpPoolState` | — | Raydium pool data |
| `priceCycle.resolver.ts` | `priceCycles`, `currentPriceCycle` | — | Price snapshots |
| `solPrice.resolver.ts` | `solPrice` | — | SOL/USD price |
| `emissionsVault.resolver.ts` | `emissionsVault` | — | Emissions tracking |
| `referralRewards.resolver.ts` | `referralRewards(wallet)` | `claimReferralRewards` | Referral system |
| `refiningRewards.resolver.ts` | `refiningRewards(wallet)` | — | Unrefined reward state |
| `onchainAccounts.resolver.ts` | `onchainAccount(name)` | — | Cached on-chain state |
| `chat.resolver.ts` | `chatMessages(factionId, limit)` | `sendChatMessage` | In-game chat |
| `analytics.resolver.ts` | `playerActivity`, `dailyActiveUsers`, `retentionCohort` | — | Backend analytics |
| `degenBtcTokenomics.resolver.ts` | `tokenomics` | — | Token supply/burn data |
| `stakingWallet.resolver.ts` | `stakingWallet(wallet)` | — | Staking wallet summary |

### 6.2 API Gaps (Relative to Contract Changes)

| Missing Feature | Why It Matters |
|----------------|----------------|
| **Jackpot history query** | Frontend needs "past jackpots" leaderboard |
| **Jackpot near-miss feed** | Viral FOMO: "You were 3 rolls away from 500 SOL!" |
| **MVP leaderboard** | 12 heroes per war — needs dedicated query |
| **Paper hand burn feed** | Social proof of deflation |
| **HODL tax redistribution feed** | Diamond hands see who's paying them |
| **Win streak tracking** | Backend computes from events, no resolver yet |
| **Whale alert feed** | Big predictions need real-time toasts |
| **Fade-the-crowd indicator** | Crowd sentiment per round |
| **Mutation feed (privacy-preserving)** | Global ticker of mutations without wallet exposure |
| **Diamond hands detection** | Filter stakers by max lockup duration |

---

## 7. Orchestrator & Crankers

### 7.1 Game Loop (`game_loop.ts`)

**Wallet:** `GAME_WALLET_MNEMONIC` (BIP39 → `m/44'/501'/0'/0'`)

1. Detects `GlobalGameState` + `GameSession` stage
2. Waits for slot entropy before calling `endRound`
3. Triggers: `startRound()` → `endRound()` → `endRoundFactionRewards()`
4. Coordinates autominers: `startAutominerBetting()` / `stopAutominerBetting()`
5. Smart sleep: `min(60s, timeUntilRoundEnd)`

### 7.2 Economy Cycle (`snapshots_loop.ts`)

**Wallet:** `SNAPSHOTS_WALLET_MNEMONIC`

Runs 60s tick loop, one primary action per tick:
```
if lpOperationPending      → ADD_LP_AND_BURN
else if snapshots >= 8     → UPDATE_RATE
else if cooldown elapsed   → SNAPSHOT
else                       → WAIT
```

End-of-cycle maintenance tail:
1. `addLpAndBurn`
2. `settleFactionWar`
3. `crankDistributeTax`
4. `claimFactionTreasuryForFactionWar` (per active faction)
5. `distributeSolFees`

### 7.3 Autominer Bots

**Producer:** `autominerProducer.service.ts`
- Reads active autominers from Redis hash `autominer:active_vaults`
- Batches into groups of 5, pushes to BullMQ queue `autominer-tx-queue`

**Worker:** `autominerWorker.service.ts`
- **Wallet:** `AUTOMINER_WALLET_MNEMONIC` (main) + `autominer_bots.json` (10 sub-keypairs)
- Round-robins sub-wallets
- Uses v0 transactions + Address Lookup Table (5 bets/tx) or legacy (2 bets/tx)
- Funding loop: tops up sub-wallets every 5 min if < 0.01 SOL

### 7.4 Rebase Auto-Claim Loop (`rebase_claim_loop.ts`)

**Wallet:** `REBASE_CLAIM_WALLET_MNEMONIC`

- Runs every 45s
- Queries `UserRebaseReward` for pending claims
- Respects `allow_bots_to_claim` flag (cached 2 min TTL)
- Limits to 15 claims per pass
- Retry backoffs: unauthorized → 30min, not settled → 5min, generic → exp up to 6h

### 7.5 Tax Loop (`tax_loop.ts`) — Legacy

**Wallet:** `TAX_WALLET_MNEMONIC`

- Runs every 30s
- State machine: `IDLE → HARVESTING_TAX → DISTRIBUTING_TAX → CLAIMING_TREASURY`
- **Overlaps with economy cycle** — should be deprecated or merged

### 7.6 Wallet / Key Management

| Service | Env Var | Notes |
|---------|---------|-------|
| Game loop | `GAME_WALLET_MNEMONIC` | |
| Economy cycle | `SNAPSHOTS_WALLET_MNEMONIC` | |
| Tax loop | `TAX_WALLET_MNEMONIC` | |
| Autominer main | `AUTOMINER_WALLET_MNEMONIC` | |
| Autominer subs | `autominer_bots.json` | Generated keypairs, funded by main |
| Rebase claims | `REBASE_CLAIM_WALLET_MNEMONIC` | Falls back to autominer mnemonic |

---

## 8. NFT Minting Pipeline

### 8.1 Flow

```
DogeMinted Event → Indexer → DynamoDB (BtcDoge) → Queue Asset Gen Job
                                                    ↓
                              BullMQ Worker (sequential, concurrency=1)
                                                    ↓
              Decode DNA → Build Prompt → FAL.AI Full Body → Gemini Validate
                                                    ↓
                             FAL.AI DP (square crop) → Gemini Validate
                                                    ↓
                   FAL.AI Cinematic (optional, non-blocking) → Validate
                                                    ↓
                         Upload to S3 → Update DynamoDB → Regenerate Metadata JSON
```

### 8.2 DNA Decoding

**File:** `src/services/processEvents/doges.ts`

| Field | Bits | Values |
|-------|------|--------|
| Faction | 0–3 | 0–11 (12 factions) |
| Evolution | 4–6 | 0–7 (8 stages) |
| Appearance | 7–111 | 7 groups × 3 traits × 5 bits |
| Powers | 112–171 | 5 groups × 3 traits × 4 bits |
| Breed | 172–173 | 0–3 (4 per faction) |
| Type | byte 22 % 16 | 0–7 Wizard, 8–15 Muggle |
| Profession | byte 23 % 32 | 0–31 |

### 8.3 Asset Generation

**Model:** FAL.ai `nano-banana-pro/edit` (image-to-image)
**Validation:** Google Gemini 2.0 Flash (posture, style consistency, facing direction)

| Asset | Aspect Ratio | Resolution | Blocking? |
|-------|-------------|------------|-----------|
| Full Body | 3:4 | 1K | Yes |
| DP (avatar) | 1:1 | 1K | Yes |
| Cinematic | — | 2K | No |
| 3D Model | — | — | **No pipeline exists** |

### 8.4 Storage

```
s3://<bucket>/doge-assets/<faction_code>/<category_name>/region_<N>/<mint>/
  ├── full_body.png
  ├── dp.png
  └── cinematic.png

s3://<bucket>/doges/<mint>.json   ← metadata JSON
```

On-chain URI: `https://assets.minebtc.fun/doges/<mint>.json` (stable, never changes)

### 8.5 Critical NFT Pipeline Issues

| Issue | Severity | Description |
|-------|----------|-------------|
| **Faction drift** | 🔴 Critical | `ASSET_GENERATION.md` says factions 6,7,10,11 = Vietnam, Philippines, Brazil, Argentina. Code implements Iran, UK, Ukraine, Israel. Mismatch means wrong art for 4 factions. |
| **No on-chain metadata updates** | 🔴 Critical | Backend updates S3 JSON but never calls MPL Core `updateAsset`. Marketplaces cache stale metadata. Mutations are invisible on-chain. |
| **Hardcoded EC2 path** | 🔴 Critical | `BASE_BODIES_DIR = "/home/ec2-user/base_bodies"` — breaks in Docker/local. |
| **Single-worker bottleneck** | 🟡 High | Concurrency=1. 100 mints = ~1h 40m queue clear time. |
| **No decentralized storage** | 🟡 High | All on S3. No IPFS/Arweave backup. |
| **3D model field with no pipeline** | 🟡 Medium | Metadata has `animation_url` / `model3d` but no worker. |
| **Placeholder caching risk** | 🟡 Medium | Failed generations serve placeholder.png to marketplaces. |

---

## 9. Real-Time Layer

### 9.1 Redis Pub/Sub Bridge

- Workers publish completed events to Redis channel `game-events`
- Socket server subscribes and broadcasts via Socket.io rooms

### 9.2 Socket.io Rooms

| Room | Purpose |
|------|---------|
| `round:{round_id}` | Live round updates |
| `faction:{faction_id}` | Faction-specific events |
| `wallet:{wallet}` | Personal claimables, stakes |
| `global` | Global feed (jackpots, burns, etc.) |

### 9.3 Live Feed Service

`src/services/liveFeed.service.ts` + `realtimeState.service.ts`

Maintains:
- `feed:live` — recent event stream in Redis
- `economy:state` — cached economy metrics
- `tax:state` — cached tax metrics
- `round:current` — current round state
- `rebase:current` — current rebase state

### 9.4 Socket Events Emitted

From `socketEmitter.pubsub.ts`:
- `round:started`, `round:ended`, `round:rewardsDistributed`
- `player:claimablesUpdated`, `player:stakedPositionsUpdated`
- `faction:story`, `factionWar:state`
- `doge:minted`, `doge:synced`
- `feed:event`

**Missing socket events for contract changes:**
- `jackpot:hit`, `jackpot:nearMiss`, `jackpot:rolledOver`
- `paperHand:burned`
- `hodlTax:redistributed`
- `factionWar:mvp`
- `mutation:feed` (privacy-preserving)

---

## 10. Critical Backend Updates Needed

This section maps every contract change to the specific backend files that need updating.

### 10.1 Jackpot Rebrand (P0)

The contract renamed `motherlode` → `jackpot` globally. The backend must follow.

| Change | Files | Description |
|--------|-------|-------------|
| **Update IDL** | `idl/minebtc.json` | Replace `MotherlodeHit` → `JackpotHit`. Add `JackpotNearMiss`, `JackpotRolledOver`. Copy from `MineBtc-fi/target/idl/minebtc.json` after build. |
| **Event decoder** | `src/utils/eventDecoder.ts` | Should auto-handle via IDL, but verify BN/string coercion works for new event fields. |
| **Event router** | `src/services/processEvents/index.ts` | Add cases: `jackpotHit`, `jackpotNearMiss`, `jackpotRolledOver`. Rename `motherlodeHit` → `jackpotHit`. |
| **Game handler** | `src/services/processEvents/game.ts` | Rename `processMotherlodeHitEvent` → `processJackpotHitEvent`. Update all `motherlode_*` field references. |
| **Round model** | `src/model/Round.model.ts` | Rename fields: `dbtc_motherlode` → `dbtc_jackpot`, `motherlode_winning_rewards` → `jackpot_winning_rewards`, `motherlode_same_faction_rewards` → `jackpot_same_faction_rewards`, `motherlode_hit` → `jackpot_hit`, `motherlode_pot_size_on_hit` → `jackpot_pot_size_on_hit`. |
| **Round resolver** | `src/graphql/resolvers/round.resolver.ts` | Update `RoundResponseType` fields + mapping logic. |
| **Faction model** | `src/model/Faction.model.ts` | Rename: `motherlode_pot_size` → `jackpot_pot_size`, `last_motherlode_timestamp` → `last_jackpot_timestamp`, `last_motherlode_round_id` → `last_jackpot_round_id`, `last_motherlode_rewards` → `last_jackpot_rewards`. |
| **Faction resolver** | `src/graphql/resolvers/factions.resolver.ts` | Update field mappings. |
| **Redis state** | `src/services/redisState.service.ts` | Rename `motherlode:*` keys → `jackpot:*`. |
| **API cache** | `src/services/apiCache.service.ts` | Update cache invalidation keys. |
| **Dashboard services** | `src/services/arenaDashboard.service.ts`, `frontendDashboard.service.ts` | Rename all `motherlode` references. |
| **Socket events** | `src/utils/socketEmitter.pubsub.ts` | Add `emitJackpotHit()`, `emitJackpotNearMiss()`, `emitJackpotRolledOver()`. |
| **Live feed** | `src/services/liveFeed.service.ts` | Handle new jackpot event types in feed. |
| **DynamoDB migration** | script / backfill | Existing rows have `motherlode_*` fields. Either dual-write during transition or run a one-time migration script. |

### 10.2 Staking Visibility (P0)

| Change | Files | Description |
|--------|-------|-------------|
| **Update IDL** | `idl/minebtc.json` | `EmergencyWithdrawal` → `PaperHandBurned`, `RefiningFeeRedistributed` → `HodlTaxRedistributed`. |
| **Event router** | `src/services/processEvents/index.ts` | Rename cases: `emergencyWithdrawal` → `paperHandBurned`, `refiningFeeRedistributed` → `hodlTaxRedistributed`. |
| **Stake handler** | `src/services/processEvents/stake.ts` | Rename `processEmergencyWithdrawalEvent` → `processPaperHandBurnedEvent`. Rename `processRefiningFeeRedistributedEvent` → `processHodlTaxRedistributedEvent`. Update field extraction: `staked_token_type`, `days_remaining`, `paper_hand`, `tax_amount`. |
| **New tables** | `src/model/` | Create `PaperHandBurn.model.ts` and `HodlTaxRedistribution.model.ts` for feed history. Or extend existing `TaxDistribution` / `GameFeeAggregated`. |
| **GraphQL** | new resolver | `paperHandBurns(wallet?, limit)`, `hodlTaxRedistributions(wallet?, limit)` |
| **Socket events** | `src/utils/socketEmitter.pubsub.ts` | Add `emitPaperHandBurned()`, `emitHodlTaxRedistributed()`. |
| **Live feed** | `src/services/liveFeed.service.ts` | Add feed item types for burns and tax redistributions. |
| **Diamond Hands detection** | `src/graphql/resolvers/stakedPositions.resolver.ts` | Add query/filter: `isDiamondHand = lockup_duration == max_lockup_days`. No new event needed — derive from `MineBtcStaked` / `LiquidityStaked` events. |

### 10.3 Faction War MVP Bonus (P0)

| Change | Files | Description |
|--------|-------|-------------|
| **Update IDL** | `idl/minebtc.json` | Add `FactionWarMvp` event. |
| **Event router** | `src/services/processEvents/index.ts` | Add case `factionWarMvp`. |
| **Rebase handler** | `src/services/processEvents/rebase.ts` | Add `processFactionWarMvpEvent()`. At `FactionWarSettled`, the backend should already compute per-user rewards. Need to ALSO reserve 5% for MVPs and set `faction_mvp_bonus` per faction. |
| **Rebase model** | `src/model/Rebase.model.ts` | Add fields: `faction_mvp_user: [String]`, `faction_mvp_score: [String]`, `faction_mvp_bonus: [String]`. |
| **PlayerData model** | `src/model/PlayerData.model.ts` | Add `current_faction_war_score: String` (or Number). Reset each war. |
| **GraphQL** | `src/graphql/resolvers/rebase.resolver.ts` | Add MVP fields to `RebaseResponseType`. Add `mvpLeaderboard(factionWarId?)` query. |
| **Socket events** | `src/utils/socketEmitter.pubsub.ts` | Add `emitFactionWarMvp()` — broadcasts per faction. |
| **User rebase rewards** | `src/model/UserRebaseReward.model.ts` | Add `mvp_bonus_amount` field. Update claim logic to include MVP bonus in total. |
| **Rebase claim loop** | `src/scripts/rebase_claim_loop.ts` | No changes needed — claims the whole reward bundle including MVP. |

### 10.4 Mutation System Overhaul (Pending Contract Work)

When the contract removes global cap and adds per-user cooldown + guaranteed first mutation:

| Change | Files | Description |
|--------|-------|-------------|
| **PlayerData model** | `src/model/PlayerData.model.ts` | Add `last_mutation_round_id: Number`, `has_had_first_mutation: Boolean`. |
| **Global mutation feed** | new model + handler | Create `GlobalMutationFeed` model (privacy-preserving). Fields: `faction_id`, `mutation_type`, `new_stage`, `timestamp`. No wallet. |
| **GraphQL** | new resolver | `mutationFeed(limit)` — global ticker. |
| **Socket events** | `src/utils/socketEmitter.pubsub.ts` | `emitGlobalMutationFeed()` — "🧬 Someone in Russia just EVOLVED to Stage 4!" |
| **BtcDoge model** | `src/model/BtcDoge.model.ts` | Add `last_mutation_at: Number`, `consecutive_mutations: Number` (for streak bonuses, if implemented). |

### 10.5 Social / Viral Backend Features (P1)

These are backend-only (no contract changes needed) but critical for virality.

| Feature | Implementation | Files |
|---------|---------------|-------|
| **Win Streaks** | Indexer tracks `RoundRewardsClaimed` events per user. Count consecutive wins (win = claimed > 0). Store in `PlayerData.win_streak`, `PlayerData.max_win_streak`. | `src/services/processEvents/user.ts`, `src/model/PlayerData.model.ts` |
| **Whale Alerts** | `BetsPlaced` event: if `total_sol_bet` > threshold (e.g., 5 SOL or top-1% of round), emit whale alert. Store in live feed. | `src/services/processEvents/user.ts`, `src/services/liveFeed.service.ts` |
| **Fade the Crowd** | `RoundEnded` event: compute `sol_bets_by_faction` percentages. Cache per round. Frontend queries. | `src/services/processEvents/game.ts`, `src/services/redisState.service.ts` |
| **Jackpot Leaderboard** | New table `JackpotHistory`: `round_id`, `pot_size`, `winning_faction_id`, `winners_count`, `total_payout`. | `src/model/JackpotHistory.model.ts` |
| **Near-Miss Tracker** | `JackpotNearMiss` event: store in `PlayerData.near_misses` array or new table. Frontend shows "Close calls!" | `src/services/processEvents/game.ts` |
| **Paper Hand Leaderboard** | Aggregate `PaperHandBurned` events. Show biggest burns, most frequent paper hands. | `src/services/processEvents/stake.ts` |
| **HODL Tax Leaderboard** | Aggregate `HodlTaxRedistributed` events. Show total tax paid per user, total redistributed. | `src/services/processEvents/stake.ts` |

### 10.6 NFT Pipeline Fixes (P1)

| Fix | File | Description |
|-----|------|-------------|
| **Fix faction drift** | `src/prompts/index.ts`, `src/prompts/factions/` | Align with contract: ensure factions 0-11 map to the SAME countries as the contract. Document the definitive list. |
| **Add on-chain metadata updates** | `src/services/nftMetadata.service.ts` | After `DogeSynced` (mutation/evolution), call MPL Core `updateAsset` to refresh on-chain metadata hash. This forces marketplaces to re-fetch. |
| **Remove hardcoded EC2 path** | `src/services/assetGenerationWorker.service.ts` | Make `BASE_BODIES_DIR` an env var with fallback. |
| **Increase asset worker concurrency** | `src/queues/assetGenerationQueue.ts` | Increase from 1 to N (configurable, default 3). Monitor FAL.ai rate limits. |
| **Add IPFS/Arweave mirror** | `src/services/assetGenerationWorker.service.ts` | Upload to S3 AND pin to IPFS (or upload to Arweave/Shadow Drive). Update metadata to point to decentralized URI as primary, S3 as fallback. |

### 10.7 Infrastructure / DevOps (P2)

| Fix | Description |
|-----|-------------|
| **Add `faction_war_id` GSI to `mineBTC_rounds`** | Prevents full table scan in `aggregateRebaseVolumeFromRounds()` |
| **Persist live bets to DynamoDB** | `BetsPlaced` should write to `mineBTC_player_round_bets` immediately, not just Redis. Or use Redis persistence (AOF). |
| **Add idempotency checks** | Many events lack dedup beyond natural PK overwrite. Add `tx_signature` to idempotency key for delta-math events. |
| **Speed up fallback indexer** | 1-hour polling is too slow. Reduce to 5 minutes, or make it event-driven (Helius webhook with retries). |
| **Environment separation** | DynamoDB tables share names across environments. Add env suffix or use separate AWS accounts. |
| **Serverless.yml domain cleanup** | Still references `pinkyellow.dev`. Remove stale domains. |

---

## 11. Priority Matrix

### Must Do Before Mainnet (P0)

| # | Change | Effort | Files | Blocker? |
|---|--------|--------|-------|----------|
| 1 | Update IDL with renamed + new events | Low | `idl/minebtc.json` | ✅ Blocks all indexing |
| 2 | Rename `motherlode` → `jackpot` in event handlers | Medium | `processEvents/index.ts`, `game.ts` | ✅ Blocks jackpot visibility |
| 3 | Rename `motherlode` → `jackpot` in models | Medium | `Round.model.ts`, `Faction.model.ts` | ✅ Blocks jackpot visibility |
| 4 | Rename `motherlode` → `jackpot` in GraphQL | Low | `round.resolver.ts`, `factions.resolver.ts` | ✅ Frontend breakage |
| 5 | Rename `EmergencyWithdrawal` → `PaperHandBurned` handler | Low | `processEvents/index.ts`, `stake.ts` | ✅ Blocks burn visibility |
| 6 | Rename `RefiningFeeRedistributed` → `HodlTaxRedistributed` handler | Low | `processEvents/index.ts`, `stake.ts` | ✅ Blocks tax visibility |
| 7 | Add `FactionWarMvp` event handler | Low | `processEvents/index.ts`, `rebase.ts` | ✅ Blocks MVP bonuses |
| 8 | Add MVP fields to `Rebase` model | Low | `Rebase.model.ts` | ✅ Blocks MVP tracking |
| 9 | Add `current_faction_war_score` to `PlayerData` | Low | `PlayerData.model.ts` | ✅ Blocks MVP eligibility |
| 10 | Add new socket events (jackpot, MVP, burn, tax) | Low | `socketEmitter.pubsub.ts` | Frontend needs these |
| 11 | Fix faction drift in NFT pipeline | Medium | `src/prompts/index.ts`, `src/prompts/factions/*` | Wrong art = broken game |

### Should Do Before Launch (P1)

| # | Change | Effort | Impact |
|---|--------|--------|--------|
| 12 | Add jackpot history table + GraphQL query | Medium | 🔥🔥🔥 |
| 13 | Add paper hand burn feed table + query | Medium | 🔥🔥🔥 |
| 14 | Add HODL tax redistribution feed table + query | Medium | 🔥🔥🔥 |
| 15 | Implement win streak tracking | Medium | 🔥🔥 |
| 16 | Implement whale alerts | Low | 🔥🔥 |
| 17 | Implement fade-the-crowd indicator | Low | 🔥🔥 |
| 18 | Add `faction_war_id` GSI to rounds table | Low | Performance |
| 19 | Add on-chain MPL Core metadata updates | Medium | 🔥🔥🔥 |
| 20 | Add Diamond Hands detection query | Low | 🔥🔥 |
| 21 | Add global mutation feed (privacy-preserving) | Medium | 🔥🔥 |
| 22 | Speed up fallback indexer (1h → 5min) | Low | Reliability |

### Nice to Have (P2)

| # | Change | Effort |
|---|--------|--------|
| 23 | Add IPFS/Arweave mirror for NFT assets | Medium |
| 24 | Increase asset generation concurrency | Low |
| 25 | Add 3D model generation pipeline | High |
| 26 | Add environment suffix to DynamoDB tables | Low |
| 27 | Deprecate legacy tax loop | Low |
| 28 | Add comprehensive idempotency checks | Medium |

---

## Appendix A: Event Name Migration Cheat Sheet

When updating the backend, use this mapping:

| Old Event Name | New Event Name | Handler Function (Old) | Handler Function (New) |
|----------------|----------------|----------------------|----------------------|
| `MotherlodeHit` | `JackpotHit` | `processMotherlodeHitEvent` | `processJackpotHitEvent` |
| *(new)* | `JackpotNearMiss` | — | `processJackpotNearMissEvent` |
| *(new)* | `JackpotRolledOver` | — | `processJackpotRolledOverEvent` |
| `EmergencyWithdrawal` | `PaperHandBurned` | `processEmergencyWithdrawalEvent` | `processPaperHandBurnedEvent` |
| `RefiningFeeRedistributed` | `HodlTaxRedistributed` | `processRefiningFeeRedistributedEvent` | `processHodlTaxRedistributedEvent` |
| *(new)* | `FactionWarMvp` | — | `processFactionWarMvpEvent` |

### New Event Schemas (from contract)

```typescript
// JackpotNearMiss
{
  round_id: u64,
  roll: u64,
  threshold: u64,
  pot_size: u64,
  timestamp: i64,
}

// JackpotRolledOver
{
  round_id: u64,
  pot_size: u64,
  reason: u8, // 0 = no exact winners
  timestamp: i64,
}

// PaperHandBurned (was EmergencyWithdrawal)
{
  owner: Pubkey,
  player_data: Pubkey,
  position_index: u8,
  position_key: Pubkey,
  staked_token_type: u8, // 0=MineBTC, 1=LP
  original_amount: u64,
  penalty_amount: u64,
  returned_amount: u64,
  penalty_tax_pct: u64,
  days_remaining: u64,
  timestamp: i64,
}

// HodlTaxRedistributed (was RefiningFeeRedistributed)
{
  paper_hand: Pubkey,
  player_data: Pubkey,
  tax_amount: u64,
  redistributed_amount: u64,
  redistributed_index_increment: u128,
  remaining_total_claimable: u64,
  timestamp: i64,
}

// FactionWarMvp
{
  faction_war_id: u64,
  faction_id: u8,
  user: Pubkey,
  mvp_score: u64,
  bonus_amount: u64,
  timestamp: i64,
}
```

---

## Appendix B: Quick Start for Backend Developers

### Local Dev Setup

```bash
cd MineBtcBackend

# 1. Install dependencies
yarn install

# 2. Start Valkey (Redis-compatible)
docker run -d --name valkey -p 6379:6379 valkey/valkey:alpine

# 3. Configure environment
cp .env.example .env
# Edit .env: NODE_ENV=localnet, SOLANA_RPC_URL, DYNAMO_* keys, etc.

# 4. Run individual services
yarn webhook-server    # Port 3003
yarn worker            # BullMQ consumer
yarn socket-server     # Port 8080
yarn game-loop         # Round cranker
yarn economy-cycle     # Unified economy cranker

# 5. Or run the orchestrator (spawns all)
yarn orchestrator
```

### GraphQL Local Playground

```bash
yarn graphql-local     # Runs Apollo Server on PORT (default 3001)
# Visit http://localhost:3001/graphql
```

### Updating the IDL

After building the contract in `MineBtc-fi`:

```bash
cp ../MineBtc-fi/target/idl/minebtc.json ./idl/minebtc.json
# Then restart the worker process to pick up new events
```

---

*End of Backend Assessment.*
