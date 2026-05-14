# Economy and Rewards

Canonical sources: `state.rs`, `economy.rs`, `game.rs`, `stake.rs`, `faction_war.rs`, `tax.rs`, and `setup_scripts/3_init_mineBTC.js`.

## Round Reward Distribution

Per setup script live config:

- degenBTC stakers: 3% of round emission.
- Exact winning faction+direction bettors: 50%.
- Same winning faction but non-winning directions: 21% each.
- Jackpot: 5%.

The same-faction percentage is per losing direction. With 3 directions, this means `50 + 21 + 21 + 3 + 5 = 100`.

## SOL Bet Flow

Per setup script:

- Protocol fee: 15% of gross SOL bet.
- Staker share: 10% of protocol fee, which equals 1.5% of gross bet.
- Remaining protocol treasury share: 13.5% of gross bet before referral effects.
- Prize pot receives the non-fee portion, adjusted by cycle SOL split where applicable.
- Cycle SOL split: 5% of gross bet reserved for faction-war cycle SOL rewards.
- Referral commission comes from protocol fee:
  - 5% of protocol fee for cross-country recruits.
  - 10% of protocol fee for same-country recruits.

## SOL Treasury Distribution (`distribute_sol_fees`)

- `buyback_pct` default 80% → `buybacks_sol_vault` (price snapshots + POL).
- `nft_market_making_pct` default 3% → `inventory_sweep_vault` (NFT marketplace fuel + keeper bounties).
- Residual (default ~17%) → `fee_recipient` via WSOL (dev earnings).
- Constraint: `buyback_pct + nft_market_making_pct ≤ 100`.

## HODL Tax

- Configured by `DegenBtcDistConfig.hodl_tax_pct`.
- Current setup script uses 10%.
- Charged when withdrawing degenBTC rewards if there are remaining HODL-pool participants.
- Redistributed through `HodlPool.hodl_tax_index`; it is not a protocol drain.

## Staking Multipliers

- Lockup staking multiplier uses `HashpowerConfig`, setup expects base 100 and max 300, meaning 1x to 3x.
- Passive HashBeast staking multiplier is capped by `PASSIVE_HASHBEAST_STAKING_MAX_MULTIPLIER = 3000` (3.0x on HashBeast scale).
- Combined passive staking boost can therefore reach 9x when max lockup and max HashBeast multiplier align.
- Gameplay HashBeast multiplier cap is `GAMEPLAY_MAX_MULTIPLIER = 4200` (4.2x).

## Economy Cycle

- `snapshot_price()` appends price observations and earmarks SOL for POL.
- `update_rate()` adjusts degenBTC emission rate based on price movement:
  - Default threshold: 3%.
  - Default increase when price rises: 1%.
  - Default decrease when price falls: 3%.
- `add_lp_and_burn()` performs POL liquidity add and burns LP.
- `DegenBtcMining.pol_stats.lp_operations_count` is the economy-cycle counter used by faction-war settlement.

## Faction-War Cycle Rewards

- Faction wars are gameplay-score-driven cycles tied to LP-burn operations.
- `FactionWarConfig.settle_at_lp_op_count` points to the LP operation count that unlocks settlement.
- Ranking source:
  - Gameplay scores per faction, including round outcome score and mutation-score bonuses.
  - Round wins are the first tiebreaker; faction id is the deterministic final tiebreaker.
- Direction resolution compares start ranks to final ranks:
  - Improved rank -> Up.
  - Same rank -> Neutral.
  - Worse rank -> Down.
- Reward split bps from current setup:
  - Base reward: 7500 bps.
  - MVP reward: 500 bps.
  - HashBeast reward: 2000 bps.
- There is no separate loyalty pool. Home-country activity matters through HashBeast mutation score and MVP eligibility.

## HashBeast Genesis and Lifecycle Economics

- Genesis mint cap: 36,000.
- Lifetime HashBeast cap: **none** (the `max_supply` field has been removed from `HashBeastConfig`). Post-genesis, breeding is the only mint path; it is admin-gated (`breeding_allowed` flag) and price-curved, not hard-capped.
- Per-faction genesis cap: 3,000 with 12 configured factions.
- Genesis base price: 1 SOL.
- Genesis curve A: 2,100,000.
- Breeding disabled at launch by config, but seeded:
  - Breed base price: 2 SOL.
  - Breed curve A: 200,000.
  - Breed cost = `max(curve_price, 1.5 × current_floor_anchor)`. Anchor sourced from `FloorHistory.current_anchor()`.
  - Payment is 50% SOL (25% to fee_recipient, 75% to sol_treasury) + 50% degenBTC by SOL value (50% burned, 50% to mining vault). Breeding is blocked unless the genesis sale is sold out and a floor anchor exists.
  - Same-faction and same-rebirth-generation gates on parents.
- Per-asset rebirth cap: 7. Each rebirth pays the owner the asset's accumulated_val and pushes it into the country lootbox queue (or burns if queue full).
- HashBeast tickets are point-value free tickets usable for betting:
  - 0.001 SOL equivalent.
  - 0.01 SOL equivalent.
  - 0.1 SOL equivalent.

## NFT Market Making

- **Funded from SOL, not from the dbtc transfer tax.** `SolFeeConfig::nft_market_making_pct` (default 3%) of every `distribute_sol_fees` flow routes into `inventory_sweep_vault`.
- Permissionless on-chain market maker:
  - 20-slot sorted floor queue tracks cheapest user listings.
  - `sweep_floor_lowest` ix anyone can crank — buys cheapest, auto-disposes (queue if space, else relist at formula markup, else burn if 7-day floor crashed below `BURN_TREND_BPS_THRESHOLD = -3000`).
  - 7-day rolling floor anchor based on median of qualifying user-to-user sales (5-min minimum listing age, 17-sample minimum) with queue/prior-anchor caps. First anchor is capped to marketplace min; queue fallback can downshift, but cannot raise an existing anchor by itself.
  - Relist markup formula: `1500 ± trend_modifier - 500 × expire_count` bps, clamped to [-2000, +6000] bps. `MAX_EXPIRES = 3` strikes per asset before forced burn.
  - Keeper bounty `KEEPER_REWARD_LAMPORTS = 0.0005 SOL` per sweep / snapshot / expire ix.
- 50% of marketplace sale proceeds (when seller is `inventory_pda`) flows back into `inventory_sweep_vault` via `handle_inventory_proceeds`; the other 50% goes to `sol_treasury`.

## Token Tax

- Token-2022 transfer tax default: 10 bps.
- Tax config split (default):
  - Faction treasury: 25%.
  - Burn: 50%.
  - Recycle to mining vault: residual 25%.
  - There is **no NFT floor sweep slice** in the tax — that path was removed. NFT market making is funded from SOL fees instead.
- Faction treasury rewards after settlement:
  - 80% rank-weighted.
  - 20% lucky draw.
  - Reward indexes credit the post-fee amount that reaches the mining vault, because protocol transfers also pay Token-2022 transfer fees.
