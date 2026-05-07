# MineBTC Program Guide

This file is the source-of-truth orientation note for anyone editing the `mineBTC` Anchor program.

## Product Framing

MineBTC is a **degen country arena game** on Solana where:

- players pick a country and a direction, bet SOL, and winning claims can evolve their doge NFTs
- own-country gameplay support decides which country climbs the leaderboard each cycle
- the same bet powers both the **round raffle** (instant SOL + degenBTC rewards) and the **cycle leaderboard** (longer-term degenBTC rewards based on which countries moved)
- a deflationary economy runs on a 0.1% transfer tax: burn + NFT floor sweep + faction treasury + mining vault
- an automated economy cycle (price snapshots → rate adjustment → LP burn) keeps tokenomics self-sustaining

**The game in one sentence:** "Pick your country, bet SOL, win claims, your doge evolves, your country climbs, you earn degenBTC."

Player country is permanent after signup. Referral rewards are also
country-aware: referred users always get the same 1% degenBTC claim bonus, while
referrers earn a higher degenBTC reward when they recruit someone into their own
country. This keeps the growth loop simple: "bring people to your flag."

Use these canonical terms:

- `country` or `faction` — one of 12-15 playable nations
- `direction` — `Down`, `Neutral`, `Up`
- `round` — fast 60-second betting loop with random winner
- `faction war` — longer competitive period tied to the economy cycle (LP burn cadence), where gameplay scores determine country rankings
- `operator doge` / `gameplay doge` — the live NFT locked for rounds, contributes gameplay score, earns XP from eligible claim rolls, can mutate
- `staked doges` — passive NFTs that boost staking hashpower
- `story event` — a claim-time Doge event (Evolution / Power / Trait); the contract may mutate DNA internally, but the backend decides how to render it
- `gameplay score` — contribution to your country's leaderboard rank from own-country SOL support with an active gameplay doge

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

- exact `country + direction` winners: main SOL + degenBTC round rewards
- same-country wrong-direction bettors: consolation degenBTC rewards
- winning-country stakers: staking reward share
- global jackpot: extra degenBTC for exact winners of the selected faction when hit

### Cycle Layer (Gameplay Score Leaderboard)

Each round bet also accumulates into the active cycle. Own-country SOL bets from users with an active gameplay doge contribute deterministic gameplay score to their faction.

A cycle is defined by:

- `faction_war_id`
- `start_ranks` (from previous cycle)
- `faction_gameplay_scores` (internal field; accumulated gameplay scores during this cycle)
- per-country direction totals, plus separate own-country loyalty totals

**How cycles work:**

1. Cycle auto-starts on first bet after the previous cycle settles
2. Each eligible own-country SOL bet adds score: `GAMEPLAY_SUPPORT_SCORE_WEIGHT × bet_size × doge_multiplier`
3. Cycle settles when the economy cycle's LP burn completes
4. Factions are ranked by total gameplay scores, with round wins and SOL support as tiebreakers
5. Rank changes resolve each country's winning direction
6. Players who bet correct directions earn degenBTC via the base pool; own-country correct bettors also share the loyalty pool

**When RPG progression is disabled** (`rpg_progression` off), cycle gameplay scoring and mutation rolls pause.

### Doge Layer

Two distinct doge roles:

- `gameplay_doge`: one operator doge locked for live play, carries multiplier, DNA, XP cache
- `staked_doges`: up to 3 passive boosts for staking hashpower

**Mutation/story event system:**

- Story events trigger during winning round or faction-war reward claims (SOL stake only, requires gameplay doge)
- Round claim odds use the winning faction stake; exact wins get stronger odds than same-faction consolation wins
- Faction-war claim odds are strongest for own-country correct calls, especially when the country moved Up
- Round-level mutation counters still create scarcity and pacing pressure
- Per-faction difficulty scaling: each event in a round makes the next one harder for that faction
- Base chance is configurable, reduced by Doge multiplier (high-mult Doges trigger less often)
- Types: Evolution (~10%), Power (~30%), Trait (~60%)
- XP boosts multiplier on story events: Evolution 5-10% of XP, Power/Trait 2-5% of XP
- active_multiplier capped at `GAMEPLAY_MAX_MULTIPLIER` (4.2x)
- 2-step gameplay doge unlock prevents mid-cycle withdrawal gaming

