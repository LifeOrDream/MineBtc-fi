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

## HODL Tax

- Configured by `MineBtcDistConfig.hodl_tax_pct`.
- Current setup script uses 10%.
- Charged when withdrawing degenBTC rewards if there are remaining HODL-pool participants.
- Redistributed through `HodlPool.hodl_tax_index`; it is not a protocol drain.

## Staking Multipliers

- Lockup staking multiplier uses `HashpowerConfig`, setup expects base 100 and max 300, meaning 1x to 3x.
- Passive Doge staking multiplier is capped by `PASSIVE_DOGE_STAKING_MAX_MULTIPLIER = 3000` (3.0x on Doge scale).
- Combined passive staking boost can therefore reach 9x when max lockup and max Doge multiplier align.
- Gameplay Doge multiplier cap is `GAMEPLAY_MAX_MULTIPLIER = 4200` (4.2x).

## Economy Cycle

- `snapshot_price()` appends price observations and earmarks SOL for POL.
- `update_rate()` adjusts degenBTC emission rate based on price movement:
  - Default threshold: 3%.
  - Default increase when price rises: 1%.
  - Default decrease when price falls: 3%.
- `add_lp_and_burn()` performs POL liquidity add and burns LP.
- `MineBtcMining.pol_stats.lp_operations_count` is the economy-cycle counter used by faction-war settlement.

## Faction-War Cycle Rewards

- Faction wars are gameplay-score-driven cycles tied to LP-burn operations.
- `FactionWarConfig.faction_war_settle_cycle` points to the LP operation count that unlocks settlement.
- Ranking source:
  - Gameplay support scores per faction.
  - Round wins and own-country SOL support are tiebreakers.
- Direction resolution compares start ranks to final ranks:
  - Improved rank -> Up.
  - Same rank -> Neutral.
  - Worse rank -> Down.
- Reward split bps from current setup:
  - Base reward: 7000 bps.
  - Loyalty reward: 2000 bps.
  - MVP reward: 500 bps.
  - Doge reward: 500 bps.
- Contract defaults differ slightly in `state.rs` (`6500/2000/500/1000`), but setup explicitly applies the live values above.

## Doge Genesis and Lifecycle Economics

- Genesis mint cap: 36,000.
- Lifetime Doge cap: 100,000.
- Per-faction genesis cap: 3,000 with 12 configured factions.
- Genesis base price: 1 SOL.
- Genesis curve A: 2,100,000.
- Breeding disabled at launch by config, but seeded:
  - Breed base price: 2 SOL.
  - Breed curve A: 200,000.
- Doge tickets are point-value free tickets usable for betting:
  - 0.001 SOL equivalent.
  - 0.01 SOL equivalent.
  - 0.1 SOL equivalent.

## Token Tax

- Token-2022 transfer tax default: 10 bps.
- Tax config split:
  - NFT floor sweep: 25%.
  - Faction treasury: 25%.
  - Burn: 50%.
- Faction treasury rewards after settlement:
  - 80% rank-weighted.
  - 20% lucky draw.
