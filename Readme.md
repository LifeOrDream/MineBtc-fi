<p align="center">
  <a href="https://minebtc.fun">
    <img src="./docs/images/logo.png" alt="MineBTC" width="120" />
  </a>
</p>

<h1 align="center">MineBTC</h1>

<p align="center">
  <strong>Degen country arena game on Solana.<br/>Pick your country. Bet SOL. Your doge evolves. Your country climbs. You earn dogeBTC.</strong>
</p>

<p align="center">
  <a href="https://minebtc.fun"><img src="https://img.shields.io/badge/Launch_App-ffd700?style=for-the-badge&logo=data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyNCIgaGVpZ2h0PSIyNCIgdmlld0JveD0iMCAwIDI0IDI0IiBmaWxsPSJub25lIiBzdHJva2U9ImJsYWNrIiBzdHJva2Utd2lkdGg9IjIiPjxwYXRoIGQ9Ik0xOCAxM3Y2YTIgMiAwIDAgMS0yIDJINWEyIDIgMCAwIDEtMi0yVjhhMiAyIDAgMCAxIDItMmg2Ii8+PHBvbHlsaW5lIHBvaW50cz0iMTUgMyAyMSAzIDIxIDkiLz48bGluZSB4MT0iMTAiIHkxPSIxNCIgeDI9IjIxIiB5Mj0iMyIvPjwvc3ZnPg==&logoColor=black" alt="Launch App" /></a>
  <a href="https://docs.minebtc.fun"><img src="https://img.shields.io/badge/Documentation-333?style=for-the-badge&logo=gitbook&logoColor=white" alt="Docs" /></a>
</p>

---

## The Game

MineBTC is a country arena game where every bet does three things at once:

1. **Enters a 60-second round raffle** for instant SOL + dogeBTC rewards
2. **Scores points for your country** on the competitive leaderboard
3. **Can trigger your doge NFT to mutate**, permanently upgrading its stats

Countries compete for the top of the leaderboard. Players compete for rewards. Doges evolve through gameplay. The economy self-sustains through deflationary tokenomics and permanent liquidity locks.

---

## Two Reward Loops, One Bet

### Round Loop (60 seconds)

Every minute, a new round runs:

1. Players place `country + direction` bets (Up / Down / Neutral)
2. A random winning country and direction are selected via slot-hash entropy
3. Rewards are distributed:

| Pool | Share | Who Gets It |
|------|-------|-------------|
| **Winner pool** | 50% of dogeBTC emission | Exact country+direction match (pro-rata) |
| **Consolation pool** | 40% of dogeBTC emission | Same country, wrong direction (split per direction) |
| **Staker pool** | 5% of dogeBTC emission | Everyone staking on the winning country |
| **Motherlode** | 5% of dogeBTC emission | 1/625 chance jackpot for exact winners |
| **SOL prize pot** | Accumulated from net bets | Exact winners split proportionally |

### Rebase Loop (~4 hours, tied to economy cycle)

The same bets also accumulate into a longer competitive cycle called a **rebase**:

1. Doge mutations that fire during rounds **score points for their country**
2. At the end of the cycle, countries are ranked by total mutation scores
3. Rankings are compared to the previous cycle to determine which countries moved Up, Down, or stayed Neutral
4. Players who correctly bet the direction of their **own country** earn dogeBTC from the rebase mining pool
5. Only own-country bets count -- you must be loyal to earn

**Mutation score formula:**
```
score = type_weight × bet_size × doge_multiplier
        Evolution=100, Power=30, Trait=10
```

Higher bets + better doges = bigger score contribution to your country.

---

## Doge NFTs: The Progression Engine

Doges are functional game pieces with on-chain 256-bit DNA:

### Two Doge Roles

- **Gameplay doge (operator):** One doge locked for active play. Earns XP from betting. Can mutate (Evolution / Power / Trait). Mutations upgrade stats and score points for the country leaderboard.
- **Staked doges (passive):** Up to 5 doges boosting staking hashpower. More staked doges = higher staking APR.

### How Mutations Work

Every SOL bet with a gameplay doge rolls for a mutation:

```
Base chance: 20%
× bet_strength (your bet / highest bet on your country)
× multiplier_penalty (1.0x doge = full chance, 10.0x = 10% chance)
× faction_penalty (each prior mutation this round makes the next harder)

Global cap: max mutations per round = active_factions / 3
```

**Mutation types:**
- **Evolution** (~10%): Stage upgrade, guaranteed visual + power trait gains, XP resets. Rarest and most impactful.
- **Power** (~30%): Combat trait upgrade, moderate multiplier boost.
- **Trait** (~60%): Visual trait upgrade, small multiplier boost.

**Multiplier range:** 1.0x → 10.0x. Higher multiplier = more weighted points per bet = bigger reward share. But mutation chance drops as multiplier rises, creating a weeks-long progression curve.

### XP System

XP accumulates from SOL bets and boosts the multiplier increase when a mutation fires:

```
XP gain rate = base_rate × (1.0 / current_multiplier)
```

A fresh doge gains XP fast. A maxed doge gains XP slowly. This prevents whales from speed-running progression.

When a mutation fires, it **consumes** the XP it used:
- Evolution: consumes ALL XP (full reset)
- Power/Trait: consumes the portion used for the multiplier boost

