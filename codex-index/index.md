# MineBTC Contracts Index

This index maps the contract repository for fast cross-repo work. Treat this repo as the source of truth for game rules, account layout, fee math, rounds, cycles, HashBeast NFTs, staking, referrals, and backend/frontend data contracts.

## Repository Shape

- `programs/mineBTC/src/lib.rs` - Anchor instruction surface for the MineBTC game program.
- `programs/mineBTC/src/state.rs` - canonical account structs, constants, PDA seeds, fee defaults, and protocol limits.
- `programs/mineBTC/src/events.rs` - event surface the backend should index and stream to the frontend.
- `programs/mineBTC/src/instructions/` - implementation by domain:
  - `admin.rs` - global config, factions, fees, HashBeast config, game state, custodian setup.
  - `game.rs` - round start/end, winner selection, reward distribution.
  - `user.rs` - player signup, bets, autominers, claims, gameplay HashBeast locking/unlocking, story events.
  - `stake.rs` - degenBTC/LP staking, reward claims, HODL tax, referral claims.
  - `hashbeasts.rs` - HashBeast minting, whitelist/admin minting, staking, breeding, and rebirth.
  - `economy.rs` - SOL fee distribution (buybacks + NFT MM + dev), price snapshots, emission rate update, LP add/burn.
  - `marketplace_cpi.rs` - permissionless on-chain NFT market maker: floor queue, sale history, floor history, sweep + auto-dispose, expire program listing, register/list/cancel/buy wrappers.
  - `faction_war.rs` - mutation-driven cycle scoring, rankings, settlement, cycle claims.
  - `tax.rs` - Token-2022 withheld fee harvesting and distribution (faction_treasury_pct + burn_pct + residual to mining vault; no NFT floor sweep slice).
  - `helper.rs` - PDA helpers, transfers, reward math, staking math.
- `raydium/programs/cp-swap/` - bundled/custom Raydium CP-Swap program used for local/devnet pool flows.
- `setup_scripts/` - deployment, token init, pool init, game init, local testing, and keeper loops.

## Primary Sub-Indexes

- [Accounts and PDAs](accounts-and-pdas.md)
- [Instruction Surface](instruction-surface.md)
- [Deployment and Initialization](deployment-and-initialization.md)
- [Events and Indexing](events-and-indexing.md)
- [Economy and Rewards](economy-and-rewards.md)
- [Frontend and Backend Data Implications](frontend-backend-data.md)

## Current Program Identity

- Anchor program: `minebtc`
- Declared program ID in `programs/mineBTC/src/lib.rs`: `DPfSfuStn4cU1p4G7PTcqDiWdufGg9kpJPrsnatG6SLG`
- Companion Anchor program: `degenbtc_market` (standalone NFT marketplace; mineBTC CPIs into it for floor sweep / auto-dispose)
- Token: Token-2022 `degenBTC` / `dBTC`, 6 decimals, 2.1B fixed supply, transfer tax default 10 bps.
- Default round duration from setup config: 60 seconds.
- Factions in setup config: USA, China, Russia, India, Japan, South Korea, Iran, UK, North Korea, France, Brazil, Israel. Contract supports up to 15.

## Source-Of-Truth Rules For Later Phases

- Use `state.rs` constants/defaults over docs/frontend/backend assumptions.
- Use `events.rs` for backend indexing and socket emission design.
- Use `setup_scripts/3_init_mineBTC.js` for canonical initialization order and live config values.
- Do not assume existing docs/backend/frontend are current if they disagree with contract code.
- No backward compatibility is required during launch-prep changes; stale mock/data compatibility can be removed.
