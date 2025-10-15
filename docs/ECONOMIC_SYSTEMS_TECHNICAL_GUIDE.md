# 💰 DogeBTC Economic Systems: Technical Deep Dive

> **For Developers & Economists** | Version 1.0.0 | October 15, 2025

---

## Table of Contents

1. [Dynamic Emission System](#dynamic-emission-system)
2. [Protocol Owned Liquidity (POL)](#protocol-owned-liquidity-pol)
3. [Staking Mathematics](#staking-mathematics)
4. [Mining Distribution Math](#mining-distribution-math)
5. [Loot System Economics](#loot-system-economics)
6. [Economic Attack Vectors](#economic-attack-vectors)
7. [Parameter Optimization](#parameter-optimization)

---

## 1. Dynamic Emission System

### Overview

The DBTC emission rate **automatically adjusts every 8 hours** based on market price movements from the Raydium pool. This creates a self-regulating supply mechanism that responds to demand.

### Price Oracle Implementation

```rust
pub struct DogeBtcMining {
    // ... other fields
    raydium_pool_state: Pubkey,           // AMM pool address
    price_history: Vec<PriceEntry>,       // 8 entries (1 per hour)
    avg_price_8h: u64,                    // Current cycle average
    prev_avg_price_8h: u64,               // Previous cycle average
    current_dist_rate: u64,               // Current emission per slot
    last_rate_update: i64,                // Last update timestamp
    slots_for_swap: u64,                  // Slots per swap (default: 9000)
}

pub struct PriceEntry {
    timestamp: i64,
    price: u64, // SOL per DBTC (scaled by 10^9)
}
```

### Hourly Update Process

```
Function: update_dbtc_dist_per_slot()
Called: Once per hour (anyone can call, automated backend recommended)

Flow:
1. Check 1 hour has passed since last update
   ├─→ If < 1 hour: Exit early (no-op)
   └─→ If ≥ 1 hour: Continue

2. Calculate DBTC amount for this hour
   ├─→ dbtc_amount = current_dist_rate × slots_for_swap
   └─→ Default: ~9,000 slots/hour

3. Swap DBTC → SOL via Raydium
   ├─→ Transfer DBTC to pool
   ├─→ Receive SOL
   ├─→ Record exact SOL received
   └─→ Store in WSOL token account

4. Fetch current price from Raydium pool
   ├─→ Read pool state
   ├─→ Calculate: price = sol_reserves / dbtc_reserves
   └─→ Add to price_history with timestamp

5. Accumulate SOL for POL
   ├─→ sol_for_pol += sol_received
   └─→ Held in WSOL account until cycle completes

6. If 8 hours completed (8 price entries):
   ├─→ Calculate weighted average price
   ├─→ Compare with previous cycle
   ├─→ Adjust emission rate
   ├─→ Execute POL (add liquidity + burn LP)
   ├─→ Clear price_history
   └─→ Start new cycle
```

### Weighted Average Calculation

```rust
// Weights: Recent prices matter more
weights = [1, 2, 3, 4, 5, 6, 7, 8]
// Hour 0 gets weight 1, Hour 7 gets weight 8

weighted_sum = 0
total_weights = 0

for (i, entry) in price_history.iter().enumerate() {
    weight = i + 1
    weighted_sum += entry.price × weight
    total_weights += weight
}

avg_price_8h = weighted_sum / total_weights
// total_weights = 1+2+3+4+5+6+7+8 = 36
```

**Example:**
```
Hour 0: 0.001 SOL/DBTC × weight 1 = 0.001
Hour 1: 0.0012 SOL × weight 2 = 0.0024
Hour 2: 0.0011 SOL × weight 3 = 0.0033
Hour 3: 0.0013 SOL × weight 4 = 0.0052
Hour 4: 0.0014 SOL × weight 5 = 0.007
Hour 5: 0.0015 SOL × weight 6 = 0.009
Hour 6: 0.0016 SOL × weight 7 = 0.0112
Hour 7: 0.0017 SOL × weight 8 = 0.0136

Sum = 0.053
Avg = 0.053 / 36 = 0.00147 SOL/DBTC

(Recent prices have more influence!)
```

### Emission Rate Adjustment

```rust
// Compare current vs previous 8-hour average
if avg_price_8h > prev_avg_price_8h {
    // Price INCREASED
    current_dist_rate = current_dist_rate × 101 / 100  // +1%
    msg!("📈 Price up, emission +1%")
    
} else if avg_price_8h < prev_avg_price_8h {
    // Price DECREASED  
    current_dist_rate = current_dist_rate × 97 / 100  // -3%
    msg!("📉 Price down, emission -3%")
    
} else {
    // Price UNCHANGED
    msg!("➡️ Price stable, emission unchanged")
}

// Save for next cycle
prev_avg_price_8h = avg_price_8h
```

### Why Asymmetric Response? (+1% / -3%)

**Economic Reasoning:**
- **Price increases**: Gradual emission increase (+1%) prevents oversupply
- **Price decreases**: Aggressive emission cut (-3%) quickly reduces selling pressure
- **Result**: System favors price stability and upward movement

**Simulation:**

```
Starting emission: 1,000 DBTC/slot
Price scenario: Continuous decline

Cycle 1: Price down → 1,000 × 0.97 = 970 (-3%)
Cycle 2: Price down → 970 × 0.97 = 940.9 (-3%)
Cycle 3: Price down → 940.9 × 0.97 = 912.7 (-3%)
Cycle 4: Price down → 912.7 × 0.97 = 885.3 (-3%)

After 4 cycles (32 hours): 11.5% emission reduction
This creates strong buy pressure (supply shock)

Cycle 5: Price up → 885.3 × 1.01 = 894.2 (+1%)
Cycle 6: Price up → 894.2 × 1.01 = 903.1 (+1%)

Gradual recovery prevents overshooting
```

### Admin Override

```rust
// Admin can force specific LP amount for POL
update_dbtc_dist_per_slot(lp_token_amount: u64)

if lp_token_amount > 0 {
    // Verify authority
    require!(authority == global_config.ext_authority);
    
    // Use specified amount instead of automatic calculation
    // Useful for emergency adjustments
}
```

---

## 2. Protocol Owned Liquidity (POL)

### What is POL?

**Traditional Liquidity:**
- Users provide liquidity
- Users can withdraw anytime
- Creates sell pressure risk

**Protocol Owned Liquidity:**
- Protocol adds liquidity from emissions
- LP tokens are BURNED (permanent)
- Liquidity can NEVER be removed
- Creates permanent price floor

### Implementation

```rust
pub struct ProtocolOwnedLiquidity {
    total_lp_burnt: u64,        // Cumulative LP tokens burned
    total_sol_added: u64,       // Cumulative SOL added
    total_dbtc_added: u64,      // Cumulative DBTC added
    lp_operations_count: u32,   // Number of POL cycles
}
```

### 8-Hour POL Cycle

```
Step 1: SWAP (every hour during 8-hour cycle)
    ├─→ Calculate: dbtc_amount = current_dist_rate × slots_for_swap
    ├─→ Transfer DBTC to Raydium pool (via CPI)
    ├─→ Receive SOL in WSOL token account
    ├─→ Accumulate: sol_for_pol += sol_received
    └─→ Price recorded for oracle

Step 2: ACCUMULATE (hours 1-7)
    └─→ SOL builds up in WSOL account

Step 3: ADD LIQUIDITY (at hour 8)
    ├─→ total_sol = sol_for_pol + latest_swap_sol
    ├─→ Calculate balanced DBTC amount for pool
    ├─→ Transfer both DBTC and WSOL to pool
    ├─→ Raydium pool mints LP tokens
    └─→ LP tokens received in program's LP account

Step 4: BURN LP TOKENS (immediately after)
    ├─→ Burn ALL LP tokens received
    ├─→ Update pol_stats:
    │   ├─→ total_lp_burnt += lp_minted
    │   ├─→ total_sol_added += sol_consumed
    │   └─→ total_dbtc_added += dbtc_consumed
    ├─→ Emit LpTokensBurned event
    └─→ Verify burn success (balance check)

Step 5: RESET CYCLE
    ├─→ Clear price_history
    ├─→ sol_for_pol = 0 (reset)
    └─→ Ready for next 8-hour cycle
```

### POL Economics

#### Growth Projection

```
Assumptions:
- current_dist_rate = 1,000 DBTC/slot
- slots_for_swap = 9,000 (per hour)
- DBTC price = 0.001 SOL

Hourly swap:
dbtc_swapped = 1,000 × 9,000 = 9,000,000 DBTC
sol_received = 9,000,000 × 0.001 = 9,000 SOL

8-hour cycle:
total_dbtc = 72,000,000 DBTC
total_sol = 72,000 SOL

If balanced add (50/50 value):
dbtc_for_lp = ~36,000,000 DBTC
sol_for_lp = ~36,000 SOL

LP tokens minted: ~√(36M × 36K) ≈ 36,000,000 LP
All burned immediately!

Per day (3 cycles):
POL growth = 108,000 SOL + 108M DBTC in permanent liquidity
```

#### Long-Term Impact

```
After 1 month (90 cycles):
POL = 3,240,000 SOL + 3.24B DBTC permanently locked

After 6 months:
POL = 19,440,000 SOL + 19.44B DBTC

This creates an UNBREAKABLE price floor:
- Liquidity cannot be removed
- Price has permanent support
- Sell pressure absorbed by deep liquidity
```

### Safety Checks

```rust
// Before LP burn, verify minting occurred
lp_balance_before = check_lp_account()
lp_balance_after = check_lp_account()

require_eq!(
    lp_balance_final,
    lp_balance_before,
    "LP burn verification failed"
);

// Ensure LP tokens were actually burned
```

---

## 3. Staking Mathematics

### Time-Weighted Multipliers

#### Formula

```rust
fn calculate_multiplier(
    lockup_days: u64,
    min_lockup: u64,  // 7 days
    max_lockup: u64,  // 1,095 days (3 years)
    base_mult: u16,   // 100 (1.0x)
    max_mult: u16,    // 900 (9.0x)
) -> u16 {
    // Linear interpolation
    let range = max_lockup - min_lockup
    let position = lockup_days - min_lockup
    let mult_range = max_mult - base_mult
    
    base_mult + (mult_range × position / range)
}
```

#### Multiplier Table

| Lockup Days | Multiplier | Weighted (per 1000 DBTC) |
|-------------|------------|--------------------------|
| 7 | 100 (1.0x) | 1,000 |
| 30 | 119 (1.19x) | 1,190 |
| 90 | 176 (1.76x) | 1,760 |
| 180 | 291 (2.91x) | 2,910 |
| 365 | 391 (3.91x) | 3,910 |
| 730 | 645 (6.45x) | 6,450 |
| 1095 | 900 (9.0x) | 9,000 |

**Key Insight:** 3-year lockup gives 9x more voting power and rewards than 1-week!

### Weighted Amount Calculation

```rust
// User stakes 10,000 DBTC for 365 days
staked_amount = 10,000
lockup_days = 365
multiplier = 391 (from table)

// Calculate weighted amount
weighted_amount = staked_amount × multiplier / 100
weighted_amount = 10,000 × 391 / 100
weighted_amount = 39,100 weighted points
```

### Electricity Generation

```rust
// Set by admin (example values)
electricity_per_weighted_moondoge = 1,000

// User's electricity from this position
electricity = weighted_amount × electricity_per_weighted_moondoge
electricity = 39,100 × 1,000
electricity = 39,100,000 units

// This electricity is added to user's moonbase
// via CPI: moonbase::update_user_electricity()
```

### SOL Rewards (Precision Math)

#### Global Accumulator

```rust
pub struct DogeBtcVault {
    accumulated_sol_per_point: u128,  // Precision: 1,000,000
    weighted_dbtc_locked: u64,        // Total weighted points
    total_sol_distributed: u64,       // Historical tracking
}

PRECISION_FACTOR: u128 = 1,000,000
```

#### SOL Distribution

```
Admin adds 100 SOL to vault
total_weighted = 1,000,000 points

new_sol_per_point = (100 × 10^9 × 1,000,000) / 1,000,000
                  = 100,000,000,000,000 / 1,000,000
                  = 100,000,000,000

accumulated_sol_per_point += 100,000,000,000
```

#### User Claiming

```rust
pub struct UserMoonElectricity {
    total_weighted_moondoge: u64,
    moondoge_reward_debt: u128,      // Last checkpoint
    pending_moondoge_rewards: u64,   // Unclaimed
}

// Calculate pending rewards
fn calculate_pending() -> u64 {
    let earned = (user.total_weighted_moondoge as u128 
        × vault.accumulated_sol_per_point) / PRECISION_FACTOR
    
    earned - user.moondoge_reward_debt
}
```

**Example:**
```
User weighted: 39,100 points (3.91% of total)
accumulated_sol_per_point: 100,000,000,000
reward_debt: 0 (first time)

earned = (39,100 × 100,000,000,000) / 1,000,000
       = 3,910,000,000,000,000 / 1,000,000
       = 3,910,000,000 lamports
       = 3.91 SOL

User receives: 3.91 SOL (exactly 3.91% of 100 SOL added!)
reward_debt = 100,000,000,000 (set checkpoint)
```

#### Incremental Updates

```
More SOL added (50 SOL):
new_per_point = 50B × 1M / 1,000,000 = 50,000,000,000
accumulated = 100B + 50B = 150,000,000,000

User pending (from last checkpoint):
earned_since = (39,100 × 150B) / 1M - 100B
             = 5,865B / 1M - 100B
             = 5,865,000,000 - 3,910,000,000
             = 1,955,000,000 lamports
             = 1.955 SOL (3.91% of 50 SOL, perfect!)
```

### Early Withdrawal Penalty

```rust
// Calculate time remaining
remaining_seconds = lockup_end_timestamp - current_timestamp
total_duration = lockup_end_timestamp - start_timestamp
remaining_pct = (remaining_seconds × 100) / total_duration

// Calculate penalty
emergency_tax = 10% (configurable)
penalty_pct = emergency_tax × remaining_pct / 100

// Apply penalty
penalty_amount = staked_amount × penalty_pct / 100
return_amount = staked_amount - penalty_amount

// Penalty is BURNED (for DBTC) or sent to treasury (for LP)
```

**Example:**
```
Staked: 10,000 DBTC for 365 days
Unstake after 100 days (265 days remaining)

remaining_pct = 265 / 365 × 100 = 72.6%
penalty_pct = 10% × 72.6% / 100 = 7.26%
penalty = 10,000 × 7.26% = 726 DBTC

User receives: 9,274 DBTC
Burned: 726 DBTC
```

**Economic Effect:** Discourages early exits, reduces circulating supply

---

## 4. Mining Distribution Math

### Index-Based Distribution

**Problem:** How to fairly distribute tokens to thousands of users without iterating through them?

**Solution:** Global index tracking + per-user checkpoints

#### Global State

```rust
pub struct DogeBtcMining {
    dbtc_tokens_minted_per_hashpower: u128,  // Global accumulator
    total_active_hashpower: u64,             // Sum of all users
    last_slot: u64,                          // Last update slot
    current_dist_rate: u64,                  // Tokens per slot
}
```

#### Per-User State

```rust
pub struct UserMoonBaseInstance {
    active_hashpower: u64,     // User's contribution
    dbtc_claim_index: u128,    // User's last checkpoint
    claimable_dbtc: u64,       // Pending tokens
}
```

### Mathematical Proof

**Invariant:** Each user receives tokens proportional to their hashpower share

```
Total tokens mined in period T:
tokens = slots × dist_rate

User share:
user_share = (user_hashpower / total_hashpower) × tokens

Index increment:
Δindex = (tokens × SCALE) / total_hashpower

User claimable:
claimable = (Δindex × user_hashpower) / SCALE

Substitution:
claimable = (tokens × SCALE / total_hashpower × user_hashpower) / SCALE
claimable = tokens × user_hashpower / total_hashpower ✓

Perfect proportional distribution!
```

### Overflow Prevention

```rust
// ❌ UNSAFE (can overflow)
let index_inc = (new_tokens × MAX_SAFE_U64) / total_hashpower;

// ✅ SAFE (u128 intermediate math)
let index_inc = ((new_tokens as u128)
    .saturating_mul(MAX_SAFE_U64 as u128)
    .saturating_div(total_hashpower as u128));

MAX_SAFE_U64 = u64::MAX / 1,000,000
             = 18,446,744,073,709,551,615 / 1,000,000
             = 18,446,744,073,709
```

**Why This Works:**
- u128 max = 340,282,366,920,938,463,463,374,607,431,768,211,455
- new_tokens max = ~10^15 (reasonable upper bound)
- MAX_SAFE_U64 = ~10^13
- Product = ~10^28 (fits easily in u128)

### Edge Cases

#### Zero Hashpower
```rust
if doge_btc_mining.total_active_hashpower == 0 {
    return Ok(()); // No distribution, no crash
}
```

#### User Claims Before First Mine
```rust
if user.dbtc_claim_index == 0 {
    // Initialize to current global index
    user.dbtc_claim_index = doge_btc_mining.dbtc_tokens_minted_per_hashpower;
    return Ok(()); // No tokens yet
}
```

#### Hashpower Changes

```rust
// CRITICAL: Mine pending tokens BEFORE changing hashpower!

// ❌ WRONG
user.active_hashpower += new_module_hashpower; // Lost rewards!

// ✅ CORRECT
mine_dbtc_for_user(user, doge_btc_mining)?;  // Claim pending first
user.active_hashpower += new_module_hashpower; // Then update
```

**This is why `mine_dbtc_for_user()` is called before:**
- install_module
- remove_module
- upgrade_module
- Any hashpower change!

---

## 5. Loot System Economics

### Accumulation Rate

```rust
// 10% of all mining rewards
mining_output = 1,000,000 DBTC/day
loot_accumulation = 100,000 DBTC/day (10%)

// 10% of all SOL fees
sol_collected = 100 SOL/day
loot_accumulation = 10 SOL/day (10%)
```

### Distribution Rate (Expected Value)

**Assumptions:**
- 1,000 active users
- Average level: 15
- Normal distribution across levels

**Daily Loot Events:**
```
Minor wins (levels 1-14): ~50 events/day
├─→ Avg payout: 0.1 SOL + DBTC
└─→ Daily cost: 5 SOL

Rare wins (levels 15-24): ~15 events/day
├─→ Avg payout: 2 SOL + DBTC
└─→ Daily cost: 30 SOL

Legendary wins (levels 25+): ~5 events/day
├─→ Avg payout: 5 SOL + DBTC
└─→ Daily cost: 25 SOL

Milestones: ~20 events/day
├─→ Avg payout: 1 SOL + DBTC
└─→ Daily cost: 20 SOL

Jackpots: ~0.2 events/day (rare!)
├─→ Avg payout: 500 SOL
└─→ Daily cost: 100 SOL

TOTAL: ~180 SOL/day distributed
```

**Sustainability Check:**
```
Accumulation: 10 SOL/day
Distribution: 180 SOL/day
Deficit: -170 SOL/day

This requires pre-funding or adjustment!
```

### Recommended Adjustments

**Option 1: Reduce Vault Cuts**
```
Instead of 1%, 5%, 8%:
Use: 0.1%, 0.5%, 0.8%

New daily cost: ~18 SOL/day
More sustainable with 10 SOL/day accumulation
Requires vault pre-seeding: ~500-1,000 SOL
```

**Option 2: Increase Accumulation**
```
Instead of 10%:
Use: 20% loot allocation

Accumulation: 20 SOL/day
Distribution: 180 SOL/day
Still requires pre-seeding but more sustainable
```

**Option 3: Dynamic Vault Cuts**
```
vault_cut_multiplier = min(
    1.0,
    current_vault_balance / target_vault_balance
)

If vault is low:
- Reduce payouts proportionally
- Prevents depletion
- Auto-adjusts to accumulation rate
```

### Jackpot Economics

**Fixed pots create predictable costs:**

```
Jackpot probability: 0.20% base
Trigger frequency: Levels 10, 20, 30, 40...

Expected cost per 1,000 players:
- Reaching level 10: ~950 players
- 0.20% chance × 950 = ~2 jackpots
- Average pot: ~600 SOL
- Cost: ~1,200 SOL

This is FRONT-LOADED spending
(Most wins at early levels)

After level 30: Very few players
- Reaching level 30: ~50 players
- 0.20% × 50 = 0.1 jackpots
- Rare but massive when they hit!
```

**Vault Seeding Recommendation:**
```
Initial seed: 2,000-5,000 SOL
Purpose: Cover early jackpots and high payouts
Replenishes: Over time from 10% accumulation
Timeline: 6-12 months to self-sustain
```

---

## 6. Economic Attack Vectors

### ❌ Sybil Attack (Creating Many Accounts)

**Attack:** Create 1,000 moonbases to farm loot

**Defense:**
- ✅ Each moonbase costs 0.5-1.42 SOL (expensive)
- ✅ Loot based on LEVEL not account count
- ✅ Leveling requires significant time/effort
- ✅ Early levels have low loot rates
- **Result:** Not profitable, costs exceed gains

### ❌ Flash Loan Attack (Temporary Hashpower)

**Attack:** Borrow DBTC, stake, claim, unstake in one TX

**Defense:**
- ✅ Mining uses index-based distribution (time-dependent)
- ✅ Cannot claim tokens not yet mined
- ✅ Staking has lockup periods
- ✅ Early unstake = 10% penalty
- **Result:** Impossible with current design

### ❌ Price Oracle Manipulation

**Attack:** Manipulate Raydium pool price to game emissions

**Defense:**
- ✅ 8-hour weighted average (hard to sustain)
- ✅ Recent prices weighted more (but still averaged)
- ✅ Asymmetric response (+1%/-3%) limits impact
- ✅ POL grows over time (harder to manipulate)
- **Result:** Extremely expensive, likely unprofitable

### ❌ Loot Farming

**Attack:** Create many accounts, level to 5, farm milestones

**Defense:**
- ✅ High SOL cost per moonbase (0.5+ SOL)
- ✅ XP requirement for level 5: ~1,740 XP
- ✅ Milestone reward: 0.5% of vault
- ✅ If vault is 100 SOL: 0.5 SOL payout
- **Cost:** 0.5 SOL + time to reach level 5
- **Gain:** 0.5 SOL (if vault has 100 SOL)
- **Result:** Breakeven at best, not profitable

### ❌ Dragon Egg Exploit

**Attack:** Incubate egg, get power, remove, sell, repeat

**Defense:**
- ✅ NFT physically transferred to custody PDA (cannot sell)
- ✅ Power only increases while incubated
- ✅ Removing resets incubation (must re-lock)
- **Result:** Impossible, NFT is truly locked

### ✅ Legitimate Strategies (Intended Gameplay)

**Optimize Staking:**
- Long lockups for max multiplier
- Stake both DBTC and LP for dual rewards
- **Result:** Rewarded with electricity + SOL

**Upgrade Focus:**
- Prioritize high-ROI modules
- Level up for better upgrades
- **Result:** Higher hashpower = more DBTC

**Early Achievement:**
- Rush to high levels first
- Get exclusivity bonuses
- **Result:** 2x vault rewards for pioneers

**Referral Network:**
- Build community
- Earn 15% of spending
- **Result:** Passive SOL income + XP

---

## 7. Parameter Optimization

### Emission Rate Tuning

```rust
// Initial setting
doge_btc_per_slot = 1,000 DBTC

// Considerations:
Total supply: 21,000,000,000 DBTC (21 billion)
Target distribution: 80% to mining (16.8 billion)
Expected hashpower: 1,000,000 hash/s average
Slots per day: 216,000 (2.5 slots/sec × 86,400 sec)

Daily emissions: 1,000 × 216,000 = 216,000,000 DBTC/day
Yearly emissions: 78,840,000,000 DBTC/year

Problem: This exceeds total supply in < 1 year!

Recommendation:
doge_btc_per_slot = 100 DBTC (10x reduction)
Daily: 21,600,000 DBTC
Yearly: 7,884,000,000 DBTC (~37% of supply)
Duration: ~2-3 years to distribute mining allocation
```

### Electricity Conversion Rates

```rust
// electricity_per_weighted_moondoge = X
// electricity_per_weighted_lp_tokens = Y

// Module consumption examples:
Mining Module:    50,000 units/hour
Attraction:       30,000 units/hour

// User staking example:
1,000 DBTC, 30-day lockup
weighted = 1,190
electricity = 1,190 × X

// For user to run 10 mining modules:
needed_electricity = 10 × 50,000 = 500,000
required_weighted = 500,000 / X

If X = 1,000:
required_weighted = 500
required_dbtc ≈ 420 DBTC (with 1.19x mult)

If X = 100:
required_weighted = 5,000
required_dbtc ≈ 4,200 DBTC

Recommendation: X = 500-1,000 (balanced)
```

### Loot Vault Percentage

```rust
// Current: 10% of mining/fees → loot

// Alternative scenarios:

5% allocation:
- More sustainable
- Slower vault growth
- Smaller payouts
- Longer to reach jackpot threshold

15% allocation:
- Faster vault growth
- Larger payouts
- Reaches jackpot sooner
- May reduce user mining rewards

20% allocation:
- Very fast growth
- Massive payouts possible
- Significant mining reward reduction
- Higher engagement but less mining incentive

Recommendation: Start at 10%, adjust based on metrics
```

### Jackpot Pot Sizes

```rust
// Current pots: [1000, 750, 690, 510, 420] SOL

// Scaling considerations:
If DBTC price = $0.10 and SOL = $100:
1,000 SOL = $100,000 jackpot!

If DBTC price = $0.01 and SOL = $100:
1,000 SOL = $100,000 jackpot
(Same USD value!)

// Pots scale with SOL price automatically
// No adjustment needed for token price changes

// Alternative smaller pots for lower-budget:
[100, 75, 50, 25, 10] SOL
- More frequent hits
- Lower individual wins
- More players can participate

// Alternative massive pots for whale games:
[10000, 5000, 2500, 1000, 500] SOL
- Rare mega-events
- Requires huge vault
- Creates massive FOMO
```

---

## 🎯 Economic Model Summary

### Strengths

✅ **Self-Regulating Emissions**
- Price-responsive supply
- Automatic rebalancing
- No manual intervention needed

✅ **Permanent Liquidity**
- POL grows forever
- Unbreakable price floor
- Reduces volatility over time

✅ **Deflationary Pressure**
- 1% transfer tax burned
- Early unstake penalties burned
- Loot burns 10% of mining output

✅ **Dual-Token Loot**
- SOL + DBTC rewards
- Increases DBTC utility
- Reduces sell pressure

✅ **Time-Weighted Staking**
- Rewards long-term holders
- Punishes short-term speculation
- Up to 9x rewards for 3-year locks

### Potential Issues

⚠️ **Loot Vault Sustainability**
- Current parameters may drain vaults
- Requires initial seeding (2,000-5,000 SOL)
- Monitor accumulation vs distribution ratio
- **Fix:** Reduce vault_bp percentages or increase loot_percentage

⚠️ **Emission Rate May Be Too High**
- 1,000 DBTC/slot = 78B DBTC/year
- Exceeds total supply
- **Fix:** Reduce to 50-100 DBTC/slot

⚠️ **Electricity May Be Too Cheap**
- Low staking requirement for many modules
- Could lead to oversupply of modules
- **Fix:** Reduce electricity_per_weighted or increase module consumption

⚠️ **Dragon Egg Power Has No Utility**
- Power accumulates but does nothing
- **Fix:** Implement power-based bonuses (hashpower boost, loot multiplier, etc.)

### Recommended Adjustments

```rust
// Mining emissions
doge_btc_per_slot: 100 (reduce from 1,000)

// Loot vault cuts
minor_vault_bp: 10-50 (reduce from 100)
rare_vault_bp: 50-200 (reduce from 500)
legendary_vault_bp: 100-400 (reduce from 800)

// Electricity conversion
electricity_per_weighted_moondoge: 500-1,000

// Dragon Egg utility
power_hashpower_bonus = egg.power / 1,000
// 10,000 power = +10 hashpower bonus
// 100,000 power = +100 hashpower bonus
```

---

## 📊 Monitoring Dashboard Metrics

### Real-Time Metrics

```
Mining System:
- current_dist_rate
- total_active_hashpower
- total_tokens_mined
- avg_price_8h
- Price trend (last 24 hours)

POL System:
- total_lp_burnt (cumulative)
- total_sol_added (cumulative)
- total_dbtc_added (cumulative)
- lp_operations_count
- POL growth rate

Loot System:
- loot SOL vault balance
- loot DBTC vault balance
- loot accumulation rate
- loot distribution rate
- Vault sustainability ratio

Staking System:
- Total DBTC staked
- Total LP staked
- Average multiplier
- Total electricity generated
- SOL rewards pending
```

### Alert Thresholds

```
🔴 CRITICAL:
- Loot vault < 10 SOL (cannot pay out)
- Emission rate < 10 DBTC/slot (too low)
- POL growth < 0 (not adding liquidity)

🟡 WARNING:
- Loot distribution > accumulation for 7 days
- Price declining for 3 cycles (24 hours)
- Staking TVL down 50% in 30 days

🟢 HEALTHY:
- Loot vault > 100 SOL
- Price stable or rising
- POL growing steadily
- Staking TVL increasing
```

---

**This economic system is complex but well-designed. The main risks are loot vault sustainability and emission rates. With the recommended adjustments, it can be highly sustainable and engaging for years.**


