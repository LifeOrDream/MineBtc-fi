# MineBTC Program Guide

This file is the source-of-truth orientation note for anyone editing the `mineBTC` Anchor program.

## Product Framing

MineBTC is a **degen country arena game** on Solana where:

- players pick a country and a direction, bet SOL, and winning claims can evolve their hashbeast NFTs
- own-country gameplay support decides which country climbs the leaderboard each cycle
- the same bet powers both the **round raffle** (instant SOL + degenBTC rewards) and the **cycle leaderboard** (longer-term degenBTC rewards based on which countries moved)
- a deflationary economy runs on a 0.1% transfer tax: burn + faction treasury + mining vault recycle (no NFT floor sweep slice — NFT market making is funded from SOL via `distribute_sol_fees::nft_market_making_pct`, default 3%)
- an automated economy cycle (price snapshots → rate adjustment → LP burn) keeps tokenomics self-sustaining

**The game in one sentence:** "Pick your country, bet SOL, win claims, your hashbeast evolves, your country climbs, you earn degenBTC."

Player country is permanent after signup. Referral rewards are also
country-aware: referred users always get the same 1% degenBTC claim bonus, while
referrers earn a higher degenBTC reward when they recruit someone into their own
country. This keeps the growth loop simple: "bring people to your flag."

Use these canonical terms:

- `country` or `faction` — one of 12-15 playable nations
- `direction` — `Down`, `Neutral`, `Up`
- `round` — fast 60-second betting loop with random winner
- `faction war` — longer competitive period tied to the economy cycle (LP burn cadence), where gameplay scores determine country rankings
- `operator hashbeast` / `gameplay hashbeast` — the live NFT locked for rounds, contributes gameplay score, earns XP from eligible claim rolls, can mutate
- `staked hashbeasts` — passive NFTs that boost staking hashpower
- `story event` — a claim-time HashBeast event (Evolution / Power / Trait); the contract may mutate DNA internally, but the backend decides how to render it
- `gameplay score` — contribution to your country's leaderboard rank from own-country SOL support with an active gameplay hashbeast

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

Each round bet also accumulates into the active cycle. Own-country SOL bets from users with an active gameplay hashbeast contribute deterministic gameplay score to their faction.

A cycle is defined by:

- `faction_war_id`
- `start_ranks` (from previous cycle)
- `faction_gameplay_scores` (internal field; accumulated gameplay scores during this cycle)
- per-country direction totals, plus separate own-country loyalty totals

**How cycles work:**

1. Cycle auto-starts on first bet after the previous cycle settles
2. Each eligible own-country SOL bet adds score: `GAMEPLAY_SUPPORT_SCORE_WEIGHT × bet_size × hashbeast_multiplier`
3. Cycle settles when the economy cycle's LP burn completes
4. Factions are ranked by total gameplay scores, with round wins and SOL support as tiebreakers
5. Rank changes resolve each country's winning direction
6. Players who bet correct directions earn degenBTC via the base pool; own-country correct bettors also share the loyalty pool

**When RPG progression is disabled** (`rpg_progression` off), cycle gameplay scoring and mutation rolls pause.

### HashBeast Layer

Two distinct hashbeast roles:

- `gameplay_hashbeast`: one operator hashbeast locked for live play, carries multiplier, DNA, XP cache
- `staked_hashbeasts`: up to 3 passive boosts for staking hashpower

**Mutation/story event system:**

- Story events trigger during winning round or faction-war reward claims (SOL stake only, requires gameplay hashbeast)
- Round claim odds use the winning faction stake; exact wins get stronger odds than same-faction consolation wins
- Faction-war claim odds are strongest for own-country correct calls, especially when the country moved Up
- Round-level mutation counters still create scarcity and pacing pressure
- Per-faction difficulty scaling: each event in a round makes the next one harder for that faction
- Base chance is configurable, reduced by HashBeast multiplier (high-mult HashBeasts trigger less often)
- Types: Evolution (~10%), Power (~30%), Trait (~60%)
- XP boosts multiplier on story events: Evolution 5-10% of XP, Power/Trait 2-5% of XP
- active_multiplier capped at `GAMEPLAY_MAX_MULTIPLIER` (4.2x)
- 2-step gameplay hashbeast unlock prevents mid-cycle withdrawal gaming

**HashBeast mint supply:**

- `HashBeastConfig` owns non-sale state: collection, total minted count, and breeding config
- `HashBeastMintConfig` owns genesis-sale-only state: bonding curve price, ticket tiers, genesis mint count, and per-country caps
- Genesis mints are capped at the configured genesis_mint_limit (currently 36,000, 3,000 max per country across 12 countries); there is no lifetime supply cap — post-genesis breeding is governed by an admin flag plus a bonding-curve price (max(curve, 1.5× current floor anchor)) that scales with `total_hashbeasts_minted`
- `rebirth_hashbeast` pays the HashBeast's locked degenBTC to the owner, then either rebirths the NFT into lootbox inventory or burns it if the queue/inventory is full or the NFT already reached `MAX_REBIRTH_COUNT`
- Rebirth increments `HashBeastMetadata.rebirth_count`, writes the same 0-7 value into DNA bits at offset 174, rerolls fresh DNA, and resets multiplier, XP, breed count, cooldown, accumulated value, gameplay lock, and parent lineage
- Market-maker sweeps/expiry cascades that push NFTs into lootboxes do not rebirth or reset those NFTs; they preserve existing DNA/stats
- Burns/rebirths do not reduce `total_hashbeasts_minted`; post-genesis breeding is governed by breed-count limits, same-rebirth-level pairing, and the breeding bonding curve, not a lifetime cap field
- `breed_hashbeasts` is blocked until the genesis sale is sold out (`genesis_mints >= genesis_mint_limit`); after that it prices every birth at `max(breeding_curve, 1.5x current floor anchor)`, charges 50% SOL + 50% degenBTC by SOL value, sends SOL 25% to `fee_recipient` / 75% to `sol_treasury`, and splits degenBTC 50% burn / 50% back to the mining vault

### Economy Layer

- 0.1% transfer tax on all degenBTC: split between burn (50%), faction treasury (25%), and mining-vault recycle (residual 25%); no NFT floor sweep slice
- NFT market making is SOL-funded — `distribute_sol_fees` peels off `nft_market_making_pct` (default 3%) into `inventory_sweep_vault`. Permissionless on-chain market maker handles floor sweeps, auto-disposition (queue/relist/burn), and keeper rewards
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
- `HashBeastMetadata`

## Main File Ownership

| File | Main Responsibility |
|------|----------------------|
| `instructions/game.rs` | Round start/end, winner selection, round reward indexes |
| `instructions/user.rs` | Betting, autominers, round claims, gameplay hashbeasts, story events |
| `instructions/faction_war.rs` | Cycle config, gameplay-score settlement, cycle claims |
| `instructions/stake.rs` | degenBTC and LP staking |
| `instructions/hashbeasts.rs` | HashBeast minting, breeding, staking, gameplay lock/unlock, rebirth |
| `instructions/economy.rs` | Price snapshots, emissions, POL, SOL fee distribution |
| `instructions/marketplace_cpi.rs` | Permissionless on-chain NFT market maker (floor queue, sweep + auto-dispose, expire), CPIs to `degenbtc_market` |
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
- active gameplay HashBeasts on own-country SOL bets add gameplay score during bet processing
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
[b"hashbeast-metadata", hashbeast_mint.key().as_ref()]
[b"hashbeast-custody"]
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
- `HashBeastUsedForGameplay`
- `HashBeastSynced`

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
