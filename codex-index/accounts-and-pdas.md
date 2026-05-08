# Accounts and PDAs

Canonical source: `programs/mineBTC/src/state.rs`.

## Core Config Accounts

- `GlobalConfig` PDA `[global-config]`
  - Authority, pending authority, fee recipient, SOL treasury PDA, supported factions, fee configs, Raydium pool state, snapshot interval, gameplay tuning, pause flag.
  - Fee config is whole-percent based for SOL fees and degenBTC distributions.
  - Pause blocks new bets, autominers, round starts, HashBeast mints, and breeding. Settlement, claims, staking, economy cranks remain available.

- `MineBtcMining` PDA `[mine-btc-mining]`
  - Token vault, emission rate per round, total mined/distributed, Raydium pool, price history, recent/track price, POL tracking, LP token price, emission adjustment params, LP operation pending flag.
  - Price history max: 8 entries, matching the 4-hour conceptual economy cycle at 30-minute default snapshots.

- `HodlPool` PDA `[hodl-pool]`
  - `hodl_tax_index` and `total_minebtc_claimable`; powers HODL tax redistribution on degenBTC withdrawals.

- `HashpowerConfig` PDA `[hashpower-config]`
  - Lockup day bounds and lockup multiplier. Setup config expects base 100 and max 300.

## Game Accounts

- `GlobalGameSate` PDA `[global-game-state]`
  - Active flag, can-begin-round flag, current round, round duration, last completed round, last winning faction, global jackpot pot.

- `GameSession` PDA `[game-session, round_id_u64_le]`
  - Round stage: 0 active, 1 winner finalized/pending faction reward distribution, 2 rewards finalized.
  - Timing: start slot/timestamp, end timestamp, scheduled entropy slot, entropy used, fallback marker.
  - Bet totals: SOL, points, weighted points, staker fee.
  - Per-faction arrays: user indexes, SOL by faction, points/weighted points by faction-direction.
  - Result fields: winning faction/direction, reward pools, reward indexes, jackpot details.
  - Mutation telemetry: highest faction bet, per-faction mutation counts, total mutations this round.

- `UserGameBet` PDA `[user-bet, user_pubkey, round_id_u64_le]`
  - A user can hold multiple faction-direction positions in one round.
  - Stores faction IDs, directions, SOL bets, point/ticket bets, weighted points, totals, fee, gameplay HashBeast, mutation type, faction-war accumulation flag.

## Player and Referral Accounts

- `PlayerData` PDA `[player, user_pubkey]`
  - Wallet owner, referral code, current/origin faction, referrer faction, same-faction referral flag.
  - degenBTC and LP staking hashpower/staked/reward debt.
  - Pending SOL, pending degenBTC, unrefined degenBTC, pending round and faction-war claims.
  - Position indexes, staked HashBeasts, passive HashBeast multiplier, free ticket balances.
  - Gameplay HashBeast lock state, active multiplier, cached DNA/XP, unlock request cycle, current faction-war score.

- `ReferralRewards` PDA `[referral-rewards, referrer_pubkey]`
  - Referrer owner, owner faction, referral counts, same-faction counts, per-faction recruit counts, pending SOL rewards, total SOL earned.
  - Lifetime referrer cap: 100,000 SOL.
  - System no-referral sentinel is `[referral-rewards, system_program_id]`; frontend/backend should still pass it when no user referral is used.

## Autominer Accounts

- `AutominerVault` PDA `[autominer, user_pubkey]`
  - Faction config: specific faction-direction picks or random count with shared direction.
  - SOL per round, remaining rounds, last bet round, reserve balance, reload flag, optional ticket tier.
  - Funds are held in global autominer custody PDA `[autominer-custody]`.

## HashBeast NFT Accounts

- `HashBeastConfig` PDA `[hashbeast-config]`
  - Metaplex Core collection, total lifetime mints, breeding flag and breeding curve.

- `HashBeastMintConfig` PDA `[hashbeast-mint-config]`
  - Genesis mint activity, base price, curve, genesis cap, mints by faction, ticket tiers.

- `HashBeastMetadata` PDA `[hashbeast-metadata, hashbeast_mint]`
  - Core asset mint, parents, breed count/cooldown, rebirth_count (0–7, also encoded in DNA bits at offset 174), created timestamp, faction, multiplier, accumulated value, DNA, incubated player, last update, XP.

