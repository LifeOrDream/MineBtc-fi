<p align="center">
  <a href="./Readme.md">Overview</a> |
  <a href="./GAMEPLAY.md">Gameplay</a> |
  <a href="./ECONOMY.md">Economy</a> |
  <a href="./NFTS.md">HashBeasts</a>
</p>

# Economy

degenBTC is MineBTC's mined game token: a fixed-supply, Token-2022 Bitcoin meme asset distributed through gameplay instead of an airdrop farm.

The economy has one job: **turn player SOL volume into game rewards, buybacks, permanent liquidity, NFT floor support, and adaptive dBTC emissions.**

## Economy Thesis

| Design question | MineBTC answer |
|---|---|
| How is dBTC distributed? | Through rounds, faction wars, staking lanes, jackpots, and claim flows. |
| What creates buy pressure? | SOL fee routing into buybacks and protocol-owned liquidity. |
| What slows emissions when price weakens? | Snapshot-based emission adjustment every economy cycle. |
| What supports NFT floor demand? | SOL-funded inventory sweep vault with permissionless floor sweeps. |
| What rewards delayed withdrawal? | HODL tax on gameplay-earned dBTC claims. |
| What prevents passive farming from being the whole game? | Country competition, autominers, jackpots, HashBeast progression, MVP rewards. |

## degenBTC

Current token setup:

| Property | Value |
|---|---:|
| Name | degenBTC |
| Symbol | dBTC |
| Decimals | 6 |
| Fixed supply | 2.1B |
| Token standard | Token-2022 |
| Transfer tax | 0.1% |
| Base round emission | 1,000 dBTC |

The token is pre-minted into the program's mining vault and released by game logic. It is not emitted from a team wallet or dashboard faucet.

## Round Emission Split

Each 60-second round slices configured dBTC emissions into reward lanes.

| Lane | Current share | Economic role |
|---|---:|---|
| Exact country + direction winners | 50% | Main high-conviction reward. |
| Same country, wrong direction A | 21% | Keeps country loyalists alive even when direction misses. |
| Same country, wrong direction B | 21% | Same-country consolation lane. |
| Winning-country stakers | 3% | Passive faction backing. |
| Jackpot | 5% | Long-tail suspense and pot growth. |

This means a round is never just one winner bucket. It rewards exact prediction, country loyalty, staking alignment, and jackpot attention.

## SOL Bet Router

Current bet routing:

| Flow | Current value | Destination |
|---|---:|---|
| Protocol fee | 15% of gross bet | Split into staker rewards, referrals, and treasury. |
| Cycle SOL split | 5% of gross bet | Faction-war SOL reward pool. |
| Round prize side | Remainder | Paid to round winners. |
| Same-country referral | 1.0% of gross bet | Referrer from protocol fee. |
| Cross-country referral | 0.5% of gross bet | Referrer from protocol fee. |

Protocol-fee treasury distribution:

| Treasury lane | Current share | Use |
|---|---:|---|
| Buybacks / economy cycle | 70% | Price snapshots, dBTC swaps, protocol-owned liquidity. |
| NFT market-making vault | 3% | HashBeast floor sweeps and keeper rewards. |
| Fee recipient residual | 27% | Multisig / operating revenue. |

```text
gross SOL bet
  -> protocol fee
       -> staker SOL rewards
       -> referral rewards
       -> treasury
            -> buybacks and POL
            -> NFT market-making vault
            -> fee recipient
  -> cycle SOL pool
  -> round prize pot
```

## Economy Cycle

The macro loop lives in `economy.rs`.

| Step | Instruction | Purpose |
|---:|---|---|
| 1 | `distribute_sol_fees` | Moves available treasury SOL into buybacks, NFT market-making, and fee recipient lanes. |
| 2 | `snapshot_price` | Uses buyback SOL for a small Raydium SOL -> dBTC swap and records observed price. |
| 3 | `update_rate` | Uses the snapshot window to adjust `dbtc_per_round` and faction-war mining multiplier. |
| 4 | `add_lp_and_burn` | Pairs SOL + dBTC into Raydium LP, burns LP, and closes the current faction-war cycle boundary. |

Current snapshot tuning:

| Parameter | Current value |
|---|---:|
| Production target interval | 30 minutes |
| Current devnet setup interval | 5 minutes |
| Snapshot window | 8 samples |
| Production target cycle length | about 4 hours |
| Current devnet cycle length | about 40 minutes |
| Price-change threshold | 3% |
| Emission increase on upside | +1% |
| Emission decrease on downside | -3% |

The asymmetry is intentional: emissions loosen slowly when price is strong and tighten faster when price is weak.

## Protocol-Owned Liquidity

