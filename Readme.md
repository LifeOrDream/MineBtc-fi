<p align="center">
  <a href="https://minebtc.fun">
    <img src="./docs/images/logo.png" alt="MineBTC" width="120" />
  </a>
</p>

<h1 align="center">MineBTC</h1>

<p align="center">
  <strong>Degen country arena game on Solana.<br/>Pick your country. Bet SOL. Win claims mutate your hashbeast. Your country climbs. You earn degenBTC.</strong>
</p>

<p align="center">
  <a href="https://minebtc.fun"><img src="https://img.shields.io/badge/Launch_App-ffd700?style=for-the-badge&logo=data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyNCIgaGVpZ2h0PSIyNCIgdmlld0JveD0iMCAwIDI0IDI0IiBmaWxsPSJub25lIiBzdHJva2U9ImJsYWNrIiBzdHJva2Utd2lkdGg9IjIiPjxwYXRoIGQ9Ik0xOCAxM3Y2YTIgMiAwIDAgMS0yIDJINWEyIDIgMCAwIDEtMi0yVjhhMiAyIDAgMCAxIDItMmg2Ii8+PHBvbHlsaW5lIHBvaW50cz0iMTUgMyAyMSAzIDIxIDkiLz48bGluZSB4MT0iMTAiIHkxPSIxNCIgeDI9IjIxIiB5Mj0iMyIvPjwvc3ZnPg==&logoColor=black" alt="Launch App" /></a>
  <a href="https://docs.minebtc.fun"><img src="https://img.shields.io/badge/Documentation-333?style=for-the-badge&logo=gitbook&logoColor=white" alt="Docs" /></a>
</p>

---

## The Game

MineBTC is a country arena game where every bet does three things at once:

1. **Enters a 60-second round raffle** for instant SOL + degenBTC rewards
2. **Scores gameplay support for your country** on the competitive leaderboard
3. **Sets up claim-time hashbeast mutation rolls** when that bet later wins rewards

Countries compete for the top of the leaderboard. Players compete for rewards. HashBeasts evolve through gameplay. The economy self-sustains through deflationary tokenomics and permanent liquidity locks.

---

## Two Reward Loops, One Bet

### Round Loop (60 seconds)

Every minute, a new round runs:

1. Players place `country + direction` bets (Up / Down / Neutral)
2. A random winning country and direction are selected via slot-hash entropy
3. Rewards are distributed:

| Pool | Share | Who Gets It |
|------|-------|-------------|
| **Winner pool** | 50% of degenBTC emission | Exact country+direction match (pro-rata) |
| **Consolation pool** | 40% of degenBTC emission | Same country, wrong direction (split per direction) |
| **Staker pool** | 5% of degenBTC emission | Everyone staking on the winning country |
| **Motherlode** | 5% of degenBTC emission | 1/625 chance jackpot for exact winners |
| **SOL prize pot** | Accumulated from net bets | Exact winners split proportionally |

### Rebase Loop (~4 hours, tied to economy cycle)

The same bets also accumulate into a longer competitive cycle called a **rebase**:

1. Own-country SOL bets with an active gameplay hashbeast **score gameplay support for that country**
2. At the end of the cycle, countries are ranked by total gameplay scores
3. Rankings are compared to the previous cycle to determine which countries moved Up, Down, or stayed Neutral
4. Players who correctly bet final directions earn degenBTC from the rebase mining pool
5. Own-country correct bettors get the loyalty share and the strongest hashbeast mutation odds

**Gameplay score formula:**
```
score = support_weight × own_country_sol_bet × hashbeast_multiplier
        support_weight=10
```

Higher own-country bets + better gameplay hashbeasts = bigger score contribution to your country.

---

## HashBeast NFTs: The Progression Engine

HashBeasts are functional game pieces with on-chain 256-bit DNA:

### Two HashBeast Roles

- **Gameplay hashbeast (operator):** One hashbeast locked for active play. Earns XP from betting. Own-country bets add gameplay score, and winning reward claims can mutate it (Evolution / Power / Trait).
- **Staked hashbeasts (passive):** Up to 5 hashbeasts boosting staking hashpower. More staked hashbeasts = higher staking APR.

