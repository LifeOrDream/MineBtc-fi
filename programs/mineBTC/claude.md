# MineBTC Program Guide

This file is the source-of-truth orientation note for anyone editing the `mineBTC` Anchor program.

## Product Framing

Use this framing everywhere in code comments, docs, setup scripts, and assistant output:

- MineBTC is a **live country arena game** with an **index-driven epoch prediction layer**
- the same bet powers both the round game and the epoch market
- the game is also a **data pipeline**
- the category is **Geopolitical Risk Intelligence**

Do **not** describe the current system as:

- a 24-block betting game
- block-high / block-low betting
- players choosing block numbers

Use these canonical terms instead:

- `country` or `faction`
- `direction`: `Down`, `Neutral`, `Up`
- `round`: fast 60-second loop
- `epoch`: slower index settlement loop
- `active index`: the oracle-scored market that current bets feed into
- `operator doge`: the live gameplay doge locked for rounds

When in doubt, lead with what is already live and measurable, then describe what the current contract state enables next.

## Core Game Model

### Round Layer

Players place one or more `country + direction` bets during an active round.

At round end:

1. the contract randomly chooses a winning country from countries that actually received bets
2. it randomly chooses a winning direction from active directions on that country
3. payouts are split into:

- exact `country + direction` winners: main SOL + dBTC round rewards
- same-country wrong-direction bettors: consolation dBTC rewards
- winning-country stakers: staking reward share
- motherlode jackpot: extra dBTC for exact winners when hit

The important implication: **round direction matters again for round rewards**, not just for epoch accounting.

### Epoch Layer

Each round bet is also accumulated into the active epoch market.

An epoch is defined by:

- `epoch_id`
- `index_id`
- `question_hash`
- `start_scores` / `start_ranks`
- `final_scores` / `final_ranks`
- per-country direction totals

Typical index families may be `Economics`, `Military`, `AI Race`, or `Space Race`, but the contract models them generically as index states instead of hardcoded product lanes.

The oracle updates index scores during the epoch. At settlement:

- each country resolves to `Down`, `Neutral`, or `Up` from rank change
- reward pools are weighted by final ranking
- users only earn on countries where they chose the correct direction

### Doge Layer

There are two distinct doge roles:

- `gameplay_doge`: one operator doge locked for live play, carrying multiplier, DNA, and XP cache
- `staked_doges`: passive staking boosts for hashpower

Current behavior:

- gameplay rounds can update cached doge XP / mutation state
- sync back into `DogeMetadata` still happens during round reward claim or gameplay withdrawal
- there is not yet an epoch-lock rule preventing immediate operator withdrawal after an epoch bet

If you change operator-doge progression, keep the separation between:

- short-term round fun / XP
- long-term epoch accuracy / progression

## Important Accounts

### Global / Shared

- `GlobalConfig`
- `GlobalGameSate`
- `MineBtcMining`
- `EpochConfig`
- `IndexState`
- `EpochState`
- `UnrefinedRewards`

### Per Country

- `FactionState`

### Per Player

- `PlayerData`
- `UserGameBet`
- `UserEpochBets`
- `AutominerVault`
- `StakedPosition`
- `DogeMetadata`

## Main File Ownership

| File | Main Responsibility |
|------|----------------------|
| `instructions/game.rs` | Round start/end, winner selection, round reward indexes |
| `instructions/user.rs` | Manual betting, batch betting, autominers, round claims, gameplay doges |
| `instructions/epoch.rs` | Index initialization, oracle score updates, epoch settlement, epoch claims |
| `instructions/stake.rs` | MineBTC and LP staking |
| `instructions/doges.rs` | Doge minting, breeding, staking |
| `instructions/economy.rs` | Price snapshots, emissions, POL |
| `instructions/tax.rs` | Transfer-tax accounting |
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
- a player can bet on the same country with different directions in the same round
- the same bet updates both `GameSession` totals and `EpochState` / `UserEpochBets`

### Round Settlement

`instructions/game.rs`

- `int_start_round`
- `int_end_round`
- `int_end_round_faction_rewards`

Important rules:

- randomness is commit-reveal based
- winner selection is `country -> direction`
- same-country, wrong-direction bettors can still earn the consolation dBTC pool
- `end_round_faction_rewards` also advances epoch mining accounting

### Epoch Settlement

`instructions/epoch.rs`

- `schedule_next_epoch_market_internal`
- `update_epoch_scores_internal`
- `settle_epoch_internal`
- `claim_epoch_rewards_internal`

Important rules:

- `EpochConfig.active_index_id` tells you which index current round bets feed into
- first-market bootstrap matters
- epoch rewards are paid from per-country correct-direction totals, not from a flat country exposure pool

### Autominers

`instructions/user.rs`

Autominer supports:

- `FactionsConfig::Specific { picks }`
- `FactionsConfig::Random { count, direction }`

Important rules:

- no block-based autominer config exists anymore
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
[b"epoch-config"]
[b"unrefined-rewards"]

// Per entity
[b"faction", faction_name.as_bytes()]
[b"player", user.key().as_ref()]
[b"game-session", &round_id.to_le_bytes()]
[b"user-bet", user.key().as_ref(), &round_id.to_le_bytes()]
[b"epoch", &epoch_id.to_le_bytes()]
[b"user-epoch", user.key().as_ref(), &epoch_id.to_le_bytes()]
[b"index-state", &[index_id]]
[b"autominer", user.key().as_ref()]
[b"autominer-custody"]
[b"doge-metadata", doge_mint.key().as_ref()]
[b"doge-custody"]
```

Important gotcha:

- `FactionState` uses the **faction name bytes**, not the numeric faction id

## Event Expectations

Indexers and off-chain systems should expect these as the key product events:

- `RoundStarted`
- `BetsPlaced`
- `RoundEnded`
- `MotherlodeHit`
- `RoundRewardsClaimed`
- `EpochMarketScheduled`
- `EpochScoresUpdated`
- `EpochSettled`
- `EpochRewardsClaimed`
- `AutominerInitialized`
- `AutominerReloaded`
- `DogeUsedForGameplay`
- `DogeSynced`

If you change gameplay semantics, update events and docs together.

## Documentation Rules

When changing this repo:

- keep README language aligned with the current contract model
- avoid reintroducing old block-betting language
- prefer describing the product as **round arena + epoch market**
- avoid making "prediction market" the primary label when "live game + intelligence data pipeline" is more precise
- if setup scripts still initialize a 24h epoch, say that clearly and point to `setup_scripts/config.json`

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
