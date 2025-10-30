# 🌙 DogeBTC MoonBase: Complete System Architecture

> **Production-Ready Documentation** | Version 1.0.0 | Last Updated: October 15, 2025

---

## 📋 Table of Contents

1. [System Overview](#system-overview)
2. [Program Architecture](#program-architecture)
3. [Economic Model](#economic-model)
4. [Module System](#module-system)
5. [Grid & Placement](#grid--placement)
6. [Mining & Distribution](#mining--distribution)
7. [Staking & Electricity](#staking--electricity)
8. [XP & Leveling](#xp--leveling)
9. [Loot System](#loot-system)
10. [Dragon Egg NFTs](#dragon-egg-nfts)
11. [Referral System](#referral-system)
12. [User Journey](#user-journey)
13. [Economic Analysis](#economic-analysis)
14. [Security & Safety](#security--safety)

---

## 1. System Overview

### 🎯 Core Concept
DogeBTC is a **lunar mining simulation game** on Solana where players:
- Build and upgrade mining facilities on the moon
- Stake DBTC tokens and LP tokens to generate electricity
- Mine DBTC tokens through hashpower accumulation
- Level up through XP to unlock rare loot rewards
- Incubate Dragon Egg NFTs that grow in power with mining output

### 🏗️ Two-Program Architecture

#### **MoonBase Program** (Core Game Logic)
- Moonbase creation and management
- Module system (mining rigs, attractions)
- Grid-based tile placement
- DBTC token mining with dynamic emissions
- XP/leveling and loot distribution
- Dragon Egg NFT integration
- Referral rewards tracking

#### **MoonEconomy Program** (Staking & Electricity)
- DBTC token staking with time-weighted multipliers
- LP token staking for liquidity providers
- Electricity generation from staking
- SOL reward distribution to stakers
- Cross-program integration with MoonBase

---

## 2. Program Architecture

### Core State Accounts

#### **MoonBase Program**

```rust
GlobalConfig (1 per program)
├─ Base creation costs and settings
├─ Total moonbases created tracker
├─ SOL collected metrics
├─ Dragon Egg NFT configuration
├─ Faction system (25 max factions)
├─ Expansion configurations (20 max)
└─ Loot percentage settings

DogeBtcMining (1 per program)
├─ Token vault for mining rewards
├─ Dynamic distribution rate (updated hourly)
├─ Price oracle (8-hour rolling average)
├─ Global hashpower tracker
├─ Index-based mining distribution (u128)
├─ Protocol Owned Liquidity (POL) stats
└─ Raydium pool integration

UserMoonBaseInstance (1 per user)
├─ Owner and referral tracking
├─ Active hashpower and electricity
├─ Mining claim index (u128) and claimable tokens
├─ XP, level, daily login streak
├─ Grid occupation bitmap (38 bytes for 300 tiles)
├─ Purchased expansions list
├─ Available modules inventory
├─ PvP HP and game state
└─ Incubated Dragon Egg reference

ModuleInstance (many per user)
├─ Config ID and upgrade level
├─ Grid position (x, y, width, height)
├─ Runtime state (HP, production stats)
├─ Electricity cost
└─ Active/inactive status

DragonEggMetadata (1 per egg)
├─ NFT mint address
├─ Power level (0-100,000)
├─ DNA (32 bytes for breeding)
├─ Incubated moonbase reference
└─ Timestamps

LootRewards (1 per program)
├─ DBTC vault accumulation (10% of mining)
├─ SOL vault accumulation (10% of fees)
├─ Total distributed amounts
└─ Vault authority bumps

LevelStats (1 per program)
├─ Top 10 levels tracking
├─ User count per level
├─ Max level achieved globally
└─ Timestamps
```

#### **MoonEconomy Program**

```rust
GlobalConfig (1 per program)
├─ Authority and fee collector
├─ Lockup duration settings (min/max days)
├─ Time multipliers (base to max)
└─ Distribution allocations

DogeBtcVault (1 per program)
├─ Staking custodian account
├─ Total and weighted DBTC locked
├─ Accumulated SOL per point (u128)
├─ Electricity conversion rate
└─ Emergency withdrawal tax

LiquidityVault (1 per program)
├─ LP token custodian account
├─ Total and weighted LP locked
├─ Accumulated SOL per point (u128)
├─ Electricity conversion rate
└─ Emergency withdrawal tax

UserMoonElectricity (1 per user)
├─ Total staking stats (DBTC + LP)
├─ Electricity earned total
├─ Pending SOL rewards
├─ Reward debt checkpoints (u128)
├─ Position indices (max 7 each)
└─ Total SOL claimed

DogeBtcPosition (up to 7 per user)
├─ Staked amount and weighted amount
├─ Lockup timestamps and duration
├─ Time multiplier applied
└─ Electricity generated per day

LiquidityPosition (up to 7 per user)
└─ Same structure as DogeBtcPosition
```

### PDA (Program Derived Address) Seeds

#### MoonBase
```
global-config                    → GlobalConfig
moon-doge-mining                → DogeBtcMining
sol-treasury                    → SOL collection vault
mdoge-vault-authority           → Token vault signer
user-moonbase + user_pubkey     → UserMoonBaseInstance
module-instance + user + index  → ModuleInstance
dragon-egg-metadata + mint      → DragonEggMetadata
dragon-egg-custody              → NFT custody vault
incubation-state + user         → IncubationState
loot-rewards                    → LootRewards
loot-sol-vault                  → SOL loot vault
loot-mdoge-vault                → DBTC loot vault
level-stats                     → LevelStats
referral-rewards + user         → ReferralRewards
```

#### MoonEconomy
```
global_config                   → GlobalConfig
dogebtc_vault                  → DogeBtcVault
liquidity_vault                 → LiquidityVault
dogewifbtc-custodian           → DBTC custody account
liquidity-custodian            → LP custody account
user-electricity + user         → UserMoonElectricity
dogewifbtc-position + user + i  → DogeBtcPosition
liquidity-position + user + i   → LiquidityPosition
fee_collector                   → Fee collector PDA
```

---

## 3. Economic Model

### 🎡 The Complete Economic Flywheel

```
User SOL In
    ↓
    ├─→ 50% to SOL Treasury (base creation/module purchases)
    ├─→ 15% to Referrer (if applicable)
    └─→ 35% to SOL Treasury (remainder)
    
SOL Treasury
    ├─→ 10% to Loot SOL Vault (ongoing accumulation)
    └─→ 90% held for operations

DBTC Mining (Dynamic Emissions)
    ↓
    ├─→ 90% to Users (via hashpower share)
    └─→ 10% to Loot DBTC Vault

Every 8 Hours (via update_dbtc_dist_per_slot):
    ├─→ Swap portion of DBTC for SOL (via Raydium)
    ├─→ Add SOL + DBTC to Raydium Pool
    ├─→ Burn all LP tokens received (Protocol Owned Liquidity)
    ├─→ Update price oracle (8-hour rolling average)
    ├─→ Adjust emission rate:
    │   ├─→ Price UP → +1% emission increase
    │   └─→ Price DOWN → -3% emission decrease
    └─→ Reset cycle

Users Stake DBTC/LP
    ↓
    ├─→ Receive Electricity (used to power modules)
    ├─→ Earn SOL rewards (from treasury distributions)
    └─→ Time-weighted multipliers (longer lockup = more rewards)

Modules Consume Electricity
    ↓
    ├─→ Mining Modules: Generate Hashpower → Mine DBTC
    └─→ Attraction Modules: Generate XP → Level Up → Loot

Loot System
    ↓
    ├─→ 10% of all DBTC mined → Loot DBTC Vault
    ├─→ 10% of all SOL fees → Loot SOL Vault
    └─→ Distributed on level-ups (probability + milestones + jackpots)
```

### 💰 Token Economics

#### DBTC Token Properties
- **Decimals**: 6
- **Type**: SPL Token-2022 (with transfer tax support)
- **Transfer Tax**: 1% burn on transfers (deflationary)
- **Total Supply**: Pre-minted (fixed supply)
- **Distribution**: Via mining (hashpower-based) with dynamic rates

#### Initial Distribution (Theoretical)
```
Mining Rewards:     ~85-90% (to users over time)
Loot Vault:         ~10% (from mining output)
Development:        ~0-5% (initial allocation)
```

### 📊 Dynamic Emission System (8-Hour Cycles)

#### Price Oracle Integration
```rust
DogeBtcMining {
    price_history: Vec<PriceEntry>, // 8 entries, 1 per hour
    avg_price_8h: u64,              // Weighted average
    prev_avg_price_8h: u64,         // Previous cycle
    current_dist_rate: u64,         // Adjusts every 8 hours
}

PriceEntry {
    timestamp: i64,
    price: u64, // SOL per DBTC (10^9 scale)
}
```

#### Emission Adjustment Logic
```
Every 8 hours:
1. Collect 8 hourly price snapshots from Raydium pool
2. Calculate weighted average price
3. Compare with previous 8-hour average
4. Adjust emission rate:
   - Price increased → Emission +1% (encourage mining)
   - Price decreased → Emission -3% (reduce selling pressure)
   - Price unchanged → Keep same rate
```

#### Why This Works
- **Responsive to market**: Auto-adjusts to demand
- **Dampened volatility**: 8-hour average prevents manipulation
- **Asymmetric response**: Faster cuts on price drops (-3%) vs increases (+1%)
- **Self-balancing**: High prices → more mining → more supply → price stabilization

### 🏦 Protocol Owned Liquidity (POL)

#### Mechanism
```
Every 8-hour cycle:
1. Swap DBTC → SOL (via Raydium)
2. Combine with accumulated SOL from treasury
3. Add both to Raydium DBTC-SOL pool
4. Receive LP tokens
5. BURN all LP tokens immediately (permanent liquidity)
6. Track POL stats:
   - total_lp_burnt
   - total_sol_added
   - total_dbtc_added
   - lp_operations_count
```

#### Economic Benefits
- **Permanent Liquidity**: Burned LP = liquidity can never be removed
- **Price Floor**: Growing permanent liquidity creates strong price floor
- **No IL Risk**: Protocol doesn't care about impermanent loss (LP is burned)
- **Sustainable**: Funded by emissions, not token sales
- **Transparency**: All POL stats tracked on-chain

---

## 4. Module System

### Module Types

#### **Mining Modules**
- **Purpose**: Generate hashpower → Mine DBTC tokens
- **Stats**: `max_hp`, `base_hashpower`, `power_consumption`
- **Scaling**: 15% exponential growth per upgrade level
- **Formula**: `hashpower = base_hashpower × (1.15^upgrade_level)`

**Example Progression** (base_hashpower = 100):
```
Level 0: 100 hash/s
Level 1: 115 hash/s (+15%)
Level 5: 201 hash/s (2.01x)
Level 10: 404 hash/s (4.04x)
```

#### **Attraction Modules**
- **Purpose**: Generate XP for leveling up
- **Stats**: `max_hp`, `base_xp_per_hour`, `power_consumption`
- **Scaling**: 15% exponential growth per upgrade level
- **Formula**: `xp_per_hour = base_xp × (1.15^upgrade_level)`

### Module Configuration

```rust
ModuleConfig {
    id: u16,                           // Unique identifier
    name: String,                      // Display name (max 32 chars)
    image_url: String,                 // Asset URL (max 64 chars)
    module_type: ModuleType,           // Mining or Attraction
    stats: ModuleStats,                // Type-specific stats
    faction_ids: Vec<u8>,              // Faction restrictions (empty = all)
    min_level: u8,                     // Minimum moonbase level
    width: u8,                         // Grid tiles wide
    height: u8,                        // Grid tiles tall
    mint_cost: u64,                    // SOL cost to mint/buy
    upgrade_cost: u64,                 // Base SOL per upgrade
    upgrade_level_requirements: Vec<u8>, // Moonbase levels per upgrade
    is_active: bool,                   // Can be purchased?
}
```

### Upgrade System

#### Level Requirements
```
Example: upgrade_level_requirements = [5, 10, 15, 20]

Upgrade 1: Requires moonbase level 5
Upgrade 2: Requires moonbase level 10
Upgrade 3: Requires moonbase level 15
Upgrade 4: Requires moonbase level 20
```

#### Progressive Cost Scaling
```
Formula: cost = base_cost × (1.25^upgrade_level)

Example (base_cost = 0.05 SOL):
Upgrade 1: 0.05 SOL × 1.25 = 0.0625 SOL
Upgrade 2: 0.05 SOL × 1.56 = 0.078 SOL
Upgrade 3: 0.05 SOL × 1.95 = 0.0975 SOL
Upgrade 4: 0.05 SOL × 2.44 = 0.122 SOL
Total for 4 upgrades: ~0.31 SOL
```

### Module Lifecycle

```
1. MINTING (Buy Module)
   User pays: mint_cost SOL
   ├─→ Module added to available_modules inventory
   ├─→ Module is UNDEPLOYED (not on grid)
   ├─→ Award XP (if applicable)
   └─→ Referral fee processed (15%)

2. INSTALLATION (Deploy to Grid)
   User places at (x, y) coordinates
   ├─→ Validate grid placement (bounds + overlap)
   ├─→ Check electricity availability
   ├─→ Mark tiles occupied in bitmap
   ├─→ Update hashpower (if mining module)
   ├─→ Consume electricity
   ├─→ Set is_active = true
   └─→ Award 50 XP

3. UPGRADING
   User pays: upgrade_cost × (1.25^next_level) SOL
   ├─→ Check moonbase level requirement
   ├─→ Increase upgrade_level
   ├─→ Recalculate stats (hashpower/XP)
   ├─→ Update global hashpower (if mining)
   ├─→ Process referral fee
   └─→ Award 30 XP

4. REMOVAL
   User removes module from grid
   ├─→ Clear tiles in bitmap
   ├─→ Reduce hashpower (if mining)
   ├─→ Free up electricity
   ├─→ Set is_active = false
   └─→ Module returns to inventory

5. DELETION
   User permanently deletes module
   ├─→ Remove must be called first (undeployed)
   ├─→ Removes from available_modules
   └─→ Frees account space
```

---

## 5. Grid & Placement

### Grid Specifications

```rust
GRID_WIDTH: 20 tiles
GRID_HEIGHT: 15 tiles
TOTAL_TILES: 300 tiles
BITMAP_SIZE: 38 bytes

DEFAULT_MOONBASE_WIDTH: 10 tiles
DEFAULT_MOONBASE_HEIGHT: 8 tiles
DEFAULT_AREA: 80 tiles
```

### Bitmap Storage System

**Why Bitmap?**
- **Constant Size**: 38 bytes for 300 tiles (vs 300+ bytes for array)
- **Fast Operations**: Bit manipulation is extremely efficient
- **Predictable Rent**: Fixed account size

**How It Works:**
```
Each tile is 1 bit in the 38-byte array
Tile (x, y) → Index = (y × 20) + x
Index → Byte = index / 8, Bit = index % 8

Example:
Tile (5, 3) → Index = (3 × 20) + 5 = 65
Byte 8, Bit 1 → occupied_bitmap[8] & (1 << 1)
```

### Placement Validation

```rust
fn can_place_module(moonbase, x, y, width, height) -> bool {
    // 1. Bounds check
    if x + width > current_width { return false; }
    if y + height > current_height { return false; }
    
    // 2. Overlap check (iterate all tiles)
    for dy in 0..height {
        for dx in 0..width {
            if is_tile_occupied(x + dx, y + dy) {
                return false; // Collision!
            }
        }
    }
    
    return true; // Valid placement
}
```

### Expansion System

```rust
ExpansionConfig {
    id: 8,
    name: "Western Mining Sector",
    required_level: 10,
    cost_sol: 2_000_000_000, // 2 SOL
    new_width: 15,
    new_height: 12,
    is_active: true,
}
```

**Expansion Flow:**
```
1. User reaches level 10
2. Calls expand_moonbase(expansion_id: 8)
3. Pays 2 SOL (with referral split)
4. current_width: 10 → 15
5. current_height: 8 → 12
6. New area: 80 → 180 tiles (+100 tiles)
7. Awarded XP: 100 + (level × 10) = 200 XP
```

---

## 6. Mining & Distribution

### Index-Based Distribution System

**Why Index System?**
- **Gas Efficient**: No loops through all users
- **Fair Distribution**: Perfectly proportional to hashpower
- **Overflow Safe**: Uses u128 for intermediate math

#### Core Formula

```rust
// Global index tracking
dbtc_tokens_minted_per_hashpower: u128

// Every slot:
new_tokens_mined = slots_passed × current_dist_rate
index_increment = (new_tokens_mined × MAX_SAFE_U64) / total_hashpower
dbtc_tokens_minted_per_hashpower += index_increment

// Per user claiming:
index_diff = global_index - user.dbtc_claim_index
claimable = (index_diff × user_hashpower) / MAX_SAFE_U64
user.dbtc_claim_index = global_index
```

#### Overflow Prevention

```rust
MAX_SAFE_U64 = u64::MAX / 1_000_000

// All multiplications use u128
let result = ((a as u128)
    .saturating_mul(b as u128)
    .saturating_div(c as u128)) as u64;
```

### Dynamic Emission Mechanics

#### 8-Hour Cycle Breakdown

```
Hour 0: update_dbtc_dist_per_slot() called
├─→ Swap DBTC for SOL (amount based on current_dist_rate)
├─→ Record price in price_history[0]
└─→ Accumulate SOL for POL

Hour 1-7: (automatic price snapshots)
├─→ update_dbtc_dist_per_slot() can be called hourly
├─→ Each call adds 1 price entry
└─→ SOL accumulates for POL

Hour 8: Cycle completes
├─→ Final update_dbtc_dist_per_slot() call
├─→ Calculate 8-hour weighted average
├─→ Compare with previous avg
├─→ Adjust emission rate:
│   ├─→ avg_price UP → dist_rate × 1.01
│   └─→ avg_price DOWN → dist_rate × 0.97
├─→ Add liquidity to Raydium (accumulated SOL + DBTC)
├─→ Burn LP tokens
├─→ Clear price_history
└─→ Start new 8-hour cycle
```

#### Price Calculation (Weighted Average)

```rust
// More recent prices get higher weight
weights = [1, 2, 3, 4, 5, 6, 7, 8] // Hour 0 to Hour 7

weighted_sum = Σ(price[i] × weight[i])
total_weights = Σ(weight[i]) = 36

avg_price_8h = weighted_sum / total_weights
```

**Why Weighted?**
- Recent prices matter more than old ones
- Smooth transition between cycles
- Reduces impact of brief price spikes

### Mining Claim Flow

```
User calls: claim_dbtc_tokens()

1. update_global_mining_index()
   ├─→ Calculate slots_passed since last update
   ├─→ Calculate new_tokens_mined
   ├─→ Update global index
   └─→ Update total_tokens_mined

2. mine_dbtc_for_user()
   ├─→ Calculate index_diff
   ├─→ Calculate user's share: (index_diff × hashpower) / MAX_SAFE_U64
   ├─→ Update user.dbtc_claim_index
   └─→ Add to user.claimable_dbtc

3. claim_dogebtc_tokens()
   ├─→ Transfer claimable_dbtc from vault to user
   ├─→ Transfer 10% to loot_dbtc_vault
   ├─→ Reset user.claimable_dbtc = 0
   └─→ Calculate mining XP: (claimed_amount / 1000) × 15

4. [IF Dragon Egg incubated]
   ├─→ Calculate power_increase = claimed_amount / 1000
   ├─→ Update egg.power (capped at 100,000)
   └─→ Update incubation_state

5. Process daily login and award XP
6. Emit DogeBtcTokensClaimed event
```

---

## 7. Staking & Electricity

### The Electricity Bridge

**MoonEconomy generates electricity → MoonBase consumes it**

```
Staking System (MoonEconomy)          MoonBase System
        ↓                                     ↓
User stakes DBTC/LP                   Modules need electricity
        ↓                                     ↓
Weighted amount calculated            Module has power_consumption
        ↓                                     ↓
Electricity = weighted × rate         Check: used_electricity ≤ available
        ↓                                     ↓
CPI: update_user_electricity()        Update available_electricity
        ↓                                     ↓
available_electricity += amount       Module can be installed
```

### Time-Weighted Staking

#### Multiplier Calculation

```rust
// Linear interpolation based on lockup duration
multiplier = base + (max - base) × (lockup - min) / (max_lockup - min_lockup)

Example (min=7d, max=1095d, base=100, max_multiplier=900):
7 days:     100 (1.0x)
30 days:    119 (1.19x)
90 days:    176 (1.76x)
365 days:   391 (3.91x)
1095 days:  900 (9.0x)
```

#### Weighted Amount

```
staked_amount = 1000 DBTC
lockup = 365 days
multiplier = 391 (3.91x)

weighted_amount = 1000 × 391 / 100 = 3,910 weighted points
```

#### Electricity Conversion

```rust
// Set by admin (configurable)
electricity_per_weighted_moondoge = 1000

// User electricity from this position
electricity = weighted_amount × electricity_per_weighted_moondoge
electricity = 3,910 × 1,000 = 3,910,000 units
```

### SOL Rewards Distribution

#### Accumulation Model

```rust
// When SOL is added to vault:
new_sol_per_point = sol_amount × PRECISION_FACTOR / total_weighted
accumulated_sol_per_point += new_sol_per_point

PRECISION_FACTOR = 1,000,000 (6 decimals)
```

#### User Rewards Calculation

```rust
// Pending rewards formula
pending = (user_weighted × accumulated_sol_per_point) / PRECISION - reward_debt

// When user stakes more:
reward_debt = accumulated_sol_per_point (checkpoint)

// When user claims:
payout = pending_rewards
reward_debt = accumulated_sol_per_point (reset checkpoint)
```

#### Example

```
Global State:
total_weighted_dbtc = 100,000
accumulated_sol_per_point = 5,000,000,000 (5 SOL × 1M precision)

User A:
weighted_amount = 10,000 (10% of total)
reward_debt = 0 (first time)

Pending = (10,000 × 5,000,000,000) / 1,000,000 - 0
        = 50,000,000,000 / 1,000,000
        = 50,000 lamports
        = 0.00005 SOL

User A claims → reward_debt = 5,000,000,000

More SOL added (10 SOL):
new_per_point = 10 × 10^9 × 1M / 100,000 = 100,000,000,000
accumulated = 5,000,000,000 + 100,000,000,000 = 105,000,000,000

User A pending = (10,000 × 105B) / 1M - 5B
               = 1,050B / 1M - 5B
               = 1,050,000 - 5,000
               = 1,045,000 lamports
               = 0.001045 SOL (10% of 10 SOL added)
```

### Early Withdrawal Penalties

#### DBTC Staking
```rust
// Dynamic penalty based on time remaining
remaining_pct = (lockup_end - now) / (lockup_end - start) × 100
penalty_pct = emergency_tax × remaining_pct / 100

// Penalty is BURNED (deflationary)

Example:
emergency_tax = 10%
locked for 365 days
unstake after 100 days (265 days remaining)

remaining_pct = 265 / 365 × 100 = 72.6%
penalty_pct = 10% × 72.6% = 7.26%
penalty_amount = staked × 7.26%

Result: User gets 92.74%, 7.26% is burned
```

#### LP Staking
```rust
// Same formula but penalty goes to TREASURY (not burned)
// LP tokens are not burnable after minting by Raydium
```

---

## 8. XP & Leveling

### XP Sources

| Activity | XP Reward | Notes |
|----------|-----------|-------|
| Daily Login | 10 XP | Once per 24 hours |
| Install Module | 50 XP | Per deployment |
| Upgrade Module | 30 XP | Per upgrade level |
| Moonbase Expansion | 100 + (level×10) XP | Scales with difficulty |
| Mining | 15 XP per 1000 DBTC | Calculated during claim |
| Referral Earnings | 500 XP per SOL | sqrt scaling: `√(sol_earned) × 500` |

### Level Progression

#### Exponential Curve
```rust
required_xp(level) = 120 × (1.35^level)

Rounded to nearest 10 for clean numbers
```

#### Level Table

| Level | XP Required | Cumulative | Approximate Time |
|-------|-------------|------------|------------------|
| 1 | 160 | 160 | 16 daily logins |
| 2 | 220 | 380 | 38 days |
| 3 | 300 | 680 | 68 days |
| 5 | 540 | 1,740 | 3-4 weeks mining |
| 10 | 2,100 | 9,500 | 2-3 months |
| 15 | 5,300 | 28,000 | 4-6 months |
| 20 | 9,400 | 72,000 | 8-12 months |
| 25 | 16,700 | 156,000 | 1-2 years |
| 30 | 29,600 | 290,000 | 2-3 years |

### Daily Login System

```rust
// Automatic daily login processing
if last_login_ts + 24 hours < current_time {
    user.xp += 10
    user.last_login_ts = current_time
    user.daily_login_streak += 1
}

// Streak breaks after 48 hours
if last_login_ts + 48 hours < current_time {
    user.daily_login_streak = 0 // Reset!
}
```

---

## 9. Loot System

### Dual-Currency Loot (SOL + DBTC)

**Every loot win pays BOTH:**
1. SOL (direct from loot SOL vault)
2. DBTC (equivalent value based on 8-hour avg price)

#### Value Mirroring

```rust
// Get current DBTC price from oracle
dbtc_price_in_sol = doge_btc_mining.avg_price_8h

// If SOL payout is 10 SOL
sol_payout = 10 × 10^9 lamports

// Calculate equivalent DBTC
dbtc_payout = (sol_payout × 10^9) / dbtc_price_in_sol

// User receives BOTH
```

### Tier-Based Probabilities

| Level Range | Type | Base Chance | Vault Cut | Guaranteed? |
|-------------|------|-------------|-----------|-------------|
| 1-4 | Minor | 3% + 0.2%/lvl | 1% | No |
| 5, 10 | Milestone-5 | 100% | 0.5% | **YES** |
| 6-14 | Minor | 3% + 0.2%/lvl | 1% | No |
| 15, 20 | Milestone-Rare | 100% | 2% | **YES** |
| 15-24 | Rare | 15% | 5% | No |
| 25+ (÷5) | Milestone-Legendary | 100% | 4% | **YES** |
| 25+ | Legendary | 25% | 8% | No |
| 10, 20, 30... | Jackpot Wheel | 0.20% | Fixed Pots | No |

### Exclusivity Bonuses

**Based on user count at that level:**

| Rank | Users at Level | Chance Mult | Vault Mult | Description |
|------|----------------|-------------|------------|-------------|
| 🥇 First | 1 | ×1.20 | ×2.00 | Only player at this level |
| 🥈 Top 3 | 2-3 | ×1.15 | ×1.50 | Elite group |
| 🥉 Top 10 | 4-10 | ×1.10 | ×1.25 | Pioneers |
| 🏅 Top 25 | 11-25 | ×1.05 | ×1.00 | Early adopters |
| 🌍 Crowd | 26+ | ×1.00 | ×1.00 | Regular players |

#### Example Calculation

**Level 25 achievement (first player globally):**

```
Base values:
- Chance: 25% (2,500 bp)
- Vault cut: 8% (800 bp)

Exclusivity bonus (first player):
- Chance mult: ×1.20
- Vault mult: ×2.00

Final values:
- Chance: 25% × 1.20 = 30% (3,000 bp)
- Vault cut: 8% × 2.00 = 16% (1,600 bp)

If vault has 100 SOL:
- Payout: 16 SOL + equivalent DBTC
- Win probability: 30%
```

### Jackpot System

**Trigger:** Levels divisible by 10 (10, 20, 30, 40...)

**Probability:** 0.20% base (affected by exclusivity)

**Fixed Pots (descending priority):**
```
1. 1,000 SOL (requires ≥1,100 SOL in combined vault)
2. 750 SOL (requires ≥825 SOL)
3. 690 SOL (requires ≥759 SOL)
4. 510 SOL (requires ≥561 SOL)
5. 420 SOL (requires ≥462 SOL)
```

**Safety:** Requires 110% of pot value in vault (10% buffer)

#### Jackpot Selection Logic

```rust
fn try_jackpot(vault_sol: u64, roll: u16) -> (u64, bool) {
    // Probability check (20 basis points = 0.20%)
    if roll > 20 { return (0, false); }
    
    // Try pots in descending order
    if vault_sol >= 1_100_000_000_000 { return (1_000_000_000_000, true); }
    if vault_sol >= 825_000_000_000 { return (750_000_000_000, true); }
    if vault_sol >= 759_000_000_000 { return (690_000_000_000, true); }
    if vault_sol >= 561_000_000_000 { return (510_000_000_000, true); }
    if vault_sol >= 462_000_000_000 { return (420_000_000_000, true); }
    
    (0, false) // Not enough in vault
}
```

### Currency Selection Algorithm

**The system intelligently picks SOL vs DBTC based on availability:**

```rust
fn pick_currency(sol_vault, dbtc_vault, sol_desired, dbtc_desired) {
    // Strategy 1: Try to pay in SOL (preferred for UX)
    if can_pay_in_sol(sol_vault, sol_desired) {
        return (sol_desired, 0); // Pay all in SOL
    }
    
    // Strategy 2: Try to pay in DBTC
    if can_pay_in_dbtc(dbtc_vault, dbtc_desired) {
        return (0, dbtc_desired); // Pay all in DBTC
    }
    
    // Strategy 3: Split payment
    // Pay as much SOL as possible, rest in DBTC
    let available_sol = min(sol_vault × 0.1, sol_desired);
    let remaining_value = sol_desired - available_sol;
    let dbtc_for_remainder = convert_sol_to_dbtc(remaining_value);
    
    if dbtc_vault >= dbtc_for_remainder {
        return (available_sol, dbtc_for_remainder);
    }
    
    // Strategy 4: Best effort with vault limits
    return (min(sol_vault × 0.1, sol_desired), 
            min(dbtc_vault × 0.1, dbtc_desired));
}
```

### Safety Limits

```rust
// Normal loot (non-jackpot)
MIN_PAYOUT: 0.01 SOL (10,000,000 lamports)
MAX_PAYOUT: 100 SOL (100,000,000,000 lamports)
MAX_VAULT_PCT: 10% (never drain >10% per win)

// Jackpot (can exceed 10% if vault is large enough)
JACKPOT_BUFFER: 110% (requires 10% safety margin)
```

---

## 10. Dragon Egg NFTs

### NFT Locking Mechanism

**Critical Security Feature:** Physical NFT custody

```
User owns Dragon Egg NFT
        ↓
calls: incubate_dragon_egg()
        ↓
NFT transferred: User Wallet → Custody PDA
        ↓
user_moonbase.incubated_dragon_egg = Some(egg_metadata_pda)
        ↓
NFT is LOCKED (user cannot transfer/sell)
        ↓
Power grows automatically during claim_dbtc_tokens()
        ↓
calls: remove_dragon_egg()
        ↓
NFT transferred: Custody PDA → User Wallet (via invoke_signed)
        ↓
user_moonbase.incubated_dragon_egg = None
        ↓
User regains full control
```

### Power Growth Formula

```rust
// During claim_dbtc_tokens()
if user_moonbase.incubated_dragon_egg.is_some() {
    power_increase = claimed_dbtc_amount / POWER_RATE_MULTIPLIER
    
    POWER_RATE_MULTIPLIER = 1,000
    
    egg.power = min(egg.power + power_increase, MAX_EGG_POWER)
    MAX_EGG_POWER = 100,000
}
```

**Example:**
```
User claims 50,000 DBTC
power_increase = 50,000 / 1,000 = 50
Current power: 1,200
New power: 1,250 (+50)
```

### DNA System

```rust
// Generated at minting
dna: [u8; 32] // 32 bytes of genetic data

fn generate_dragon_egg_dna(slot, user, count) -> [u8; 32] {
    let hash = keccak::hashv(&[
        &slot.to_le_bytes(),
        &user.to_bytes(),
        &count.to_le_bytes(),
    ]);
    hash.0
}
```

**Future Use Cases:**
- Breeding (combine DNAs)
- Rarity traits
- Evolution paths
- Special abilities

### Pricing Tiers

```
Moonbase Creation:
├─→ Tier 1 (0.5 SOL): No Dragon Egg
└─→ Tier 2 (1.42 SOL): Includes Dragon Egg NFT
```

---

## 11. Referral System

### Fee Structure

```
User spends SOL on modules/upgrades/expansions
        ↓
15% goes to referrer (if set)
85% goes to treasury
        ↓
Referrer's ReferralRewards PDA gets credited
        ↓
Referrer can claim_referral_rewards()
        ↓
Receives SOL + XP bonus
```

### XP Calculation

```rust
// Base XP from referral count
base_xp = referrals_count × 100

// Bonus XP from SOL earned
sol_bonus_xp = √(total_sol_earned) × 500

total_referral_xp = base_xp + sol_bonus_xp
```

**Example:**
```
10 referrals
Total SOL earned: 4 SOL (4,000,000,000 lamports)

base_xp = 10 × 100 = 1,000 XP
sqrt_lamports = √(4,000,000,000) ≈ 63,246
sol_bonus = 63,246 × 500 / 1,000,000,000 = 31 XP

Total: 1,031 XP
```

### Referral Rewards Account

```rust
ReferralRewards {
    owner: Pubkey,              // Referrer
    total_sol_earned: u64,      // Lifetime SOL from referrals
    sol_claimed_for_xp: u64,    // SOL already converted to XP
    referrals_count: u16,       // Number of successful referrals
    bump: u8,
}
```

**Anti-Spam:** SOL is only counted for XP once (tracked in `sol_claimed_for_xp`)

---

## 12. User Journey

### 🌅 Phase 1: Getting Started (Levels 1-5)

**Day 1: Moonbase Creation**
```
1. User creates moonbase
   - Option A: 0.5 SOL (no NFT)
   - Option B: 1.42 SOL (with Dragon Egg NFT)
   - Sets referral code (optional)
   - Chooses faction (cosmetic)
   
2. Moonbase initialized
   - Size: 10×8 tiles (80 tiles)
   - Available electricity: 0
   - Hashpower: 0
   - Level: 0, XP: 0
   
3. Next steps
   - Stake DBTC or LP to get electricity
   - Buy first mining module
```

**Week 1: First Modules**
```
1. Stake 1000 DBTC (30-day lockup)
   - Weighted: ~1,190 points
   - Electricity: 1,190,000 units
   - Earns SOL rewards
   
2. Buy Mining Module (config_id: 1)
   - Cost: 0.05 SOL
   - Module added to inventory (undeployed)
   
3. Install Mining Module at (2, 2)
   - Consumes electricity: 50,000 units
   - Adds hashpower: 100 hash/s
   - Awards 50 XP
   
4. Daily login for 7 days
   - 70 XP total
   - Streak: 7 days
   
5. Reach Level 1
   - Required: 160 XP
   - Current: 120+ XP
   - Loot roll: ~3.6% chance for small reward
```

### 🚀 Phase 2: Expansion (Levels 5-15)

**Month 1-2: Scaling Up**
```
1. Buy and install more modules
   - 5x Mining Modules
   - 2x Attraction Modules
   - Total hashpower: ~600 hash/s
   - Total electricity: ~350,000 units
   
2. Stake more for electricity
   - Stake 5,000 DBTC (90-day lockup)
   - Multiplier: ~1.8x
   - Weighted: ~9,000 points
   - Electricity: ~9,000,000 units
   
3. First expansions
   - Level 10 unlocks: +50 tiles
   - Level 15 unlocks: +100 tiles
   - More space for modules
   
4. Mine DBTC regularly
   - Claim every 24-48 hours
   - Accumulate: ~10,000-50,000 DBTC/day
   - Mining XP: ~150-750 XP/day
   
5. Dragon Egg growth (if owned)
   - Power increases with each claim
   - 50,000 DBTC claimed = +50 power
   - Egg power: 0 → 2,000-5,000 after 2 months
```

### ⚡ Phase 3: Optimization (Levels 15-25)

**Month 3-6: Competitive Play**
```
1. Upgrade modules
   - Upgrade top 5 mining modules to level 2-3
   - Hashpower: 600 → 1,200 hash/s
   - Cost: ~0.5-1 SOL per module
   - Awards 30 XP per upgrade
   
2. Compete for rare levels
   - Aim to be first to level 20
   - Exclusivity bonuses: 2x vault rewards!
   - Rare loot chances: 15% with 5% vault cuts
   
3. Referral program
   - Invite friends
   - Earn 15% of their spending
   - Bonus XP from referral SOL
   
4. Max electricity generation
   - Stake 50,000+ DBTC
   - 1-year lockups for 3.9x multiplier
   - Run 20-30 modules simultaneously
```

### 👑 Phase 4: End Game (Levels 25+)

**Month 6+: Legendary Status**
```
1. Legendary loot tier
   - 25% base chance for loot
   - 8% vault cuts
   - If first to level 30: 30% chance, 16% vault!
   
2. Jackpot opportunities
   - Every 10 levels: 0.20% jackpot chance
   - Potential wins: 420-1,000 SOL + DBTC
   - Exclusivity boosts chance to ~0.24%
   
3. Fully optimized base
   - 50+ modules deployed
   - All at level 5-10 upgrades
   - Hashpower: 10,000-50,000 hash/s
   - Mining: 500,000-2M DBTC/day
   
4. Dragon Egg maturation
   - Power: 50,000-100,000 (max)
   - Potential future utility
   - Breeding/evolution systems
```

---

## 13. Economic Analysis

### 💎 Value Flows

#### SOL Inflows
```
Moonbase Creation:     0.5-1.42 SOL per user
Module Purchases:      0.01-0.5 SOL per module
Module Upgrades:       0.05-5 SOL per upgrade (scales)
Expansions:            1-10 SOL per expansion

Total User LTV:        ~10-100+ SOL over lifetime
```

#### SOL Outflows
```
Referral Payouts:      15% of all SOL spent
Loot Distributions:    10% of fees → then distributed
Staking Rewards:       From treasury → DBTC/LP stakers
```

#### DBTC Inflows (Creation)
```
Mining Emissions:      Dynamic rate (starts ~X per slot)
Adjusted hourly:       ±1% or -3% based on price
```

#### DBTC Outflows (Sinks)
```
Transfer Tax:          1% burned on all transfers
Early Unstake:         Penalty amount burned
Loot Distributions:    10% to loot → distributed to users
POL Burns:             Effectively locked in LP (permanent)
```

### 📈 Economic Health Metrics

#### Sustainability Indicators

**Positive Signs:**
- ✅ POL growing (permanent liquidity increasing)
- ✅ Price stable or rising over 8-hour cycles
- ✅ Emission rate auto-adjusting appropriately
- ✅ Loot vaults accumulating faster than distributed
- ✅ Staking TVL growing
- ✅ Active users increasing

**Warning Signs:**
- ⚠️ Emission rate in sustained decline (price dropping)
- ⚠️ Loot vaults depleting (too many high-level users)
- ⚠️ POL not growing (insufficient swap volume)
- ⚠️ Staking TVL declining (users unstaking)

#### Key Ratios to Monitor

```
Loot Sustainability Ratio = accumulation_rate / distribution_rate
Target: > 1.5x (vault grows over time)

POL Growth Rate = pol_added_per_cycle / total_liquidity
Target: > 1% per cycle

Emission Efficiency = dbtc_mined / total_hashpower
Target: Stable or slight decline

Staking APR = sol_distributed / tvl_in_sol
Target: 10-50% APR competitive
```

### 🎯 Economic Balance Recommendations

#### Module Pricing
```
Entry modules:     0.01-0.05 SOL
Mid-tier:          0.1-0.5 SOL
Advanced:          1-5 SOL
Elite:             10+ SOL

Upgrade progression should feel rewarding but gated by level
```

#### Expansion Pricing
```
First expansion:   1-2 SOL (level 5-10)
Mid expansions:    3-5 SOL (level 15-20)
Late expansions:   10-20 SOL (level 25+)
```

#### Loot Vault Targets
```
Minimum safe balance:
SOL Vault:    10 SOL (supports small wins)
DBTC Vault:   100,000 DBTC (supports small wins)

Healthy balance:
SOL Vault:    100-500 SOL (supports jackpots)
DBTC Vault:   1-10M DBTC (supports large payouts)

Critical mass:
SOL Vault:    1,000+ SOL (all jackpots available)
DBTC Vault:   50M+ DBTC (massive payouts possible)
```

---

## 14. Security & Safety

### Overflow Protection

**Critical:** All financial calculations use u128 intermediate math

```rust
// ✅ SAFE
let result = ((a as u128)
    .saturating_mul(b as u128)
    .saturating_div(c as u128)) as u64;

// ❌ UNSAFE
let result = (a * b) / c; // Can overflow!
```

**Locations:**
- ✅ Mining index calculations
- ✅ Loot payout calculations
- ✅ Staking reward debt
- ✅ Price conversions

### Access Controls

```rust
// Admin-only functions
require!(
    authority.key() == global_config.ext_authority,
    ErrorCode::Unauthorized
);

// User-only functions
require!(
    user.key() == user_moonbase.owner,
    ErrorCode::Unauthorized
);

// Fee collector CPI
require!(
    fee_collector PDA signs via invoke_signed,
    ErrorCode::Unauthorized
);
```

### Economic Safety Rails

#### Loot System
```rust
// Never drain vaults completely
max_payout = min(
    desired_payout,
    vault_balance × 0.1, // 10% max
    100 × 10^9,          // 100 SOL max
);

// Jackpot requires buffer
jackpot_available = vault_balance >= pot_amount × 1.1;
```

#### Staking Penalties
```rust
// Early withdrawal is punished, not exploitable
penalty = min(
    staked_amount × emergency_tax × remaining_time_pct,
    staked_amount × 0.1, // 10% max
);
```

#### Dragon Egg Power
```rust
// Capped growth prevents inflation
egg.power = min(
    egg.power + power_increase,
    MAX_EGG_POWER, // 100,000 cap
);
```

### Anti-Exploit Measures

**No Exploitation Vectors:**
- ✅ Index-based mining (no loops to game)
- ✅ Timestamp-based daily logins (cooldown enforced)
- ✅ NFT custody (cannot sell while incubated)
- ✅ Module limits removed (no artificial scarcity)
- ✅ Electricity must be earned (cannot fake)
- ✅ Level requirements for upgrades (cannot skip)
- ✅ Loot RNG uses on-chain slot (not predictable)
- ✅ Vault limits prevent drainage

---

## 📚 Quick Reference

### Key Constants

```rust
// Decimals
DBTC_DECIMALS: 6

// Fees
REFERRAL_FEE: 15%
LOOT_PERCENTAGE: 10%
BURN_TAX: 1% (on transfers)

// Grid
GRID: 20×15 (300 tiles)
DEFAULT_MOONBASE: 10×8 (80 tiles)

// Upgrades
MAX_MODULE_UPGRADES: 10
GROWTH_RATE: 15% per level
COST_SCALING: 25% per level

// XP
XP_BASE: 120
XP_CURVE: 1.35^level
DAILY_LOGIN: 10 XP
MODULE_INSTALL: 50 XP
MODULE_UPGRADE: 30 XP

// Dragon Eggs
MAX_EGGS_PER_MOONBASE: 1
MAX_EGG_POWER: 100,000
POWER_RATE: 1,000 DBTC per power point

// Staking
MIN_LOCKUP: 7 days
MAX_LOCKUP: 1,095 days (3 years)
MAX_MULTIPLIER: 9.0x
EMERGENCY_TAX: 10%
```

### State Account Sizes

```
GlobalConfig:           ~2,000 bytes
DogeBtcMining:          ~500 bytes
UserMoonBaseInstance:   ~1,200 bytes
ModuleInstance:         ~100 bytes
ModuleConfig:           ~300 bytes
DragonEggMetadata:      ~150 bytes
LootRewards:            ~200 bytes
LevelStats:             ~200 bytes
ReferralRewards:        ~100 bytes

MoonEconomy GlobalConfig:    ~200 bytes
DogeBtcVault:               ~200 bytes
LiquidityVault:             ~200 bytes
UserMoonElectricity:        ~250 bytes
DogeBtcPosition:            ~100 bytes
LiquidityPosition:          ~100 bytes
```

---

## 🔧 Admin Operations Checklist

### Initial Deployment
```
1. Deploy MoonBase program
2. Deploy MoonEconomy program
3. Initialize GlobalConfig (both programs)
4. Initialize DogeBtcMining
5. Set up Raydium pool (DBTC-SOL)
6. Add module configurations
7. Add expansion configurations
8. Add Dragon Egg URIs
9. Set Dragon Egg collection
10. Initialize loot vaults
11. Initialize level stats
12. Configure staking parameters
```

### Ongoing Maintenance
```
Every Hour:
- Call update_dbtc_dist_per_slot() (or backend automation)
- Monitor price oracle updates
- Check POL accumulation

Every 8 Hours:
- Verify LP burn execution
- Monitor POL stats
- Check emission rate adjustments

Weekly:
- Monitor loot vault balances
- Check top level achievements
- Review economic metrics
- Adjust parameters if needed

Monthly:
- Add new module configurations
- Add new expansions
- Update Dragon Egg URI pool
- Review and optimize
```

---

**🚀 This is a production-ready, economically sustainable, and highly engaging blockchain game with multiple interconnected systems working in harmony.**


