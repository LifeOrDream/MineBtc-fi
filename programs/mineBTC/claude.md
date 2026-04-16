# MineBTC Program Guide

This file is the source-of-truth orientation note for anyone editing the `mineBTC` Anchor program.

## Product Framing

MineBTC is a **degen country arena game** on Solana where:

- players pick a country and a direction, bet SOL, and their doge NFTs evolve through gameplay
- doge mutations during rounds decide which country climbs the leaderboard each cycle
- the same bet powers both the **round raffle** (instant SOL + dogeBTC rewards) and the **cycle leaderboard** (longer-term dogeBTC rewards based on which countries moved)
- a deflationary economy runs on a 1% transfer tax: burn + NFT floor sweep + faction treasury + mining vault
- an automated economy cycle (price snapshots → rate adjustment → LP burn) keeps tokenomics self-sustaining

**The game in one sentence:** "Pick your country, bet SOL, your doge evolves, your country climbs, you earn dogeBTC."

Use these canonical terms:

- `country` or `faction` — one of 12-15 playable nations
- `direction` — `Down`, `Neutral`, `Up`
- `round` — fast 60-second betting loop with random winner
- `rebase` — longer competitive period tied to the economy cycle (LP burn cadence), where mutation scores determine country rankings
- `operator doge` / `gameplay doge` — the live NFT locked for rounds, earns XP, can mutate
- `staked doges` — passive NFTs that boost staking hashpower
- `mutation` — a doge upgrade (Evolution / Power / Trait) triggered by betting
- `mutation score` — contribution to your country's leaderboard rank from a mutation event

Do **not** describe the system as:

- a prediction market
- a geopolitical risk index
- an intelligence data pipeline
- a Bittensor subnet product

When in doubt, lead with what is already live and playable.

## Core Game Model

### Round Layer

Players place one or more `country + direction` bets during an active round.

At round end:

1. the contract randomly chooses a winning country from countries that received bets
2. it randomly chooses a winning direction on that country
3. payouts are split into:

- exact `country + direction` winners: main SOL + dogeBTC round rewards
- same-country wrong-direction bettors: consolation dogeBTC rewards
- winning-country stakers: staking reward share
- motherlode jackpot: extra dogeBTC for exact winners when hit

### Cycle Layer (Mutation Leaderboard)

Each round bet also accumulates into the active cycle. Doge mutations that fire during rounds contribute score to their faction.

A cycle is defined by:

- `rebase_id`
- `start_ranks` (from previous cycle)
- `faction_mutation_scores` (accumulated during this cycle)
- per-country direction totals (own-faction bets only)

**How cycles work:**

1. Cycle auto-starts on first bet after the previous cycle settles
2. Each doge mutation adds score: `type_weight × bet_size × doge_multiplier`
3. Cycle settles when the economy cycle's LP burn completes
4. Factions are ranked by total mutation scores → rank changes resolve directions
5. Players who bet the correct direction on their own country earn dogeBTC rewards
6. Rewards are proportional: each country's share = its winning-direction bets / total winning bets across all countries

**When mutations are disabled** (rpg_progression off), no cycle rewards are distributed.

### Doge Layer

Two distinct doge roles:

- `gameplay_doge`: one operator doge locked for live play, carries multiplier, DNA, XP cache
- `staked_doges`: up to 5 passive boosts for staking hashpower

**Mutation system:**

- Mutations trigger during betting (SOL bets only, requires gameplay doge)
- Max mutations per round = `active_factions / 3` (global budget creates scarcity)
- Per-faction difficulty scaling: each mutation in a round makes the next one harder for that faction
- Base chance 20%, reduced by doge multiplier (high-mult doges mutate less often)
- Types: Evolution (~10%), Power (~30%), Trait (~60%)
- XP boosts multiplier on mutation: Evolution 5-10% of XP, Power/Trait 2-5% of XP
- active_multiplier capped at MAX_MULTIPLIER (10x)
- 2-step gameplay doge unlock prevents mid-cycle withdrawal gaming

### Economy Layer

- 1% transfer tax on all dogeBTC: split between burn, NFT floor sweep vault, faction treasury, mining vault
- Price snapshots every 30 min (8 per cycle) → emission rate adjustment → LP add + burn
- Cycle settlement is tied to the LP burn — one competitive cycle per economy cycle
- Daily faction leaderboard distributes treasury rewards by hashpower ranking

## Important Accounts

### Global / Shared

- `GlobalConfig`
- `GlobalGameSate`
- `MineBtcMining`
- `RebaseConfig`
- `RebaseState`
- `UnrefinedRewards`

### Per Country

- `FactionState`

### Per Player

