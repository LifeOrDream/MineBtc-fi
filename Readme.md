<p align="center">
  <a href="https://minebtc.fun">
    <img src="./docs/images/logo.png" alt="MineBTC" width="128" />
  </a>
</p>

<h1 align="center">MineBTC</h1>

<p align="center">
  <strong>Casino-speed country wars for mining degenBTC on Solana.</strong><br/>
  Pick a country, bet SOL every minute, mutate HashBeasts through claims, and fight for the LP-burn faction-war crown.
</p>

<p align="center">
  <a href="https://minebtc.fun"><img src="https://img.shields.io/badge/Launch_App-ffd700?style=for-the-badge&logo=data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyNCIgaGVpZ2h0PSIyNCIgdmlld0JveD0iMCAwIDI0IDI0IiBmaWxsPSJub25lIiBzdHJva2U9ImJsYWNrIiBzdHJva2Utd2lkdGg9IjIiPjxwYXRoIGQ9Ik0xOCAxM3Y2YTIgMiAwIDAgMS0yIDJINWEyIDIgMCAwIDEtMi0yVjhhMiAyIDAgMCAxIDItMmg2Ii8+PHBvbHlsaW5lIHBvaW50cz0iMTUgMyAyMSAzIDIxIDkiLz48bGluZSB4MT0iMTAiIHkxPSIxNCIgeDI9IjIxIiB5Mj0iMyIvPjwvc3ZnPg==&logoColor=black" alt="Launch App" /></a>
  <a href="https://docs.minebtc.fun"><img src="https://img.shields.io/badge/Docs-111111?style=for-the-badge&logo=gitbook&logoColor=white" alt="Docs" /></a>
  <img src="https://img.shields.io/badge/Solana-14F195?style=for-the-badge&logo=solana&logoColor=111111" alt="Solana" />
  <img src="https://img.shields.io/badge/Anchor-512DA8?style=for-the-badge&logo=rust&logoColor=white" alt="Anchor" />
</p>

---

MineBTC is an on-chain game economy where every bet is also a prediction, every claim can move an NFT, and every economy cycle feeds back into token emissions, liquidity, country rankings, and NFT floor support.

The clean one-liner:

> **A Solana casino x prediction-market x creature-progression game where countries compete to mine degenBTC.**

## Read This First

This repo contains the contracts for the MineBTC game economy.

| Doc | What it covers |
|---|---|
| [docs/GAMEPLAY.md](./docs/GAMEPLAY.md) | 60-second rounds, country directions, jackpots, autominers, faction-war cycles, reward claims, and mutation rolls. |
| [docs/ECONOMY.md](./docs/ECONOMY.md) | degenBTC emissions, SOL routing, buybacks, protocol-owned liquidity, transfer tax, staking, HODL tax, and market-making funding. |
| [docs/NFTS.md](./docs/NFTS.md) | HashBeast DNA, mutation, breeding, rebirth, lootboxes, marketplace wrapping, floor anchor, sweeps, relists, and burns. |

Primary source files:

| Area | Source |
|---|---|
| State, constants, account layouts | `programs/mineBTC/src/state.rs` |
| 60-second rounds | `programs/mineBTC/src/instructions/game.rs` |
| Player bets, claims, autominers, mutations | `programs/mineBTC/src/instructions/user.rs` |
| LP-burn faction-war cycles | `programs/mineBTC/src/instructions/faction_war.rs` |
| Macro economy, buybacks, LP burn | `programs/mineBTC/src/instructions/economy.rs` |
| HashBeast lifecycle | `programs/mineBTC/src/instructions/hashbeasts.rs` |
| NFT marketplace and floor support | `programs/mineBTC/src/instructions/marketplace_cpi.rs` |
| Token transfer tax | `programs/mineBTC/src/instructions/tax.rs` |

## The Protocol In One Screen

| Layer | Player-facing fantasy | Contract reality |
|---|---|---|
| **Rounds** | One-minute country battles. | SOL bets resolve by slot-hash entropy into country + direction winners. |
| **Faction wars** | Communities push countries up the leaderboard. | Many rounds fold into rank-weighted base, HashBeast, and MVP reward lanes. |
| **degenBTC** | Bitcoin as a degenerate mined game token. | Fixed-supply Token-2022 asset emitted through gameplay and throttled by price snapshots. |
| **HashBeasts** | Dynamic operators that evolve from wins. | Metaplex Core assets with game-state PDAs: DNA, XP, multiplier, breed count, rebirth count. |
| **Market maker** | Protocol defends floor and restocks lootboxes. | Permissionless sweeps buy cheap listings, then queue, relist, or burn inventory. |

## One Bet, Five Effects

| Effect | What happens |
|---|---|
| **Minute outcome** | The bet can win the 60-second SOL + dBTC round. |
| **Cycle prediction** | The same country/direction exposure counts toward the active faction war. |
| **Country score** | Winning country weighted points move the leaderboard. |
| **NFT progression** | Claiming eligible rewards can mutate the active HashBeast. |
| **Economy routing** | SOL volume feeds stakers, treasury, buybacks, POL, and NFT market-making. |