`add_lp_and_burn` pairs earmarked SOL with dBTC from the mining vault, deposits into the canonical Raydium pool, then burns the LP tokens.

| Action | Result |
|---|---|
| Buyback SOL enters Raydium path | Creates price discovery and token demand. |
| SOL + dBTC are added as liquidity | Deepens the market. |
| LP tokens are burned | Liquidity becomes permanently protocol-owned. |
| LP operation count advances | Faction-war cycle can settle at the boundary. |

This makes the economy cycle do double duty: market support and game-cycle timing.

## Faction-War Mining Multiplier

Rounds mine dBTC every minute. Faction wars use a separate cycle multiplier.

| Bound | Value |
|---|---:|
| Minimum | 0.1x |
| Default | 1.0x |
| Maximum | 3.0x |
| Upside adjustment | +3% |
| Downside adjustment | -10% |

This makes the longer cycle reward pool more sensitive to token weakness than token strength.

## Transfer Tax

degenBTC uses Token-2022 transfer fees.

| Tax destination | Current split |
|---|---:|
| Burn | 50% |
| Faction treasury | 25% |
| Mining vault recycle | 25% residual |

The tax is small enough not to dominate trading, but every transfer still contributes to burn, faction rewards, or future emissions.

## HODL Tax

Gameplay-earned dBTC can remain claimable inside MineBTC instead of being withdrawn immediately.

Current withdrawal tax:

| Claim source | HODL-tax eligible? | Why |
|---|---:|---|
| Round gameplay dBTC | Yes | Gameplay claimable rewards participate in the HODL pool. |
| Faction-war dBTC | Yes | Cycle claimable rewards participate in the HODL pool. |
| Staking dBTC | No | Staking claims are passive reward claims and are paid separately. |
| Rebirth accumulated value | No | Rebirth is NFT value extraction, not ordinary gameplay claimable. |

The HODL tax is redistributed to remaining gameplay claimants through the HODL index. It is not a protocol drain.

```text
gameplay dBTC withdrawal
  -> user receives 90%
  -> 10% updates HODL index
  -> remaining gameplay claimants earn the redistribution
```

If there are no remaining eligible gameplay claimants, the contract avoids trapping value in a meaningless redistribution path.

## Staking Economy

MineBTC has two passive staking rails:

| Track | Stake asset | Reward relationship |
|---|---|---|
| dBTC staking | degenBTC | Earns from winning-country staker lanes and faction treasury indexes. |
| LP staking | Raydium LP | Same faction-oriented staking reward framework. |

Boosts:

| Boost | Current cap |
|---|---:|
| Lockup multiplier | 3x |
| Passive HashBeast boost | 3x |
| Combined passive hashpower | 9x |

Users can stake up to 3 passive HashBeasts. This is separate from the one active gameplay HashBeast operator used for round claims and mutations.

## NFT Market-Making Funding

The NFT market maker is SOL-funded from treasury distribution.

| Parameter | Value |
|---|---:|
| NFT market-making share | 3% of distributed treasury SOL |
| Vault | `inventory_sweep_vault` |
| Spend cap | 5% of vault per sweep |
| Price cap | floor anchor x 1.05 |
| Reserve | 0.05 SOL |

The vault buys cheap HashBeasts through permissionless sweeps. Inventory then goes to lootboxes, relists, or burns depending on market state and queue capacity.

## Sources And Sinks

| dBTC inflow / distribution | dBTC sink / friction |
|---|---|
| Mining vault emissions | Transfer-tax burn |
| Round rewards | Breeding dBTC burn |
| Faction-war rewards | HODL tax redistribution friction |
| Staker lanes | LP pairing and burn-supported liquidity |
| Tax recycle to mining vault | Breed payments split between burn and mining vault recycle |
| Rebirth accumulated value payouts | Supply recycling / burn pressure on the NFT side |

| SOL source | SOL use |
|---|---|
| User bets | Round prize pools |
| Protocol fees | Staker rewards |
| Breeding SOL leg | Treasury and fee recipient |
| Inventory sale proceeds | Sweep vault and treasury |
| Marketplace activity | Seller / fee flows in marketplace program |

## Why This Economy Can Be Fun

The economy is built so each loop reinforces another loop:

| Loop | Reinforces |
|---|---|
| SOL bets | round rewards, staker fees, treasury, faction-war SOL pools |
| Treasury buybacks | dBTC demand, price snapshots, POL |
| LP burns | permanent liquidity and faction-war cycle closure |
| HashBeast progression | country score, MVP competition, breed demand |
| NFT floor sweeps | lootbox inventory, supply recycling, floor confidence |
| HODL tax | delayed gameplay dBTC withdrawal |

The goal is not passive yield. The goal is a loud, repeatable, country-vs-country game that uses the token as the distribution rail.
