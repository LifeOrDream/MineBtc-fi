# Economy Cycle: How It Works

The economy cycle is the heartbeat of the MineBTC token economy. It runs continuously
in ~4-hour loops, adjusting the degenBTC emission rate based on market price and
permanently locking liquidity (POL) on every cycle.

## The Three Steps

```
Step 1: snapshot_price      (×8, every 30 min, anyone can crank)
Step 2: update_rate          (once after 8 snapshots, anyone can crank)
Step 3: add_lp_and_burn      (once after rate update, anyone can crank)
```

Each cycle can settle the faction-war leaderboard (gameplay scores).

---

## Step 0: Fee Accumulation (distribute_sol_fees)

**When:** Called periodically to flush the SOL treasury.

**Flow:**
1. Read SOL treasury balance (accumulated from bet fees)
2. Split available SOL using `SolFeeConfig`:
   - `buyback_pct` (default 80%) → `buybacks_sol_vault` (for price snapshots + POL)
   - `nft_market_making_pct` (default 3%) → `inventory_sweep_vault` (NFT marketplace sweep + keeper bounties)
   - residual (default ~17%) → `fee_recipient` via WSOL (dev earnings)
3. Buybacks vault balance grows over time as bets happen
4. Inventory sweep vault keeps the on-chain NFT market maker funded — every cycle adds bid-side liquidity

**Edge cases:**
- If treasury balance ≤ rent-exempt minimum → returns Ok, no-op
- If buyback amount = 0 → skips transfer, no error
- `buyback_pct + nft_market_making_pct` is validated ≤ 100% on update_fees

---

## Step 1: snapshot_price (×8, every 30 minutes)

**What it does:**
1. Takes 10% of available SOL from buybacks vault
2. Swaps that SOL → degenBTC via Raydium (price discovery)
3. Records the price in `price_history` (up to 8 entries)
4. Earmarks another 10% of available SOL for POL (Protocol Owned Liquidity)
5. Computes running weighted average price (later entries weighted more)

**Available SOL calculation:**
```
available = buybacks_vault_balance - rent_exempt - already_earmarked_pol
```

**Per snapshot allocation:**
```
sol_for_swap       = available / 10   (10% → buy degenBTC for price)
sol_for_pol        = available / 10   (10% → earmarked for LP later)
remaining 80%      stays in vault for future snapshots
```

Over 8 snapshots: ~80% of available SOL at snapshot 1 gets consumed
(each snapshot sees a smaller "available" because prior snapshots took 20%).

**Price formula:**
```
price = (sol_swapped_lamports × 10^6) / dbtc_received_base_units
```
Stored as u64 lamports per whole degenBTC. Divide by `1e9` for SOL per degenBTC.

**Weighted average:** weights 1-8 (earliest=1, latest=8).
Later snapshots count more, reflecting more recent market conditions.

