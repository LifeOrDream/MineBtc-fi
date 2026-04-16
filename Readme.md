<p align="center">
  <a href="https://minebtc.fun">
    <img src="./docs/images/logo.png" alt="MineBTC" width="120" />
  </a>
</p>

<h1 align="center">MineBTC</h1>

<p align="center">
  <strong>Degen country arena game on Solana. Pick your country, bet SOL, your doge evolves, your country climbs, you earn dogeBTC.</strong>
</p>

<p align="center">
  <a href="https://minebtc.fun"><img src="https://img.shields.io/badge/Launch_App-ffd700?style=for-the-badge&logo=data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyNCIgaGVpZ2h0PSIyNCIgdmlld0JveD0iMCAwIDI0IDI0IiBmaWxsPSJub25lIiBzdHJva2U9ImJsYWNrIiBzdHJva2Utd2lkdGg9IjIiPjxwYXRoIGQ9Ik0xOCAxM3Y2YTIgMiAwIDAgMS0yIDJINWEyIDIgMCAwIDEtMi0yVjhhMiAyIDAgMCAxIDItMmg2Ii8+PHBvbHlsaW5lIHBvaW50cz0iMTUgMyAyMSAzIDIxIDkiLz48bGluZSB4MT0iMTAiIHkxPSIxNCIgeDI9IjIxIiB5Mj0iMyIvPjwvc3ZnPg==&logoColor=black" alt="Launch App" /></a>
  <a href="https://docs.minebtc.fun"><img src="https://img.shields.io/badge/Documentation-333?style=for-the-badge&logo=gitbook&logoColor=white" alt="Docs" /></a>
</p>

---

## How It Works

MineBTC runs two reward loops on the same bet:

- **Round loop:** 60-second arena rounds. Players pick a country and a direction (Up / Down / Neutral). A random winner is selected. Exact matches earn SOL + dogeBTC. Same-country wrong-direction gets consolation dogeBTC. Stakers earn a share. Motherlode jackpot has a 1/625 chance.

- **Rebase loop:** The same bets accumulate over an economy cycle (~4 hours). Doge mutations that fire during betting score points for the player's country. At the end of the cycle, countries are ranked by total mutation scores. Players who bet the correct direction on their own country earn dogeBTC from the rebase mining pool.

### The Doge NFT System

Doges are functional game pieces, not cosmetics:

- **Gameplay doge:** One operator doge locked for live rounds. Earns XP from betting, can mutate (Evolution / Power / Trait). Mutations boost the doge's multiplier and score points for its country on the rebase leaderboard.
- **Staked doges:** Up to 5 passive doges that boost staking hashpower for earning rewards.
- **DNA:** 256-bit genome with appearance traits, power traits, evolution stage, and breed type. Mutations upgrade traits over time.
- **Multiplier:** Ranges from 1.0x to 10.0x. Higher multiplier = more weighted points per bet = larger reward share. But mutation chance decreases as multiplier rises, creating a natural progression curve.

### The Economy

- **1% transfer tax** on all dogeBTC: burned (25%) + NFT floor sweep (10%) + faction treasury (40%) + back to mining vault (25%)
- **Economy cycle (~4h):** 8 price snapshots → emission rate adjustment → LP add + burn (permanent POL)
- **Faction treasury:** Distributed to stakers based on the rebase mutation leaderboard. 80% rank-weighted, 20% lucky draw for underdog factions.

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

See [ECONOMY.md](programs/mineBTC/ECONOMY.md) for detailed economy cycle and rebase documentation.

## Build And Verify

```bash
# Build the program
anchor build -p minebtc

# Format all Rust files
cargo fmt --all

# Check the MineBTC program
cargo check -p minebtc

# Run the MineBTC unit suite
cargo test -p minebtc --lib
```

## Security

See [SECURITY.md](SECURITY.md) for responsible disclosure guidance.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development workflow and terminology rules.