**Doge mint supply:**

- `DogeConfig` owns non-sale state: collection, lifetime max supply, total minted, and breeding config
- `DogeMintConfig` owns genesis-sale-only state: bonding curve price, ticket tiers, genesis mint count, and per-country caps
- Genesis mints are capped separately from lifetime supply; current deployment config targets 12,000 genesis mints with 1,000 max per country
- Burns via `send_to_heaven` do not reduce lifetime minted supply; breeding can only mint while `total_doges_minted < max_supply`

### Economy Layer

- 0.1% transfer tax on all degenBTC: split between burn, NFT floor sweep vault, faction treasury, mining vault
- Price snapshots every 30 min (8 per cycle) → emission rate adjustment → LP add + burn
- Cycle settlement is tied to the LP burn — one competitive cycle per economy cycle
- Daily faction leaderboard distributes treasury rewards by hashpower ranking

## Important Accounts

### Global / Shared

- `GlobalConfig`
- `GlobalGameSate`
- `MineBtcMining`
- `FactionWarConfig`
- `FactionWarState`
- `HodlPool`

### Per Country

- `FactionState`

### Per Player

- `PlayerData`
- `UserGameBet`
- `UserFactionWarBets`
- `AutominerVault`
- `StakedPosition`
- `DogeMetadata`

## Main File Ownership

| File | Main Responsibility |
|------|----------------------|
| `instructions/game.rs` | Round start/end, winner selection, round reward indexes |
| `instructions/user.rs` | Betting, autominers, round claims, gameplay doges, story events |
| `instructions/faction_war.rs` | Cycle config, gameplay-score settlement, cycle claims |
| `instructions/stake.rs` | degenBTC and LP staking |
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
- the same bet updates both `GameSession` totals and `FactionWarState` / `UserFactionWarBets`
- all country-direction bets feed the base faction-war pool; own-faction bets also feed loyalty rewards
- active gameplay Doges on own-country SOL bets add gameplay score during bet processing
- story events fire later during winning reward claims, using the recorded bet context

### Round Settlement

`instructions/game.rs`

- `int_start_round`
- `int_end_round`
- `int_end_round_faction_rewards`

Important rules:

- randomness is commit-reveal based (scheduled slot hash)
- winner selection is `country → direction`
- `end_round_faction_rewards` also advances faction-war mining accounting and auto-settles the cycle when the LP burn completes

### Cycle Settlement

`instructions/faction_war.rs`

- `settle_faction_war_internal` — permissionless, anyone can crank once LP burn completes
- `claim_faction_war_rewards_internal` — user claims their share

Important rules:

- settlement is gated by `mining.pol_stats.lp_operations_count >= faction_war_config.faction_war_settle_cycle`
- if no bets occurred, no cycle rewards are distributed
- rankings are computed from the internal `faction_gameplay_scores` array, then compared to previous cycle ranks

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
[b"faction-war-config"]
[b"hodl-pool"]

// Per entity
[b"faction", faction_name.as_bytes()]
[b"player", user.key().as_ref()]
[b"game-session", &round_id.to_le_bytes()]
[b"user-bet", user.key().as_ref(), &round_id.to_le_bytes()]
[b"faction-war", &faction_war_id.to_le_bytes()]
[b"user-faction-war", user.key().as_ref(), &faction_war_id.to_le_bytes()]
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
- `JackpotHit`
- `RoundRewardsClaimed`
- `StoryEventTriggered`
- `GameplayScoreAccumulated`
- `FactionWarAutoStarted`
- `FactionWarAutoSettled`
- `FactionWarSettled`
- `FactionWarRewardsClaimed`
- `AutominerInitialized`
- `AutominerReloaded`
- `DogeUsedForGameplay`
- `DogeSynced`

## Documentation Rules

When changing this repo:

- keep README language aligned with the current contract model
- describe the product as a **degen country arena game** with gameplay-score-driven competitive cycles
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