### Accumulated Value

Each round, the gameplay doge earns dogeBTC based on mutation type (1% - 6.9% of round reward). This accumulates on-chain and can only be claimed by **burning the doge** (`send_to_heaven`). Creates a natural floor price based on accumulated earnings.

---

## The Economy

### dogeBTC Token

dogeBTC is a Token-2022 token with a 0.1% transfer tax. Every transfer automatically splits:

| Split | Default % | Where It Goes |
|-------|-----------|---------------|
| **Burn** | 50% | Permanently removed from supply |
| **Faction Treasury** | 25% | Distributed to stakers via faction-war settlement |
| **NFT Floor Sweep** | 25% | Funds NFT market-making operations |
| **Back to Vault** | 0% | Recycled into mining emission pool when config leaves remainder |

### Economy Cycle (~4 hours)

The economy runs in automated loops:

```
Step 1: snapshot_price (×8, every 30 min)
        → Swaps 10% of buyback SOL for dogeBTC (price discovery)
        → Earmarks 10% for Protocol Owned Liquidity

Step 2: update_rate (after 8 snapshots)
        → Compares weighted avg price to baseline
        → Price up → increase emission rate (1%)
        → Price down → decrease emission rate (3%)

Step 3: add_lp_and_burn
        → Deposits earmarked SOL + dogeBTC into Raydium LP
        → Burns ALL LP tokens (permanent liquidity lock)
        → Triggers rebase settlement
```

The asymmetric rate adjustment (1% up / 3% down) creates structural deflationary pressure during downturns.

### SOL Fee Flow

```
Player bets 1 SOL
├─ 15% fee taken
│   ├─ 20% of fee → staker SOL reward vault
│   └─ 80% of fee → SOL treasury → buybacks (80%) + dev (20%)
└─ 85% net → SOL prize pot (for round winners)
```

### Faction Treasury (Tax Rewards to Stakers)

After each rebase settles, the accumulated faction treasury is distributed to stakers:

- **80% rank-weighted:** Every country gets something. Higher leaderboard rank = bigger share.
- **20% lucky draw:** One random underdog country (rank 5+) wins the entire pot. Equal probability, keeps small factions engaged.

---

## Staking

Two staking tracks, both earning SOL + dogeBTC:

| Track | What You Stake | What Boosts Rewards |
|-------|---------------|-------------------|
| **dogeBTC staking** | Lock dogeBTC for configurable duration | Longer lockup = higher multiplier. Staked doges boost hashpower. |
| **LP staking** | Lock Raydium LP tokens | Same multiplier mechanics as dogeBTC staking |

Stakers earn from three sources:
1. **SOL fees** from every bet (staker share)
2. **dogeBTC emission** from round staker pools (winning faction only)
3. **Faction treasury** from transfer tax (based on rebase leaderboard rank)

---

## AI Integration (Planned)

The game generates rich, structured on-chain data with every bet, mutation, and rebase:

- Country-level directional conviction weighted by real money
- NFT evolution histories (256-bit DNA trajectories over time)
- Player behavior patterns (faction loyalty, bet sizing, mutation strategies)

This data can later power:

- **AI-generated doge art** — unique visuals for each evolution stage, faction-specific styles
- **NFT market-making agent** — autonomous floor sweeps, pricing, inventory rotation using the NFT floor sweep vault
- **Content generation** — mutation stories, faction propaganda, social clips
- **Game expansion** — AI-designed mini-games, new round modes, mobile experiences

The game is self-contained today. AI integration adds value around the game without being a dependency for core gameplay.

---

## Repo Map

```text
programs/mineBTC/src/
├── lib.rs              # Program entrypoints
├── state.rs            # Account layouts and constants
├── errors.rs           # Custom error codes
├── events.rs           # Indexer-facing events
├── genescience.rs      # Doge DNA, mutations, evolution, breeding
└── instructions/
    ├── admin.rs        # Global config, factions, fee parameters
    ├── game.rs         # 60-second round loop, slot-hash randomness, winner selection
    ├── user.rs         # Betting, autominers, round claims, gameplay doges, mutations
    ├── rebase.rs       # Mutation-driven competitive cycles, settlement, rebase claims
    ├── stake.rs        # dogeBTC and LP token staking
    ├── doges.rs        # Doge NFT minting, breeding, staking, gameplay lock/unlock
    ├── economy.rs      # Price snapshots, emission rate adjustment, POL (LP add + burn)
    ├── tax.rs          # Transfer-tax harvest, faction treasury distribution
    └── helper.rs       # Shared math and vault transfer helpers
```

**Documentation:**
- [ECONOMY.md](programs/mineBTC/ECONOMY.md) — Detailed economy cycle, rebase mechanics, XP/mutation math
- [STAKING.md](programs/mineBTC/STAKING.md) — Staking mechanics, hashpower, reward indexes
- [CLAUDE.md](programs/mineBTC/claude.md) — Developer orientation guide and canonical terminology

## Build And Verify

```bash
anchor build -p minebtc
cargo fmt --all
cargo check -p minebtc
cargo test -p minebtc --lib
```

## Security

See [SECURITY.md](SECURITY.md) for responsible disclosure guidance.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development workflow and terminology rules.
