# Economy Cycle: How It Works

The economy cycle is the heartbeat of the MineBTC token economy. It runs continuously
in ~4-hour loops, adjusting the dogeBTC emission rate based on market price and
permanently locking liquidity (POL) on every cycle.

## The Three Steps

```
Step 1: snapshot_price      (×8, every 30 min, anyone can crank)
Step 2: update_rate          (once after 8 snapshots, anyone can crank)
Step 3: add_lp_and_burn      (once after rate update, anyone can crank)
```

Each cycle can settle the faction-war leaderboard (story-event scores).

---

## Step 0: Fee Accumulation (distribute_sol_fees)

**When:** Called periodically to flush the SOL treasury.

**Flow:**
1. Read SOL treasury balance (accumulated from bet fees)
2. Split by `buyback_pct` (default 80%):
   - 80% → buybacks_sol_vault (for price snapshots + POL)
   - 20% → fee_recipient via WSOL (dev earnings)
3. Buybacks vault balance grows over time as bets happen

**Edge cases:**
- If treasury balance ≤ rent-exempt minimum → returns Ok, no-op
- If buyback amount = 0 → skips transfer, no error

---

## Step 1: snapshot_price (×8, every 30 minutes)

**What it does:**
1. Takes 10% of available SOL from buybacks vault
2. Swaps that SOL → dogeBTC via Raydium (price discovery)
3. Records the price in `price_history` (up to 8 entries)
4. Earmarks another 10% of available SOL for POL (Protocol Owned Liquidity)
5. Computes running weighted average price (later entries weighted more)

**Available SOL calculation:**
```
available = buybacks_vault_balance - rent_exempt - already_earmarked_pol
```

**Per snapshot allocation:**
```
sol_for_swap       = available / 10   (10% → buy dogeBTC for price)
sol_for_pol        = available / 10   (10% → earmarked for LP later)
remaining 80%      stays in vault for future snapshots
```

Over 8 snapshots: ~80% of available SOL at snapshot 1 gets consumed
(each snapshot sees a smaller "available" because prior snapshots took 20%).

**Price formula:**
```
price = (sol_swapped × 10^9) / minebtc_received
```
Stored as u64 with 9-decimal precision (SOL per dogeBTC).

**Weighted average:** weights 1-8 (earliest=1, latest=8).
Later snapshots count more, reflecting more recent market conditions.

