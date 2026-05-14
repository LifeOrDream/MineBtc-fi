<p align="center">
  <a href="./Readme.md">Overview</a> |
  <a href="./GAMEPLAY.md">Gameplay</a> |
  <a href="./ECONOMY.md">Economy</a> |
  <a href="./NFTS.md">HashBeasts</a>
</p>

# Gameplay

MineBTC has two clocks:

| Clock | Feeling | Purpose |
|---|---|---|
| **60-second rounds** | Fast casino loop. | Users bet SOL on country + direction, chase dBTC/SOL rewards, jackpots, and claim-time HashBeast rolls. |
| **Faction-war cycles** | Community strategy loop. | Rounds combine into country rankings, final Up/Neutral/Down results, base rewards, HashBeast rewards, and MVP bonuses. |

The product thesis is simple: **one-minute gambling energy with prediction-market memory.**

Faction wars are not a hard-coded wall-clock. They close when the economy's LP-burn cycle completes. The production design target is roughly 4 hours (8 snapshots x 30 minutes); the current devnet setup uses faster 5-minute snapshots for iteration.

## Countries And Directions

Current setup uses 12 countries:

| # | Country | # | Country |
|---:|---|---:|---|
| 0 | USA | 6 | Iran |
| 1 | China | 7 | UK |
| 2 | Russia | 8 | North Korea |
| 3 | India | 9 | France |
| 4 | Japan | 10 | Brazil |
| 5 | South Korea | 11 | Israel |

Each bet chooses:

| Field | Meaning |
|---|---|
| Country | Which faction the user is backing this round. |
| Direction | `Down`, `Neutral`, or `Up`. |
| SOL amount or ticket points | Economic weight. |
| Active HashBeast multiplier | Gameplay weight if the user has an operator deployed. |

Countries are the social wrapper. They create instant teams, rivalries, memes, recruitment loops, and leaderboard pressure.

## Round Lifecycle

| Stage | Instruction | Caller | What changes |
|---:|---|---|---|
| 0 | `start_round` | Permissionless keeper | Creates the next `GameSession`, sets timer and scheduled entropy slot. |
| 0 | `join_bets` / autominer execution | User or keeper | Adds SOL, country-direction points, weighted points, and fee routing. |
| 1 | `end_round` | Permissionless keeper | Resolves entropy, chooses winning country + direction, sizes reward pools, rolls jackpot dice. |
| 2 | `settle_round` | Permissionless keeper | Finalizes reward indexes, staker lanes, jackpot state, and faction-war handoff. |
| 2 | `claim_round_rewards` | User or autominer keeper | Pays user, syncs HODL state, and can trigger HashBeast / lootbox rolls. |

```text
open round -> bets -> entropy slot lands -> result locked -> round settled -> claims live
```

The important production detail: **empty rounds still pass through settle**. Even a zero-bettor boundary round must fold into the faction-war cycle so the war cannot get stuck waiting for `last_processed_round_id`.

## Winner Selection

`end_round` derives a hash from slot-hash entropy plus round state, then resolves:

| Roll | Selection logic |
|---|---|
| Winning country | Sampled from countries with activity. If the round is empty, sampled from supported countries for continuity. |
| Winning direction | Sampled from directions with activity on the winning country. |
| Jackpot | Independent 1-in-625 roll. If it lands, the target country is selected with inverse-volume weighting. |

Entropy prefers the scheduled slot hash. If that slot ages out of the Solana `SlotHashes` ring buffer, the contract falls back to the latest available slot so the game stays live. Events surface whether fallback was used.

## Round Rewards

Current base emission is **1,000 dBTC per round**.

| Lane | Current share | Paid to |
|---|---:|---|
| Exact winner | 50% | Bets on winning country + winning direction. |
| Same country, wrong direction A | 21% | One losing direction on the winning country. |
| Same country, wrong direction B | 21% | Other losing direction on the winning country. |
| Winning-country stakers | 3% | dBTC and LP stakers for the winning country. |
| Jackpot pot | 5% | Accumulates until jackpot hit. |

Orphan handling keeps rounds clean:

| Empty lane | Behavior |
|---|---|
| Same-country losing direction has no bettors | Redirects to winner lane where appropriate. |
| Winning country has no stakers | Redirects staker slice to winners. |
| Jackpot target has no eligible claimants | Jackpot rolls forward. |

SOL rewards are split from the bet pot after protocol fee and cycle SOL split are carved out.

## Jackpot

The jackpot is the long-tail round drama.

| Parameter | Value |
|---|---:|
| Chance | 1 / 625 per round |
| Source | 5% dBTC jackpot lane |
| Targeting | Inverse-volume weighting favors under-bet countries |
| Claim behavior | If no eligible claimant exists, pot rolls forward |

This creates a reason to keep watching even when a country is not dominating normal volume.

## Claims Are Where The Game Changes

Bet placement records exposure. **Claiming is where resolved activity turns into progression.**

| Claim type | Possible side effect |
|---|---|
| Winning claim with active HashBeast | XP, multiplier, DNA mutation, country score, MVP competition. |
| Losing claim on home country | Lootbox roll if the country's queue has inventory and user has no pending claim. |
| Gameplay dBTC withdrawal | HODL tax applies to gameplay-earned dBTC only. |
| Staking reward claim | Paid separately; does not earn HODL-tax yield. |