### How Mutations Work

Mutation rolls happen when a user claims rewards from a winning round or settled rebase:

```
Base chance: 20%
× stake_strength (eligible winning stake / highest stake on the country)
× multiplier_penalty (1.0x hashbeast = full chance, 4.2x ~= 24% chance)
× faction_penalty / pacing / volume controls
× claim_boost (highest for own-country correct Up moves)

Round exact wins receive stronger odds than same-country consolation wins. Rebase claims are strongest when the user backed their own country correctly, especially when that country moved Up.
```

**Mutation types:**
- **Evolution** (~10%): Stage upgrade, guaranteed visual + power trait gains, XP resets. Rarest and most impactful.
- **Power** (~30%): Combat trait upgrade, moderate multiplier boost.
- **Trait** (~60%): Visual trait upgrade, small multiplier boost.

**Multiplier range:** 1.0x → 4.2x. Higher multiplier = more weighted points per bet = bigger reward share. But mutation chance drops as multiplier rises, creating a weeks-long progression curve.

### XP System

XP accumulates from eligible claim-time mutation stake and boosts the multiplier increase when a mutation fires:

```
XP gain rate = base_rate × (1.0 / current_multiplier)
```

A fresh hashbeast gains XP fast. A maxed hashbeast gains XP slowly. This prevents whales from speed-running progression.

When a mutation fires, it **consumes** the XP it used:
- Evolution: consumes ALL XP (full reset)
- Power/Trait: consumes the portion used for the multiplier boost

### Accumulated Value

Each successful reward claim can add degenBTC to the gameplay hashbeast based on the claim mutation result (1% - 6.9% of round reward, plus cycle HashBeast bonus where applicable). This accumulates on-chain and can be claimed through `rebirth_hashbeast`, which either rebirths the asset into lootbox inventory or burns it if the rebirth cap or inventory guardrails are hit.

---

## The Economy

### degenBTC Token

degenBTC is a Token-2022 token with a 0.1% transfer tax. Every transfer automatically splits:

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
        → Swaps 10% of buyback SOL for degenBTC (price discovery)
        → Earmarks 10% for Protocol Owned Liquidity

Step 2: update_rate (after 8 snapshots)
        → Compares weighted avg price to baseline
        → Price up → increase emission rate (1%)
        → Price down → decrease emission rate (3%)

Step 3: add_lp_and_burn
        → Deposits earmarked SOL + degenBTC into Raydium LP
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

Two staking tracks, both earning SOL + degenBTC:

| Track | What You Stake | What Boosts Rewards |
|-------|---------------|-------------------|
| **degenBTC staking** | Lock degenBTC for configurable duration | Longer lockup = higher multiplier. Staked hashbeasts boost hashpower. |
| **LP staking** | Lock Raydium LP tokens | Same multiplier mechanics as degenBTC staking |

Stakers earn from three sources:
1. **SOL fees** from every bet (staker share)
2. **degenBTC emission** from round staker pools (winning faction only)
3. **Faction treasury** from transfer tax (based on rebase leaderboard rank)

---

## AI Integration (Planned)

The game generates rich, structured on-chain data with every bet, mutation, and rebase:

- Country-level directional conviction weighted by real money
- NFT evolution histories (256-bit DNA trajectories over time)
- Player behavior patterns (faction loyalty, bet sizing, mutation strategies)

This data can later power:

- **AI-generated hashbeast art** — unique visuals for each evolution stage, faction-specific styles
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
├── genescience.rs      # HashBeast DNA, mutations, evolution, breeding
└── instructions/
    ├── admin.rs        # Global config, factions, fee parameters
    ├── game.rs         # 60-second round loop, slot-hash randomness, winner selection
    ├── user.rs         # Betting, autominers, round claims, gameplay hashbeasts, mutations
    ├── rebase.rs       # Mutation-driven competitive cycles, settlement, rebase claims
    ├── stake.rs        # degenBTC and LP token staking
    ├── hashbeasts.rs        # HashBeast NFT minting, breeding, staking, gameplay lock/unlock
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
