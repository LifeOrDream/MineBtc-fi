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

Each cycle also triggers epoch/surge settlement (mutation leaderboard).

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

# Rebase Cycle: Mutation Leaderboard

Each economy cycle (LP burn) triggers a **rebase** — the competitive cycle where
doge mutations determine country rankings and distribute dogeBTC rewards.

## How It Works

```
Players bet SOL in 60-second rounds
    │
    ├─ Mutations fire (limited by global budget + per-faction penalty)
    │   └─ Each mutation adds score: type_weight × bet_size × multiplier
    │
    ├─ mutation_scores accumulate on RebaseState per faction
    │
    └─ total_dogebtc_mined_in_rebase grows with each round
```

When the economy cycle completes (LP burn → `lp_operations_count` increments):

```
finalize_rebase_settlement():
    1. Rank factions by faction_mutation_scores (highest = rank 0)
    2. Compare with start_ranks → compute rank deltas → resolve directions
    3. Split rebase_mining_pool:
       - 90% user pool: proportional to winning-direction bets per faction
       - 10% doge bonus pool: same split but only for users whose doge mutated
    4. Stage = 1 (claims open)
    5. Persist final_ranks as next rebase's start_ranks
    6. Advance current_rebase_id
```

## Reward Pools

```
rebase_mining_pool (total dogeBTC mined in all rounds this cycle)
    │
    ├─ 90% → faction_reward_pools (split by winning-direction bet weight)
    │         Users who bet correct direction on own faction get pro-rata share
    │
    └─ 10% → faction_doge_reward_pools (same split, doge-eligible users only)
              Goes to doge's accumulated_val (claimable via burn)
```

**User claims:**
- `user_reward = faction_pool × user_bet / total_bet` (on correct direction)
- Only own-faction bets count
- Doge bonus: same formula but using `eligible_doge_direction_totals` as denominator

## Tax Treasury Distribution (also tied to rebase)

After settlement, `claim_faction_treasury_for_rebase` distributes the faction
treasury vault (accumulated from 1% transfer tax):

```
treasury_balance
    ├─ 80% rank-weighted: rank_points = active_factions - rank
    │   Higher rank = more reward, but every faction gets something
    │
    └─ 20% lucky draw: one random faction from rank 5+ wins the whole pot
        Equal probability per eligible faction, deterministic from rebase_id
```

Rewards go to faction stakers (split 50/50 between dogeBTC and LP stakers).

## State Accounts

```
RebaseConfig (singleton)
    ├─ current_rebase_id: incrementing counter
    ├─ is_active: admin toggle
    ├─ rebase_settle_cycle: LP ops count that triggers settlement
    └─ prev_rebase_mutation_ranks: carried forward to next rebase

RebaseState (one per rebase)
    ├─ rebase_id, start_timestamp, stage
    ├─ total_dogebtc_mined_in_rebase → rebase_mining_pool
    ├─ start_ranks ↔ final_ranks → rank_deltas → resolved_directions
    ├─ faction_mutation_scores (drives rankings)
    ├─ faction_direction_totals (denominator for user claims)
    ├─ eligible_doge_direction_totals (denominator for doge bonus)
    ├─ faction_reward_pools (90% user share)
    └─ faction_doge_reward_pools (10% doge bonus share)

UserRebaseBets (one per user per rebase)
    ├─ direction_bets: weighted bets on own faction only
    ├─ gameplay_doge: which doge became eligible
    └─ doge_bonus_eligible: did this user's doge mutate?
```

## Lifecycle

```
  ┌─────────────────────────────┐
  │  IDLE (no active rebase)    │
  │  rebase_state.rebase_id = 0 │
  └──────────┬──────────────────┘
             │ First bet in new cycle (auto-start)
             ▼
  ┌─────────────────────────────┐
  │  ACTIVE (stage = 0)         │
  │  Mutations accumulate       │
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
             │ All claims processed, next bet starts new rebase
             ▼ (loop)
```