This is deliberate. Mutation rolls are tied to resolved outcomes, not bet spam.

## HashBeast Story Events

When a user claims a winning round with a gameplay HashBeast active, the contract can roll a story event.

| Event | Score weight | Meaning |
|---|---:|---|
| Evolution | 4x | Major progression event, rarest and most impactful. |
| Power | 2x | Stronger power/multiplier event. |
| Trait | 1x | Visual or secondary trait event. |

Current tuning:

| Parameter | Value |
|---|---:|
| Base mutation chance | 20% |
| Chance floor | 0.25% |
| Chance cap | 25% |
| Target mutations per cycle | 12 |
| Target rounds per cycle | 240 |
| Pacing adjustment cap | +/-40% |

The roll is shaped by correctness, active multiplier, country volume, per-round mutation pressure, and cycle pacing.

## Home Vs Mercenary Mutation Impact

MineBTC allows mercenary behavior without letting it drain home-country reward pools.

| Case | Leaderboard score | HashBeast pool credit | MVP credit |
|---|---:|---:|---:|
| User mutates on home-country win | 100% | Yes | Yes |
| User mutates on foreign-country win | 50% | No | No |

This keeps the invariant safe:

```text
user_home_mutation_score <= faction_mutation_score[user_home_country]
```

Foreign wins can still visibly push a country up the leaderboard, but they cannot earn that user the HashBeast/MVP rewards of their home country.

## Faction-War Cycle

Faction war is the longer game that turns many 60-second rounds into country rankings.

| Phase | Trigger | What happens |
|---|---|---|
| Start war | `initialize_faction_war` | Creates war state and settlement accounts, seeds treasury, unblocks rounds. |
| Active rounds | `start_round -> end_round -> settle_round` | Round points, SOL, dBTC, wins, and mutation score fold into the war. |
| Boundary | `add_lp_and_burn` | Economy LP burn captures `cycle_end_round_id` and blocks new rounds. |
| Final round settle | `settle_round` | The boundary round is processed into war state. |
| War settle | `settle_war` | Computes final ranks, directions, reward pools, MVPs, and residuals. |
| Claims | `claim_faction_war_rewards` | Users claim base, HashBeast, MVP, and SOL mirror rewards. |
| Next war | `initialize_faction_war` | Clears the boundary and starts the next cycle. |

The cycle compares each country's final rank against its previous rank:

| Rank movement | Resolved direction |
|---|---|
| Moved up | `Up` |
| Stayed same | `Neutral` |
| Moved down | `Down` |

Users who predicted those final directions correctly earn the base cycle rewards.

## Cycle Reward Lanes

Current split:

| Lane | Share | Eligibility | Formula |
|---|---:|---|---|
| Base | 75% | User predicted a country's final direction correctly. | Country pool x user weighted points / total correct weighted points. |
| HashBeast | 20% | User produced home-country mutation score. | Home pool x user mutation score / faction mutation score. |
| MVP | 5% | User is the top mutation-score contributor for a country. | Rank-weighted per-country MVP bonus. |

SOL cycle rewards mirror the dBTC lane shape. Unallocated dBTC remains in the mining vault; unallocated SOL drains conservatively to treasury.

## Autominers

Autominers are the passive gameplay path. Users fund an autominer, configure it, and keepers execute the normal bet/claim loop on their behalf.

| Autotrack step | What it does |
|---|---|
| Fund | User deposits SOL or tickets. |
| Execute | Keeper places the configured round bet. |
| Settle | Round finishes through normal permissionless flow. |
| Claim | Keeper claims rewards for the user. |
| Reload | Winnings can fund future rounds, or SOL returns to owner. |

Autominers matter because MineBTC is designed for high-frequency rounds without requiring every user to click every minute.

## Loser Lootbox Rolls

Losing users can still hit a consolation NFT outcome.

Eligibility:

| Requirement | Reason |
|---|---|
| User lost the round claim. | Lootboxes are consolation upside, not winner extra yield. |
| User bet on their home country. | Keeps drops tied to country loyalty. |
| Country lootbox queue has inventory. | No phantom claims. |
| User has no pending lootbox claim. | Prevents stacking unresolved reservations. |

Drop chance by current queue depth:

| Depth | Chance | Depth | Chance |
|---:|---:|---:|---:|
| 0 | 0% | 6 | 0.58% |
| 1 | 0.03% | 7 | 0.78% |
| 2 | 0.08% | 8 | 1.00% |
| 3 | 0.15% | 9 | 1.25% |
| 4 | 0.25% | 10 | 1.50% |
| 5 | 0.40% |  |  |

Full inventory is exciting, but never a guaranteed drain race.

## Gameplay Safety Notes

| Risk | Guard |
|---|---|
| New round starts before next war exists | `cycle_end_round_id` blocks starts until the next war initializes. |
| Empty boundary round freezes cycle | Empty rounds still settle and advance `last_processed_round_id`. |
| Foreign mutations over-claim HB pool | Only home mutations update user mutation score and HB/MVP denominators. |
| Late claims mutate already-settled rankings | Late cycle score updates are skipped once war is settled. |
| Rent-starved SOL vault blocks payouts | Native SOL payouts send available amount above rent reserve instead of reverting. |

The simple UX line remains:

> **Pick a country. Bet a direction. Claim rewards. Mutate your operator. Push the country. Win the cycle.**
