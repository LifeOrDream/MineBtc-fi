# Deployment and Initialization

Canonical source: `setup_scripts/` and especially `setup_scripts/3_init_mineBTC.js`.

## Main Setup Order

1. `0_deploy_game.js`
   - Builds/deploys the Anchor `minebtc` program.
   - Extracts/writes IDL to `target/idl/minebtc.json`.
   - Writes deployment state under `setup_scripts/deployments/{cluster}.json`.

2. `0_deploy_raydium.js`
   - Deploys bundled/custom Raydium CP-Swap when not using official Raydium.

3. `1_init_degenbtc_token.js`
   - Creates Token-2022 DegenBTC mint.
   - Initializes metadata pointer and transfer fee config.
   - Mints fixed initial supply.
   - Removes mint authority.
   - Sets withdraw-withheld authority to the program PDA.
   - Freezes transfer fee config authority.

4. `2_init_degenbtc_SOL_pool.js`
   - Creates Raydium CP-Swap AMM config.
   - Initializes DegenBTC-SOL pool.
   - Adds initial liquidity and optionally burns initial LP.

5. `3_init_mineBTC.js`
   - Initializes the full MineBTC game/program state in the canonical order below.

## `3_init_mineBTC.js` Canonical Init Order

1. Validate token and HashBeast collection metadata URIs.
2. Validate HashBeast genesis cap and per-faction genesis caps.
3. Validate round degenBTC distribution sum.
4. Validate faction-war reward bps sum.
5. Initialize MineBTC program:
   - `GlobalConfig`, `DegenBtcMining`, `HodlPool`, `SOL Treasury`, `Autominer Custody`.
6. Set Raydium pool state and initialize SOL rewards/prize pot vaults.
7. Add configured factions sequentially.
8. Initialize system referral and buybacks accounts.
9. Apply live fee config (includes `nft_market_making_pct`, default 3%).
10. Initialize mining token vault and emission state.
11. Apply emission controller params.
12. Deposit mining tokens.
13. Initialize hashpower config.
14. Initialize degenBTC and LP custodian accounts.
15. Initialize `HashBeastConfig` (no `max_supply` argument — there is no lifetime cap).
16. Seed breeding config.
17. Initialize `HashBeastMintConfig`.
18. Create HashBeast Metaplex Core collection.
19. Initialize HashBeast royalties.
20. Configure ticket tiers.
21. Initialize standalone `degenbtc_market` (NFT marketplace program).
22. Initialize Inventory Pool + Floor Queue + Sale History + Floor History + Inventory Sweep Vault inside mineBTC.
23. Initialize per-faction LootboxQueue PDAs (one per active faction).
24. Initialize tax config (`treasury_pct`, `burn_pct` only — no NFT floor sweep arg).
25. Initialize global game state.
26. Initialize LP token accounts for program custody.
27. Initialize faction-war config.
28. Run the legacy faction-war config no-op in setup scripts, if needed for older runbooks.
29. Update gameplay tuning.

## Important Live Setup Values

From `setup_scripts/config.json` and `3_init_mineBTC.js`:

- Network cluster: devnet in current config.
- Token decimals: 6.
- Initial supply: 2,100,000,000 dBTC.
- Transfer tax: 10 bps.
- Round duration: 60 seconds.
- Mining emission: `degen_btc_per_round = 1,000,000,000` base units.
- Genesis HashBeast mint limit: 36,000.
- Lifetime HashBeast cap: none; only the genesis sale is capped.
- Genesis max per configured faction: 3,000 with current 12-faction config.
- Genesis base price: 1 SOL.
- Genesis curve A: 2,100,000.
- Breed base price: 2 SOL.
- Breed curve A: 200,000.
- Breeding disabled at launch unless explicitly toggled.
- Ticket tiers: 0.001 SOL, 0.01 SOL, 0.1 SOL equivalent point values.
- Hashpower lockup multiplier: 1x to 3x.

## Fee Config Applied By Init Script

`LIVE_FEE_CONFIG` in `3_init_mineBTC.js` currently sets:

- Protocol fee: 15% of SOL bets.
- Buyback/POL share: 80% of treasury SOL.
- NFT market making share: 3% of treasury SOL → `inventory_sweep_vault`.
- Stakers share: 10% of protocol fee.
- degenBTC stakers: 3% of round emission.
- Exact winners: 50%.
- Same-faction non-winning directions: 21% each.
- Jackpot: 5%.
- HODL tax: 10%.
- Snapshot interval: 5 minutes in setup script.
- Referral cut: 5% of protocol fee cross-country, 10% same-country.
- Cycle SOL split: 5% of gross bet reserved for faction-war SOL jackpot.

## Keeper and Test Scripts

- `setup_scripts/do_txs.js` - manual cranker / operations script. Single file with every game / economy / NFT-marketplace cranker (`startRound`, `endRound`, `settleRound`, `settleFactionWar`, `distributeSolFees`, `snapshotPrice`, `updateRate`, `addLpAndBurn`, `crankHarvestFees`, `crankDistributeTax`, `recordFloorSnapshot`, plus `printState` / `printGameState`). Comment/uncomment calls in `main()` to run what you want.
- `setup_scripts/test_genescience.js`, `sim_egg_mints.js` - pure-JS simulation helpers (DNA decoder, mint curve simulation).
- `setup_scripts/user_activity/` - wallet and user-action helpers (bets, etc).