**Guards:**
- `lp_operation_pending == false` (can't snapshot during LP phase)
- `price_history.len() < 8` (must call update_rate after 8)
- `snapshot_interval` seconds since last snapshot (default 1800 = 30 min)
- Pool state validated against `global_config.raydium_pool_state`
- If available SOL = 0 → returns Ok, no-op (won't revert)

**Edge case: swap returns 0 degenBTC:**
- Price is set to 0
- Snapshot still recorded (prevents stuck state)
- Weighted average diluted but cycle continues

---

## Step 2: update_rate (once, after 8 snapshots)

**What it does:**
1. Computes final weighted average price from 8 snapshots
2. Compares against `track_price` (price at last rate change) AND first snapshot price
3. Uses whichever comparison shows the larger absolute change
4. If change ≥ `price_change_threshold` (default 3%):
   - Price up → increase `dbtc_per_round` by `emission_increase_pct` (default 1%)
   - Price down → decrease `dbtc_per_round` by `emission_decrease_pct` (default 3%)
5. Sets `lp_operation_pending = true` (blocks new snapshots)
6. Clears `price_history` (ready for next cycle)

**Rate adjustment math:**
```
if price_up:   new_rate = old_rate × (100 + increase_pct) / 100
if price_down: new_rate = old_rate × (100 - decrease_pct) / 100
```

**Asymmetric by design:** price drops reduce emissions 3x faster than
price rises increase them. This creates deflationary pressure during downturns.

**Guards:**
- Requires exactly 8 price entries
- If conditions not met → returns Ok (no error, just skips)
- Rate can only go up or down by the configured percentages per cycle
- `track_price` only updates when rate actually changes (prevents drift)

**Edge case: track_price = 0 (first cycle ever):**
- `calculate_price_change_pct` returns (0, 0) for zero prices
- No rate change on first cycle (intentional: need baseline first)
- `track_price` stays 0 until the first cycle where price exceeds threshold

---

## Step 3: add_lp_and_burn (once, after update_rate)

**What it does:**
1. Takes all earmarked POL SOL (`buybacks_account.sol_for_pol`)
2. Calculates how much degenBTC is needed to match the SOL at pool ratio
3. Deposits SOL + degenBTC into Raydium pool → receives LP tokens
4. Burns ALL LP tokens received (permanent liquidity lock)
5. Updates `pol_stats` (cumulative POL metrics)
6. Clears `lp_operation_pending` flag (unblocks next cycle of snapshots)
7. Increments `pol_stats.lp_operations_count` (triggers epoch settlement)

**LP amount calculation:**
```
lp_from_sol = (available_sol × lp_supply) / sol_vault_balance
required_dbtc = (lp_from_sol × dbtc_vault_balance) / lp_supply
```
Plus 2% buffer for slippage, plus Token-2022 transfer fee gross-up.

**degenBTC limit (prevents draining the mining vault):**
If the required degenBTC exceeds 1% of available degenBTC in the mining vault,
the SOL amount is reduced proportionally to match the 1% degenBTC cap.
This ensures LP operations never drain more than 1% of the vault per cycle.

**Admin override:** If `lp_token_amount > 0` is passed, the caller must be
`ext_authority`. This allows manual LP sizing for special situations.

**SOL buffer:** 2% of POL SOL is reserved as buffer (not deposited) to
absorb Raydium slippage. Remaining SOL after deposit is returned to
buybacks_sol_vault via WSOL account closure.

**Guards:**
- `lp_operation_pending == true` (must have called update_rate first)
- Pool state validated against `global_config.raydium_pool_state`
- If `sol_for_pol == 0` → clears flag, returns Ok (no-op, unblocks cycle)
- If `sol_vault_balance == 0` or `lp_supply == 0` → clears flag, returns Ok
- `InsufficientTokensInVault` if degenBTC can't cover the deposit
- LP tokens burned immediately (no window for theft)
- Remaining SOL returned to vault (never lost)

---

## State Machine

```
                 ┌─────────────────────┐
                 │   READY TO SNAPSHOT  │
                 │  lp_operation = false│
                 │  price_history < 8   │
                 └──────┬──────────────┘
                        │ snapshot_price (×8)
                        ▼
                 ┌─────────────────────┐
                 │  8 SNAPSHOTS DONE   │
                 │  price_history == 8  │
                 └──────┬──────────────┘
                        │ update_rate
                        ▼
                 ┌─────────────────────┐
                 │   LP PENDING        │
                 │  lp_operation = true │
                 │  price_history == 0  │
                 └──────┬──────────────┘
                        │ add_lp_and_burn
                        ▼
                 ┌─────────────────────┐
                 │  CYCLE COMPLETE     │
                 │  lp_operation = false│
                 │  lp_ops_count += 1  │◄── triggers epoch settlement
                 └──────┬──────────────┘
                        │ (back to top)
                        ▼
```

**Can the cycle get stuck?**

No. Every transition has a "skip and unblock" path:
- `snapshot_price`: if no SOL available → returns Ok (doesn't add entry, but doesn't block)
- `update_rate`: if < 8 entries → returns Ok (no state change)
- `add_lp_and_burn`: if no POL SOL or empty pool → clears flag and returns Ok

The only way to "block" is if `lp_operation_pending = true` forever, which
can only happen if `update_rate` is called but `add_lp_and_burn` is never called.
Since both are permissionless, any keeper can unstick the cycle.

---

## Token Flows

```
SOL from bets
    │
    ▼ (distribute_sol_fees)
┌───────────────┐  ┌───────────────────────┐  ┌──────────────────┐
│ buybacks_vault│  │ inventory_sweep_vault │  │  fee_recipient   │
│ (80% of fees) │  │ (3% — NFT MM)         │  │ (~17% dev share) │
└───────┬───────┘  └───────────────────────┘  └──────────────────┘
        │
        ▼ (snapshot_price ×8)
┌───────────────┐     ┌──────────────────┐
│ 10% swap SOL  │────►│  degenBTC bought  │
│ → price disc. │     │  (stays in vault)│
└───────────────┘     └──────────────────┘
┌───────────────┐
│ 10% earmarked │
│ for POL       │
└───────┬───────┘
        │
        ▼ (add_lp_and_burn)
┌───────────────┐     ┌──────────────────┐
│ POL SOL +     │────►│  Raydium Pool    │
│ degenBTC from  │     │  (LP tokens)     │
│ mining vault  │     └────────┬─────────┘
└───────────────┘              │
                               ▼
                      ┌──────────────────┐
                      │  LP BURNED 🔥    │
                      │  (permanent lock)│
                      └──────────────────┘
```

---

## Key Safety Properties

1. **No SOL can be lost:** Every transfer is between program-owned PDAs.
   Remaining SOL after LP deposit is returned to buybacks vault.

2. **No degenBTC drain:** LP operations capped at 1% of mining vault per cycle.
   Even a malicious admin override (`lp_token_amount > 0`) is bounded by
   available vault balance.

3. **No stuck state:** Every instruction has a graceful exit for edge cases.
   `lp_operation_pending` is always cleared in `add_lp_and_burn`, even on no-op.

4. **Pool validation:** Every CPI to Raydium validates the pool state matches
   `global_config.raydium_pool_state`. A compromised pool can't be substituted.

5. **Permissionless cranking:** All three steps can be called by anyone.
   No admin key required to keep the economy running.

6. **Swap slippage:** `min_amount_out = 0` on price discovery swaps is
   intentional — these are tiny 10%-of-10% amounts used for price oracle,
   not large trades. Sandwich attacks on these amounts are unprofitable
   relative to the gas cost.

---

# Faction War Cycle: Gameplay Score Leaderboard

Each economy cycle (LP burn) can settle the active **Faction War** — the
competitive cycle where own-country gameplay support determines country rankings and
distribute degenBTC rewards.

Product note: the contract mutates HashBeast DNA for Evolution / Power / Trait
events only during successful reward claims. Backends should treat these as
**story events** that can become art, reels, character-history entries, or
simple indexed gameplay beats.

## How It Works

```
Players bet SOL in 60-second rounds
    │
    ├─ Own-country gameplay hashbeast bets add support score
    │   └─ score = support_weight × bet_size × multiplier
    │
    ├─ gameplay scores accumulate on FactionWarState per faction
    │
    └─ total_degenbtc_mined_in_faction_war grows with each round
```

When the economy cycle completes (LP burn → `lp_operations_count` increments):

```
finalize_faction_war_settlement():
    1. Rank factions by gameplay score, then round wins, then own-faction SOL support
    2. Compare with start_ranks → compute rank deltas → resolve directions
    3. Split faction_war_mining_pool:
       - base pool: anyone who picked a country's final direction correctly
       - loyalty pool: own-country correct-direction supporters
       - HashBeast pool: users whose gameplay HashBeast backed the resolved home-country outcome
    4. Stage = 1 (claims open)
    5. Persist final_ranks as next faction war's start_ranks
    6. Advance current_faction_war_id
```

## Reward Pools

```
faction_war_mining_pool (total degenBTC mined in all rounds this cycle)
    │
    ├─ base_reward_bps → faction_reward_pools
    │         Users who bet any country's final direction correctly get pro-rata share
    │
    ├─ loyalty_reward_bps → loyalty_reward_pools
    │         Own-country correct-direction supporters get pro-rata share
    │
    └─ hashbeast_reward_bps → faction_hashbeast_reward_pools
              Eligible gameplay HashBeasts get accumulated_val (claimable via burn)
```

---

# Betting & XP/Points System

## How a Bet Works (internal_process_bets)

A single call processes multiple country+direction bets in one transaction.

### SOL Flow Per Bet

```
Player bets 1 SOL
    │
    ├─ protocol_fee_pct (default 15%) = 0.15 SOL taken as fee
    │   ├─ stakers_pct (default 20%) of fee = 0.03 SOL → staker SOL reward vault
    │   └─ remaining 80% of fee = 0.12 SOL → SOL treasury (later: buybacks + dev)
    │
    └─ net = 0.85 SOL → SOL prize pot vault (for round winner payouts)
```

### Points vs Weighted Points

Every bet generates two numbers:

- **points** = raw bet size in lamports (1 SOL = 1,000,000,000 points)
- **weighted points** = points × active_multiplier / BASE_MULTIPLIER

For SOL bets: a 1.0x hashbeast gives wgtd_points = points. Gameplay HashBeast power is capped at 4.2x.
For ticket bets: wgtd_points = points (no multiplier). Tickets are capped at 25% of SOL volume per round.

Weighted points determine:
- Your share of degenBTC round rewards (winner pool)
- Your share of degenBTC faction-war rewards (correct direction pool)
- Gameplay score contribution to your country when backing your own country with an active gameplay hashbeast

### XP System

XP accumulates on the gameplay hashbeast from eligible claim-time mutation stake:

```
xp_gained = eligible_claim_stake / 1_000_000  (1 XP per 0.001 SOL before multiplier scaling)
```

Winning claims can still gain XP even if no mutation fires.

**XP effects on story events:**
- Evolution: XP contributes 5-10% as multiplier boost (then XP resets to 0)
- Power/Trait: XP contributes 2-5% as multiplier boost (XP preserved)

**XP is cached** on PlayerData during gameplay and synced back to HashBeastMetadata on:
- Round reward claim
- Faction-war reward claim
- Gameplay hashbeast withdrawal

### Story Event System

**Trigger conditions** (ALL must be true):
1. RPG progression enabled
2. Successful round or faction-war reward claim
3. Eligible SOL stake behind the winning claim (not tickets)
4. Gameplay hashbeast is active
5. The claim's recorded gameplay hashbeast still matches the player's active hashbeast

**Probability:**
```
base_chance = 20%  (MAX_BASE_CHANCE = 2000 bps)
stake_strength = eligible_claim_stake / highest_stake_on_faction  (0-100%)
mult_penalty = BASE_MULTIPLIER / active_multiplier  (100% at 1x, ~24% at 4.2x)
faction_penalty = 10000 / (10000 + prior_events × 5000)  (100%, 67%, 50%, 40%...)
claim_boost = reward-context multiplier

final_chance = base × stake_strength × mult_penalty × faction_penalty × volume × cooldown × pacing × claim_boost
```

Round exact wins get a stronger `claim_boost` than same-faction consolation
claims. Faction-war claims are strongest when the user backed their own country
correctly, especially when that country moved Up.

**Story event types** (when triggered):
- Evolution (~10% / (gen+1)): +50 base multiplier, guaranteed DNA upgrades, XP resets
- Power (~30%): +25 base multiplier, power trait upgrade
- Trait (~60%): +5 base multiplier, visual trait upgrade

Each story event also boosts multiplier by `base + (XP × efficiency_pct / 100)`.

**Gameplay score** (added to country's faction-war leaderboard during betting):
```
score = GAMEPLAY_SUPPORT_SCORE_WEIGHT × own_country_sol_bet × active_multiplier / BASE / PRECISION
GAMEPLAY_SUPPORT_SCORE_WEIGHT=10
```

### HashBeast accumulated_val

When claiming round rewards, the gameplay hashbeast earns degenBTC based on the claim mutation result:

| Story event | % of round degenBTC reward |
|----------|--------------------------|
| Evolution | 6.9% |
| Power | 4.2% |
| Trait | 3.0% |
| No event | 1.0% |

This accumulates on `hashbeast_metadata.accumulated_val` and can only be claimed
by rebirthing the hashbeast with `rebirth_hashbeast`.

`rebirth_hashbeast` is the only lootbox intake path that rebirths an NFT. It pays the
owner's locked degenBTC, increments the NFT's 0-7 rebirth count, writes that
count into DNA bits at offset 174, rerolls fresh DNA, and resets gameplay state
to defaults. Market-maker sweep/expiry lootbox entries preserve the NFT's
existing DNA, multiplier, XP, and breed state.

### HashBeast Breeding

`breed_hashbeasts` requires both parents to:

- run only after the genesis HashBeast sale is sold out
  (`HashBeastMintConfig.genesis_mints >= genesis_mint_limit`),
- be owned by the caller and not locked in gameplay,
- be from the same country/faction,
- have the same `rebirth_count` / DNA rebirth generation,
- have breed count below `MAX_BREED_COUNT`,
- not be the same asset, direct parent/child, or known siblings.

The offspring inherits the parents' rebirth generation, so level-1 reborn
HashBeasts breed level-1 offspring, level-2 with level-2, and so on.

Breed price is the larger of:

```text
compute_gene_price(breed_base_price, breed_curve_a, total_hashbeasts_minted)
floor_history.current_anchor * 1.5
```

Breeding is blocked if the floor anchor is missing/too low or the dbTC TWAP
price is unavailable. Payment is 50% SOL and 50% dbTC by SOL value:

```text
SOL leg: 25% -> fee_recipient, 75% -> sol_treasury
dbTC leg: 50% burned, 50% -> dbtc_token_vault
```

### Gameplay HashBeast Lifecycle

```
1. use_hashbeast_for_gameplay
   └─ Lock NFT in custody, cache stats to PlayerData

2. Play rounds (bets trigger story events, XP accumulates)

3. request_hashbeast_gameplay_unlock
   └─ Sets unlock_request_faction_war = current_faction_war_id
   └─ Must wait until next faction war to withdraw

4. withdraw_hashbeast_from_gameplay
   └─ Requires: next faction war started + no pending claims
   └─ Syncs DNA/XP/multiplier back to HashBeastMetadata
   └─ Returns NFT to user
```

The two-phase unlock prevents mid-cycle HashBeast swapping to farm story events.

### Round Reward Distribution

When a round ends, rewards flow to three groups:

```
dbtc_per_round (degenBTC emission)
    ├─ winners_pct (50%) → exact country+direction winners (pro-rata by wgtd_points)
    ├─ same_faction_pct (20% × 2 directions) → consolation for wrong-direction bettors
    │   on the winning country (pro-rata by wgtd_points per direction)
    ├─ stakers_pct (5%) → all stakers on the winning faction
    └─ jackpot_pct (5%) → global jackpot (1/625 chance of hitting, accumulates)
```

SOL rewards: winning-direction bettors split the SOL prize pot proportional to their points bets.
```

**User claims:**
- `user_reward = faction_pool × user_bet / total_bet` (on correct direction)
- Only own-faction bets count
- HashBeast bonus: same formula but using `eligible_hashbeast_direction_totals` as denominator

**Referral overlay:** if a player joined through a real referral code, their
degenBTC claim gets a 1% bonus from the emissions vault. The referrer accrues 3%
of the base claim, or 5% when both users share the same permanent country. These
overlay rewards do not reduce the round/cycle reward pools.

## Passive Staking in the Economy Loop

Passive staking is a separate accounting layer that sits downstream from rounds.

```text
economy loop sets dbtc_per_round
    ↓
round settlement decides who gets the current round's MineBTC/SOL splits
    ↓
winning faction staking lanes receive index increments
    ↓
stake.rs sync/claim functions materialize those indexes into player balances
```

Two passive staking lanes exist:

- MineBTC staking
- LP staking

Each lane tracks its own faction-level indexes:

- SOL reward index
- MineBTC reward index

Player share is not based on raw deposit amount directly. It is based on:

```text
staked_amount
    -> lockup multiplier (max 3x)
    -> weighted_amount
    -> passive HashBeast multiplier
    -> final staking hashpower (max 9x total)
```

### Passive HashBeast staking

Passive HashBeast staking is different from gameplay-HashBeast locking:

- passive HashBeasts can be from any faction
- they boost only the player's home-faction passive staking positions
- they modify `player_data.hashbeast_multiplier`
- they do **not** directly affect gameplay `active_multiplier`

### Reward realization

SOL staking rewards:

- accrue into `pending_sol_rewards`
- transfer directly to wallet on `claim_staking_rewards`

MineBTC staking rewards:

- accrue into `pending_dbtc_rewards`
- are globally tracked through `hodl_pool.total_dbtc_claimable`
- are withdrawn later through `withdraw_dbtc_rewards`

### HODL tax redistribution

When a user withdraws pending MineBTC rewards, a HODL tax can be taken and
redistributed across remaining unclaimed MineBTC balances via the global
`hodl_tax_index`.

This means claim timing matters:

- fast withdrawal = immediate liquidity, but pay HODL tax
- slower withdrawal = can earn part of other users' HODL taxs

## Tax Treasury Distribution (also tied to faction wars)

After settlement, `claim_faction_treasury_for_faction_war` distributes the faction
treasury vault (accumulated from 0.1% transfer tax):

```
treasury_balance
    ├─ 80% rank-weighted: rank_points = active_factions - rank
    │   Higher rank = more reward, but every faction gets something
    │
    └─ 20% lucky draw: one random faction from rank 5+ wins the whole pot
        Equal probability per eligible faction, deterministic from faction_war_id
```

Rewards go to faction stakers (split 50/50 between degenBTC and LP stakers).

## State Accounts

```
FactionWarConfig (singleton)
    ├─ current_faction_war_id: incrementing counter
    ├─ is_active: admin toggle
    ├─ faction_war_settle_cycle: LP ops count that triggers settlement
    └─ prev_faction_war_ranks: carried forward to next faction war

FactionWarState (one per faction war)
    ├─ faction_war_id, start_timestamp, stage
    ├─ total_degenbtc_mined_in_faction_war → faction_war_mining_pool
    ├─ start_ranks ↔ final_ranks → rank_deltas → resolved_directions
    ├─ faction_gameplay_scores (internal gameplay-score array that drives rankings)
    ├─ faction_direction_totals (base-pool denominator for user claims)
    ├─ faction_sol_direction_totals (claim-time mutation stake context)
    ├─ loyalty_direction_totals (own-country loyalty-pool denominator)
    ├─ eligible_hashbeast_direction_totals (denominator for hashbeast bonus)
    ├─ faction_reward_pools (base user share)
    ├─ loyalty_reward_pools (own-country loyalty share)
    └─ faction_hashbeast_reward_pools (HashBeast claim bonus share)

UserFactionWarBets (one per user per faction war)
    ├─ direction_bets: weighted bets across countries
    ├─ sol_direction_bets: SOL stake by country/direction for claim-time mutation context
    ├─ loyalty_direction_bets: weighted own-country bets
    ├─ gameplay_hashbeast: which hashbeast became eligible
    └─ hashbeast_bonus_eligible: did this user's active gameplay hashbeast back its own country?
```

## Lifecycle

```
  ┌─────────────────────────────┐
  │  IDLE (no active faction war) │
  │  faction_war_state id = 0     │
  └──────────┬──────────────────┘
             │ First bet in new cycle (auto-start)
             ▼
  ┌─────────────────────────────┐
  │  ACTIVE (stage = 0)         │
  │  Gameplay scores accumulate │
  │  Round bets accumulate      │
  │  Mining pool grows          │
  └──────────┬──────────────────┘
             │ LP burn completes (lp_ops_count >= settle_cycle)
             ▼
  ┌─────────────────────────────┐
  │  SETTLED (stage = 1)        │
  │  Rankings computed           │
  │  Reward pools locked         │
  │  Claims open                 │
  └──────────┬──────────────────┘
             │ All claims processed, next bet starts new faction war
             ▼ (loop)
```
