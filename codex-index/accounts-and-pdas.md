# Accounts and PDAs

Canonical source: `programs/mineBTC/src/state.rs`.

## Core Config Accounts

- `GlobalConfig` PDA `[global-config]`
  - Authority, pending authority, fee recipient, SOL treasury PDA, supported factions, fee configs, Raydium pool state, snapshot interval, gameplay tuning, pause flag.
  - Fee config is whole-percent based for SOL fees and dogeBTC distributions.
  - Pause blocks new bets, autominers, round starts, Doge mints, and breeding. Settlement, claims, staking, economy cranks remain available.

- `MineBtcMining` PDA `[mine-btc-mining]`
  - Token vault, emission rate per round, total mined/distributed, Raydium pool, price history, recent/track price, POL tracking, LP token price, emission adjustment params, LP operation pending flag.
  - Price history max: 8 entries, matching the 4-hour conceptual economy cycle at 30-minute default snapshots.

- `HodlPool` PDA `[hodl-pool]`
  - `hodl_tax_index` and `total_minebtc_claimable`; powers HODL tax redistribution on dogeBTC withdrawals.

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
  - Stores faction IDs, directions, SOL bets, point/ticket bets, weighted points, totals, fee, gameplay Doge, mutation type, faction-war accumulation flag.

## Player and Referral Accounts

- `PlayerData` PDA `[player, user_pubkey]`
  - Wallet owner, referral code, current/origin faction, referrer faction, same-faction referral flag.
  - dogeBTC and LP staking hashpower/staked/reward debt.
  - Pending SOL, pending dogeBTC, unrefined dogeBTC, pending round and faction-war claims.
  - Position indexes, staked Doges, passive Doge multiplier, free ticket balances.
  - Gameplay Doge lock state, active multiplier, cached DNA/XP, unlock request cycle, current faction-war score.

- `ReferralRewards` PDA `[referral-rewards, referrer_pubkey]`
  - Referrer owner, owner faction, referral counts, same-faction counts, per-faction recruit counts, pending SOL rewards, total SOL earned.
  - Lifetime referrer cap: 100,000 SOL.
  - System no-referral sentinel is `[referral-rewards, system_program_id]`; frontend/backend should still pass it when no user referral is used.

## Autominer Accounts

- `AutominerVault` PDA `[autominer, user_pubkey]`
  - Faction config: specific faction-direction picks or random count with shared direction.
  - SOL per round, remaining rounds, last bet round, reserve balance, reload flag, optional ticket tier.
  - Funds are held in global autominer custody PDA `[autominer-custody]`.

## Doge NFT Accounts

- `DogeConfig` PDA `[doge-config]`
  - Metaplex Core collection, max lifetime supply, total lifetime mints, breeding flag and breeding curve.

- `DogeMintConfig` PDA `[doge-mint-config]`
  - Genesis mint activity, base price, curve, genesis cap, mints by faction, ticket tiers.

- `DogeMetadata` PDA `[doge-metadata, doge_mint]`
  - Core asset mint, parents, breed count/cooldown, created timestamp, faction, multiplier, accumulated value, DNA, incubated player, last update, XP.

- `DogeFreeMintAllowance` PDA `[doge-free-mint-allowance, user_pubkey]`
  - Remaining free genesis mints.

## Staking Accounts

- `StakedPosition` PDA
  - Used for both dogeBTC and LP positions; fields include position type, index, faction, staked amount, weighted amount, timestamps, lockup duration, multiplier.

- Custodians:
  - dogeBTC custodian `[minebtc-custodian]` and authority `[minebtc-custodian-authority]`.
  - LP custodian `[lp-custodian]` and authority `[lp-custodian-authority]`.

## Faction War Accounts

- `FactionWarConfig` PDA `[faction-war-config]`
  - Current faction-war ID, active flag, settlement LP operation target, previous ranks, cycle telemetry, current dogeBTC mining multiplier.
  - Mining multiplier stored in bps; default 10,000 = 1.0x, contract caps 1,000 to 30,000.

- `FactionWarState` PDA `[faction-war, faction_war_id_u64_le]`
  - Stage 0 active, 1 settled/claims open.
  - Start/final ranks, rank deltas, resolved directions, MVP users/scores/bonuses.
  - Direction totals, loyalty totals, reward pools, round wins, SOL totals, mutation scores, eligible Doge direction totals.
  - Treasury tax base, treasury claim bitmap, SOL cycle reward pool.

- `UserFactionWarBets` PDA `[user-faction-war, user_pubkey, faction_war_id_u64_le]`
  - Per-user weighted direction bets for a cycle, gameplay Doge, doge bonus eligibility.

## Tax Accounts

- `TaxConfig` PDA `[tax-config]`
  - NFT floor sweep %, faction treasury %, burn %, total burned, unassigned faction-war treasury, vaults, whitelist.
  - Rank-weighted treasury split: 80%; lucky draw: 20%.

- Tax vaults:
  - `[withdraw-withheld-authority]`
  - `[faction-treasury-vault]`
  - `[nft-floor-sweep-vault]`
  - `[nft-sale-sol-vault]`