- `PlayerData`
- `UserGameBet`
- `UserRebaseBets`
- `AutominerVault`
- `StakedPosition`
- `DogeMetadata`

## Main File Ownership

| File | Main Responsibility |
|------|----------------------|
| `instructions/game.rs` | Round start/end, winner selection, round reward indexes |
| `instructions/user.rs` | Betting, autominers, round claims, gameplay doges, mutations |
| `instructions/rebase.rs` | Cycle config, mutation-based settlement, cycle claims |
| `instructions/stake.rs` | dogeBTC and LP staking |
| `instructions/doges.rs` | Doge minting, breeding, staking, gameplay lock/unlock |
| `instructions/economy.rs` | Price snapshots, emissions, POL |
| `instructions/tax.rs` | Transfer-tax accounting and faction treasury distribution |
| `state.rs` | Account layouts and canonical constants |
| `events.rs` | Indexer-facing event contracts |
| `errors.rs` | Reusable program errors |

## Critical Flows

### Betting

`instructions/user.rs`

- `internal_join_round`
- `internal_join_round_batch`
- `internal_process_bets`

Important rules:

- bets are `BetType::FactionDirection`
- one transaction can include multiple countries
- the same bet updates both `GameSession` totals and `RebaseState` / `UserRebaseBets`
- only own-faction bets accumulate into rebase state (cross-faction bets are round-only)
- mutations fire during bet processing, subject to global budget and per-faction penalty

### Round Settlement

`instructions/game.rs`

- `int_start_round`
- `int_end_round`
- `int_end_round_faction_rewards`

Important rules:

- randomness is commit-reveal based (scheduled slot hash)
- winner selection is `country → direction`
- `end_round_faction_rewards` also advances rebase mining accounting and auto-settles the cycle when the LP burn completes

### Cycle Settlement

`instructions/rebase.rs`

- `settle_rebase_internal` — permissionless, anyone can crank once LP burn completes
- `claim_rebase_rewards_internal` — user claims their share

Important rules:

- settlement is gated by `mining.pol_stats.lp_operations_count >= rebase_config.rebase_settle_cycle`
- if no mutations occurred (all scores = 0), no rewards are distributed
- rankings are computed from `faction_mutation_scores`, compared to previous cycle's ranks

### Autominers

`instructions/user.rs`

Autominer supports:

- `FactionsConfig::Specific { picks }`
- `FactionsConfig::Random { count, direction }`

Important rules:

- SOL mode uses `sol_per_round` as the full round budget
- ticket mode requires `sol_per_round == 0`
- ticket mode does not reserve SOL and does not pay keeper compensation

## PDA Notes

Use seeds from `state.rs`, not memory or stale docs.

Common ones:

```rust
// Global
[b"global-config"]
[b"global-game-state"]
[b"mine-btc-mining"]
[b"rebase-config"]
[b"unrefined-rewards"]

// Per entity
[b"faction", faction_name.as_bytes()]
[b"player", user.key().as_ref()]
[b"game-session", &round_id.to_le_bytes()]
[b"user-bet", user.key().as_ref(), &round_id.to_le_bytes()]
[b"rebase", &rebase_id.to_le_bytes()]
[b"user-rebase", user.key().as_ref(), &rebase_id.to_le_bytes()]
[b"autominer", user.key().as_ref()]
[b"autominer-custody"]
[b"doge-metadata", doge_mint.key().as_ref()]
[b"doge-custody"]
```

Important gotcha:

- `FactionState` uses the **faction name bytes**, not the numeric faction id

## Event Expectations

Key product events for indexers and off-chain systems:

- `RoundStarted`
- `BetsPlaced`
- `RoundEnded`
- `MotherlodeHit`
- `RoundRewardsClaimed`
- `MutationTriggered`
- `MutationScoreAccumulated`
- `RebaseAutoStarted`
- `RebaseAutoSettled`
- `RebaseSettled`
- `RebaseRewardsClaimed`
- `AutominerInitialized`
- `AutominerReloaded`
- `DogeUsedForGameplay`
- `DogeSynced`

## Documentation Rules

When changing this repo:

- keep README language aligned with the current contract model
- describe the product as a **degen country arena game** with mutation-driven competitive cycles
- do NOT use "prediction market", "geopolitical risk", "intelligence", "data pipeline", or "oracle"
- prefer simple degen-native language: "bet", "evolve", "climb", "earn"

## Verification Checklist

Run these after meaningful contract edits:

```bash
cargo fmt --all
cargo check -p minebtc
cargo test -p minebtc --lib
```

If you touch tracked setup scripts, also syntax-check them:

```bash
node --check setup_scripts/loop_scripts/game_loop.js
```