**Guards:**
- `lp_operation_pending == false` (can't snapshot during LP phase)
- `price_history.len() < 8` (must call update_rate after 8)
- `snapshot_interval` seconds since last snapshot (default 1800 = 30 min)
- Pool state validated against `global_config.raydium_pool_state`
- If available SOL = 0 → returns Ok, no-op (won't revert)

**Edge case: swap returns 0 dogeBTC:**
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
   - Price up → increase `mine_btc_per_round` by `emission_increase_pct` (default 1%)
   - Price down → decrease `mine_btc_per_round` by `emission_decrease_pct` (default 3%)
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
2. Calculates how much dogeBTC is needed to match the SOL at pool ratio
3. Deposits SOL + dogeBTC into Raydium pool → receives LP tokens
4. Burns ALL LP tokens received (permanent liquidity lock)
5. Updates `pol_stats` (cumulative POL metrics)
6. Clears `lp_operation_pending` flag (unblocks next cycle of snapshots)
7. Increments `pol_stats.lp_operations_count` (triggers epoch settlement)

**LP amount calculation:**
```
lp_from_sol = (available_sol × lp_supply) / sol_vault_balance
required_minebtc = (lp_from_sol × minebtc_vault_balance) / lp_supply
```
Plus 2% buffer for slippage, plus Token-2022 transfer fee gross-up.

**dogeBTC limit (prevents draining the mining vault):**
If the required dogeBTC exceeds 1% of available dogeBTC in the mining vault,
the SOL amount is reduced proportionally to match the 1% dogeBTC cap.
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
- `InsufficientTokensInVault` if dogeBTC can't cover the deposit
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
┌───────────────┐     ┌──────────────────┐
│ buybacks_vault │     │  fee_recipient   │
│ (80% of fees)  │     │  (20% dev share) │
└───────┬───────┘     └──────────────────┘
        │
        ▼ (snapshot_price ×8)
┌───────────────┐     ┌──────────────────┐
│ 10% swap SOL  │────►│  dogeBTC bought  │
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
│ dogeBTC from  │     │  (LP tokens)     │
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

2. **No dogeBTC drain:** LP operations capped at 1% of mining vault per cycle.
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

# Faction War Cycle: Story Event Leaderboard

Each economy cycle (LP burn) can settle the active **Faction War** — the
competitive cycle where Doge story events determine country rankings and
distribute dogeBTC rewards.

Product note: the contract may still mutate Doge DNA for Evolution / Power /
Trait events, but the user-facing primitive is broader than image mutation.
Backends should treat these as **story events** that can become art, reels,
character-history entries, or simple indexed gameplay beats.

## How It Works

```
Players bet SOL in 60-second rounds
    │
    ├─ Story events fire (limited by global budget + per-faction penalty)
    │   └─ Each event adds score: type_weight × bet_size × multiplier
    │
    ├─ story scores accumulate on FactionWarState per faction
    │
    └─ total_dogebtc_mined_in_faction_war grows with each round
```

When the economy cycle completes (LP burn → `lp_operations_count` increments):

```
finalize_faction_war_settlement():
    1. Rank factions by story score, then round wins, then own-faction SOL support
    2. Compare with start_ranks → compute rank deltas → resolve directions
    3. Split faction_war_mining_pool:
       - base pool: anyone who picked a country's final direction correctly
       - loyalty pool: own-country correct-direction supporters
       - Doge pool: users whose gameplay Doge triggered a story event
    4. Stage = 1 (claims open)
    5. Persist final_ranks as next faction war's start_ranks
    6. Advance current_faction_war_id
```

## Reward Pools

```
faction_war_mining_pool (total dogeBTC mined in all rounds this cycle)
    │
    ├─ base_reward_bps → faction_reward_pools
    │         Users who bet any country's final direction correctly get pro-rata share
    │
    ├─ loyalty_reward_bps → loyalty_reward_pools
    │         Own-country correct-direction supporters get pro-rata share
    │
    └─ doge_reward_bps → faction_doge_reward_pools
              Story-event eligible Doges get accumulated_val (claimable via burn)

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

For SOL bets: a 1.0x doge gives wgtd_points = points. Gameplay Doge power is capped at 4.2x.
For ticket bets: wgtd_points = points (no multiplier). Tickets are capped at 25% of SOL volume per round.

Weighted points determine:
- Your share of dogeBTC round rewards (winner pool)
- Your share of dogeBTC faction-war rewards (correct direction pool)
- Story score contribution to your country

### XP System

XP accumulates on the gameplay doge during betting:

```
xp_gained = total_sol_bet / 1_000_000  (1 XP per 0.001 SOL)
```

XP is **always gained** on SOL bets, even without a story event.

**XP effects on story events:**
- Evolution: XP contributes 5-10% as multiplier boost (then XP resets to 0)
- Power/Trait: XP contributes 2-5% as multiplier boost (XP preserved)

**XP is cached** on PlayerData during gameplay and synced back to DogeMetadata on:
- Round reward claim (process_mutation_sync)
- Gameplay doge withdrawal

### Story Event System

**Trigger conditions** (ALL must be true):
1. RPG progression enabled
2. SOL bet (not tickets)
3. No prior story event this round for this user
4. Gameplay doge is active
5. Global story-event budget not exhausted (max = active_factions / 3 per round)

**Probability:**
```
base_chance = 20%  (MAX_BASE_CHANCE = 2000 bps)
bet_strength = user_bet / highest_bet_on_faction  (0-100%)
mult_penalty = BASE_MULTIPLIER / active_multiplier  (100% at 1x, ~24% at 4.2x)
faction_penalty = 10000 / (10000 + prior_events × 5000)  (100%, 67%, 50%, 40%...)

final_chance = base × bet_strength × mult_penalty × faction_penalty × volume × cooldown × pacing
```

**Story event types** (when triggered):
- Evolution (~10% / (gen+1)): +50 base multiplier, guaranteed DNA upgrades, XP resets
- Power (~30%): +25 base multiplier, power trait upgrade
- Trait (~60%): +5 base multiplier, visual trait upgrade

Each story event also boosts multiplier by `base + (XP × efficiency_pct / 100)`.

**Story score** (added to country's faction-war leaderboard):
```
score = type_weight × total_sol_bet × active_multiplier / BASE / PRECISION
weights: Evolution=100, Power=30, Trait=10
```

### Doge accumulated_val

When claiming round rewards, the gameplay doge earns dogeBTC based on story event type:

| Story event | % of round dogeBTC reward |
|----------|--------------------------|
| Evolution | 6.9% |
| Power | 4.2% |
| Trait | 3.0% |
| No event | 1.0% |

This accumulates on `doge_metadata.accumulated_val` and can only be claimed
by burning the doge (`send_to_heaven`).

### Gameplay Doge Lifecycle

```
1. use_doge_for_gameplay
   └─ Lock NFT in custody, cache stats to PlayerData

2. Play rounds (bets trigger story events, XP accumulates)

3. request_doge_gameplay_unlock
   └─ Sets unlock_request_faction_war = current_faction_war_id
   └─ Must wait until next faction war to withdraw

4. withdraw_doge_from_gameplay
   └─ Requires: next faction war started + no pending claims
   └─ Syncs DNA/XP/multiplier back to DogeMetadata
   └─ Returns NFT to user
```

The two-phase unlock prevents mid-cycle Doge swapping to farm story events.

### Round Reward Distribution

When a round ends, rewards flow to three groups:

```
mine_btc_per_round (dogeBTC emission)
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
- Doge bonus: same formula but using `eligible_doge_direction_totals` as denominator

**Referral overlay:** if a player joined through a real referral code, their
dogeBTC claim gets a 1% bonus from the emissions vault. The referrer accrues 3%
of the base claim, or 5% when both users share the same permanent country. These
overlay rewards do not reduce the round/cycle reward pools.

## Passive Staking in the Economy Loop

Passive staking is a separate accounting layer that sits downstream from rounds.

```text
economy loop sets mine_btc_per_round
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
    -> passive Doge multiplier
    -> final staking hashpower (max 9x total)
```

### Passive Doge staking

Passive Doge staking is different from gameplay-Doge locking:

- passive Doges can be from any faction
- they boost only the player's home-faction passive staking positions
- they modify `player_data.doge_multiplier`
- they do **not** directly affect gameplay `active_multiplier`

### Reward realization

SOL staking rewards:

- accrue into `pending_sol_rewards`
- transfer directly to wallet on `claim_staking_rewards`

MineBTC staking rewards:

- accrue into `pending_minebtc_rewards`
- are globally tracked through `unrefined_rewards.total_minebtc_claimable`
- are withdrawn later through `withdraw_dbtc_rewards`

### Refining redistribution

When a user withdraws pending MineBTC rewards, a refining fee can be taken and
redistributed across remaining unclaimed MineBTC balances via the global
`unrefining_index`.

This means claim timing matters:

- fast withdrawal = immediate liquidity, but pay refining fee
- slower withdrawal = can earn part of other users' refining fees

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

Rewards go to faction stakers (split 50/50 between dogeBTC and LP stakers).

## State Accounts

```
FactionWarConfig (singleton)
    ├─ current_faction_war_id: incrementing counter
    ├─ is_active: admin toggle
    ├─ faction_war_settle_cycle: LP ops count that triggers settlement
    └─ prev_faction_war_mutation_ranks: carried forward to next faction war

FactionWarState (one per faction war)
    ├─ faction_war_id, start_timestamp, stage
    ├─ total_dogebtc_mined_in_faction_war → faction_war_mining_pool
    ├─ start_ranks ↔ final_ranks → rank_deltas → resolved_directions
    ├─ faction_mutation_scores (internal story-score array that drives rankings)
    ├─ faction_direction_totals (base-pool denominator for user claims)
    ├─ loyalty_direction_totals (own-country loyalty-pool denominator)
    ├─ eligible_doge_direction_totals (denominator for doge bonus)
    ├─ faction_reward_pools (base user share)
    ├─ loyalty_reward_pools (own-country loyalty share)
    └─ faction_doge_reward_pools (Doge story-event bonus share)

UserFactionWarBets (one per user per faction war)
    ├─ direction_bets: weighted bets across countries
    ├─ loyalty_direction_bets: weighted own-country bets
    ├─ gameplay_doge: which doge became eligible
    └─ doge_bonus_eligible: did this user's doge trigger a story event?
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
  │  Story events accumulate    │
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