- `HashBeastFreeMintAllowance` PDA `[hashbeast-free-mint-allowance, user_pubkey]`
  - Remaining free genesis mints.

## Staking Accounts

- `StakedPosition` PDA
  - Used for both degenBTC and LP positions; fields include position type, index, faction, staked amount, weighted amount, timestamps, lockup duration, multiplier.

- Custodians:
  - degenBTC custodian `[minebtc-custodian]` and authority `[minebtc-custodian-authority]`.
  - LP custodian `[lp-custodian]` and authority `[lp-custodian-authority]`.

## Faction War Accounts

- `FactionWarConfig` PDA `[faction-war-config]`
  - Current faction-war ID, active flag, settlement LP operation target, previous ranks, cycle telemetry, current degenBTC mining multiplier.
  - Mining multiplier stored in bps; default 10,000 = 1.0x, contract caps 1,000 to 30,000.

- `FactionWarState` PDA `[faction-war, faction_war_id_u64_le]`
  - Stage 0 active, 1 settled/claims open.
  - Start/final ranks, rank deltas, resolved directions, MVP users/scores/bonuses.
  - Direction totals, SOL direction totals, loyalty totals, reward pools, round wins, SOL totals, gameplay scores, eligible HashBeast direction totals.
  - Treasury tax base, treasury claim bitmap, SOL cycle reward pool.

- `UserFactionWarBets` PDA `[user-faction-war, user_pubkey, faction_war_id_u64_le]`
  - Per-user weighted and SOL direction bets for a cycle, gameplay HashBeast, hashbeast bonus eligibility.

## Tax Accounts

- `TaxConfig` PDA `[tax-config]`
  - faction_treasury_pct, burn_pct, total_burnt, unassigned faction-war treasury, withdraw_withheld_authority pubkey, faction_treasury_vault pubkey.
  - Tax split is `faction_treasury_pct + burn_pct + (residual → mining vault)`. NFT floor sweep slice has been removed — NFT market making is funded from SOL via `distribute_sol_fees::nft_market_making_pct` (default 3%) routing into `inventory_sweep_vault`.
  - Rank-weighted treasury split: 80%; lucky draw: 20%.

- Tax vaults:
  - `[withdraw-withheld-authority]`
  - `[faction-treasury-vault]`

## NFT Marketplace + Inventory Accounts

- `InventoryPool` PDA `[inventory-pool]`
  - Bump, cached `marketplace_program` + `marketplace_config` for CPI validation, `total_count` (cap-checked against `MAX_INVENTORY = 200`). Doubles as the on-chain custody address (mpl-core asset `owner`) for program-held HashBeasts.

- `RebornEntry` PDA `[reborn-entry, asset]`
  - One per HashBeast currently held by `inventory_pda`. Tracks status (Lootbox=0, Listed=1), origin (Reborn=0, Swept=1), `original_buy_price` (anchor for relist markup math), `expire_count` (strike count, max `MAX_EXPIRES`), faction_id, quality_score, listing_price.

- `LootboxQueue` PDA `[lootbox-queue, faction_id]`
  - Per-country lootbox queue; 5-slot packed array of asset pubkeys; recycle / sweep flows push, loser-roll claims pop.

- `LootboxClaim` PDA `[lootbox-claim, user]`
  - Per-user reservation when a losing claim's roll wins; `claim_lootbox_nft` consumes it.

- `FloorQueue` PDA `[floor-queue]`
  - Singleton 20-entry sorted-ascending queue of cheapest user-listed HashBeasts. Program-owned listings explicitly excluded.

- `SaleHistory` PDA `[sale-history]`
  - Singleton 32-slot ringbuffer of qualifying user-to-user sales (buyer ≠ inventory_pda, seller ≠ inventory_pda, listed ≥ 5 minutes before sale).

- `FloorHistory` PDA `[floor-history]`
  - Singleton 7-day rolling buffer of daily floor anchors. `compute_trend_bps()` returns the 7-day delta in bps clamped to [-10000, +10000].

- `[inventory-sweep-vault]`
  - System-owned SOL vault. Funded by `distribute_sol_fees::nft_market_making_pct` (default 3%) and 50% of `handle_inventory_proceeds`. Source of capital for `sweep_floor_lowest` and keeper bounties.