```text
Player bet
  -> 60s round pot
  -> faction-war exposure
  -> SOL fee router
  -> claim-time HashBeast roll
  -> economy cycle and NFT floor support
```

## Core Game Loop

| Step | User sees | On-chain state moves |
|---:|---|---|
| 1 | Pick country + Up/Neutral/Down. | `UserGameBet` and round aggregate counters update. |
| 2 | Round resolves after 60 seconds. | `end_round` locks entropy and result, then `settle_round` finalizes indexes. |
| 3 | Claim rewards. | dBTC/SOL rewards transfer, HODL accounting syncs, HashBeast roll can fire. |
| 4 | Country climbs or falls. | Round scores and mutation scores fold into `FactionWarState`. |
| 5 | Cycle settles after economy LP burn. | `settle_war` pays base, HashBeast, MVP, and SOL mirror lanes. |
| 6 | Economy adjusts. | Price snapshots update emissions; SOL buybacks add and burn LP. |

## Why The Design Is Interesting

MineBTC is not a normal "stake token, earn token" loop.

It is a competitive distribution machine:

| Traditional GameFi failure | MineBTC design response |
|---|---|
| Token emissions detached from real demand. | dBTC is mined through gameplay volume and adjusted by price snapshots. |
| Static NFTs become pure speculation. | HashBeasts mutate, multiply bets, boost staking, breed, rebirth, enter lootboxes, and can be burned. |
| Floor collapses break mint economics. | Treasury-funded market maker buys cheap inventory under floor-anchor guardrails. |
| Passive farms favor mercenary capital. | Countries, autominers, jackpots, and MVP lanes create social competition and repeated play. |
| Rewards become one-dimensional. | Users earn across rounds, faction wars, staking, jackpot, HODL tax, and NFT progression. |

## Current Setup Values

These are deployment setup values, not governance promises forever.

| Parameter | Value |
|---|---:|
| Round duration | 60 seconds |
| Countries | 12 |
| degenBTC supply | 2.1B fixed supply |
| degenBTC decimals | 6 |
| Transfer tax | 0.1% |
| Base round emission | 1,000 dBTC |
| Jackpot chance | 1 / 625 |
| Faction-war cycle | One economy LP-burn cycle |
| Production cycle target | 8 snapshots x 30 minutes = about 4 hours |
| Current devnet setup | 8 snapshots x 5 minutes = about 40 minutes |
| Protocol fee on SOL bets | 15% |
| Cycle SOL split | 5% of gross bet |
| Treasury buyback share | 70% |
| NFT market-making share | 3% |
| Gameplay HODL tax | 10% |
| Genesis HashBeast cap | 36,000 |
| Genesis cap per country | 3,000 |
| Base genesis mint price | 1 SOL |
| Gameplay HashBeast multiplier cap | 4.2x |
| Passive HashBeast staking cap | 3x |
| Breed floor guard | At least 1.5x floor anchor |
| Max rebirth count | 7 |

## Repository Map

```text
programs/
  mineBTC/              Main game, economy, HashBeast, staking, tax contracts
  degenbtc_market/      Standalone HashBeast marketplace used by MineBTC
raydium/
  programs/cp-swap/     Local/devnet Raydium CP-Swap program
setup_scripts/          Deployment, initialization, keeper, and devnet scripts
tests/                  Anchor / TypeScript tests
```

## Build And Verify

```bash
cargo fmt
cargo check -p minebtc
cargo check -p degenbtc_market
cargo test -p minebtc
cargo test -p degenbtc_market --lib
anchor build
```

`anchor build` can emit an upstream `mpl_core::hooked::plugin::registry_records_to_plugin_list` SBF stack warning. Treat that as a mainnet-readiness item to watch, not a docs issue.

## Production Mindset

The contracts are built as permissionless public state machines:

| Permissionless action | Why it exists |
|---|---|
| Start/end/settle rounds | Keeps the 60-second game alive without an admin operator. |
| Settle faction wars | Lets anyone close the cycle once the economy boundary is reached. |
| Run economy snapshots / LP burns | Keeps emissions and POL moving. |
| Register listings / sweep floor / expire inventory | Keeps the NFT floor queue and inventory system live. |
| Claim lootbox NFTs for users | Lets bots deliver assets while the recipient stays fixed on-chain. |

The safety posture is adversarial by default:

- canonical PDA constraints for vaults, queues, metadata, and state;
- checked arithmetic for reward indexes and pool math;
- rent-aware native SOL transfers;
- no privileged NFT market-maker cranker;
- marketplace listing and collection binding before floor entries are trusted;
- sweep limits by floor anchor, vault percentage, reserve, and stale-data windows;
- user recipients fixed by PDA/account constraints.

MineBTC is an entertainment protocol and on-chain game. It is not investment advice, financial advice, or a real-world geopolitical oracle.
