# 🌙 MoonBase User Functions - Complete Guide

## Table of Contents
1. [System Overview](#system-overview)
2. [Core Game Loop](#core-game-loop)
3. [User Function Details](#user-function-details)
4. [XP & Leveling System](#xp--leveling-system)
5. [Module System](#module-system)
6. [Mining & Economy](#mining--economy)
7. [Loot System](#loot-system)

---

## System Overview

**MoonBase** is a Solana-based game where players:
- Build and customize their moon base with mining and attraction modules
- Earn mDOGE tokens through hashpower-based mining
- Gain XP and level up through various activities
- Win loot rewards (SOL + mDOGE) when leveling up
- Expand their base as they progress

### Key Concepts

- **Hashpower**: Mining power that generates mDOGE tokens
- **Electricity**: Resource needed to power modules (obtained by staking mDOGE)
- **XP**: Experience points that lead to level-ups
- **Modules**: Placeable buildings that provide hashpower or XP generation
- **Grid System**: 20×15 tile grid where modules are placed
- **Loot Rewards**: Casino-style rewards distributed when leveling up

---

## Core Game Loop

```
1. Create MoonBase (pay SOL) → Start at level 0
2. Buy & Install Modules → Requires electricity + SOL
3. Mine mDOGE Tokens → Based on your hashpower share
4. Claim mDOGE → Convert hashpower into tokens + earn XP
5. Gain XP from Activities → Daily login, mining, modules
6. Level Up → Roll for loot rewards (SOL + mDOGE)
7. Expand Base → Unlock more tile space at higher levels
8. Upgrade Modules → Increase hashpower/XP generation
9. Repeat → Grow your empire
```

---

## User Function Details

### 1. `create_user_moonbase`

**Purpose**: Initialize a player's moonbase (one-time setup)

**What Happens**:
```rust
// 1. PAY CREATION FEE
// - Cost: 0.1 SOL (configurable)
// - Split: 50% to creation_fee_recipient, 50% processed normally
// - Of the 50%: 15% to referrer (if any), rest to treasury

// 2. CREATE YOUR MOONBASE ACCOUNT
owner: user.pubkey
level: 0
xp: 0
hashpower: 0
electricity: 0
grid: 10×8 tiles (80 tiles total)
faction: chosen faction (0-9)

// 3. CREATE YOUR REFERRAL REWARDS ACCOUNT
// - Tracks SOL earned from people you refer
// - Can claim rewards later

// 4. PROCESS REFERRAL (if you used a referral code)
// - 15% of your 50% goes to the referrer
// - Referrer's counter increments
// - Remaining goes to treasury
```

**Parameters**:
- `referrer: Option<Pubkey>` - Optional referral code from another player
- `faction_id: u8` - Your chosen faction (0-9)

**Result**:
- ✅ MoonBase created at 10×8 tiles
- ✅ Level 0, 0 XP
- ✅ Referral rewards account created
- ✅ Ready to buy modules

**Daily Login**: Automatically processed, starting your streak at 0

---

### 2. `expand_moonbase`

**Purpose**: Purchase additional grid space for your moonbase

**What Happens**:
```rust
// 1. VALIDATE EXPANSION
// - Check: Level requirement met?
// - Check: Not already purchased?
// - Check: Expansion is active?

// 2. PAY EXPANSION COST (varies by expansion)
// Example expansions:
// - 12×10 (120 tiles): Level 5, 0.05 SOL
// - 15×12 (180 tiles): Level 10, 0.1 SOL
// - 20×15 (300 tiles): Level 20, 0.25 SOL

// 3. UPDATE MOONBASE DIMENSIONS
current_width: 10 → 12
current_height: 8 → 10
purchased_expansions: [expansion_id]

// 4. AWARD XP
// Base: 100 XP
// + 10 XP per level requirement
// Example: Level 5 expansion = 100 + (5 * 10) = 150 XP

// 5. REFERRAL SPLIT (same as creation)
// 15% to referrer, 85% to treasury
```

**Parameters**:
- `expansion_id: u8` - Which expansion to purchase (0-19)

**Result**:
- ✅ More tiles to place modules
- ✅ XP awarded
- ✅ Daily login processed
- ✅ Potential level-up (no loot, use simple XP)

---

### 3. `buy_module`

**Purpose**: Purchase a module WITHOUT installing it yet

**What Happens**:
```rust
// 1. VALIDATE PURCHASE
require!(user.level >= module.min_level)
require!(user_owned_count < module.max_per_base)
require!(module.is_active)
require!(faction_allowed || module.faction_ids.is_empty())

// 2. PAY MODULE COST
// Cost varies by module type:
// - Basic Miner: 0.01 SOL
// - Advanced Miner: 0.05 SOL
// - Attraction Module: 0.02 SOL

// 3. CREATE UNDEPLOYED MODULE INSTANCE
config_id: module_config_id
upgrade_level: 0
is_active: false  // NOT DEPLOYED YET
pos_x: 0
pos_y: 0
current_hp: max_hp
electricity_cost: calculated from stats

// 4. UPDATE AVAILABLE MODULES
// Increment count in your inventory:
available_modules: [{ config_id: 1, count: 2 }, ...]

// 5. INCREMENT MODULES_COUNT
modules_count: 0 → 1 (used for PDA derivation)

// 6. AWARD XP BASED ON SOL SPENT
// Formula: sqrt(lamports) * 500 / 1_000_000_000
// Example: 0.01 SOL = ~50 XP
```

**Parameters**:
- `config_id: u16` - Which module type to buy

**Result**:
- ✅ Module instance created (undeployed)
- ✅ Available modules count updated
- ✅ XP awarded
- ✅ Module sits in inventory, ready to install

**Note**: Module does NOT use electricity or generate rewards until installed!

---

### 4. `install_module`

**Purpose**: Deploy an undeployed module to a specific grid position

**What Happens**:
```rust
// 1. VALIDATE PLACEMENT
require!(!module.is_active) // Must be undeployed
require!(pos_x + width <= current_width)
require!(pos_y + height <= current_height)
require!(tiles_not_occupied(pos_x, pos_y, width, height))

// 2. CHECK ELECTRICITY REQUIREMENT
electricity_needed: module.electricity_cost
require!(used_electricity + electricity_needed <= available_electricity)

// 3. MARK TILES AS OCCUPIED
// Update bitmap for grid positions
occupied_bitmap[tile_index] = 1

// 4. ACTIVATE MODULE
module.is_active = true
module.pos_x = pos_x
module.pos_y = pos_y

// 5. UPDATE MOONBASE STATS
used_electricity += module.electricity_cost
pvp_hp += module.max_hp

// 6. IF MINING MODULE: UPDATE HASHPOWER
active_hashpower += module.current_hashpower(level)
GLOBAL.total_active_hashpower += module.current_hashpower(level)

// 7. AWARD XP
// Fixed: 25 XP for installation
```

**Parameters**:
- `module_index: u8` - Which module to install (0-49)
- `pos_x: u8` - X position on grid (0-19)
- `pos_y: u8` - Y position on grid (0-14)

**Result**:
- ✅ Module placed on grid
- ✅ Electricity consumed
- ✅ Hashpower active (if mining module)
- ✅ Module generates rewards
- ✅ 25 XP awarded

---

### 5. `remove_module`

**Purpose**: Undeploy a module (keeps it owned, frees up space)

**What Happens**:
```rust
// 1. VALIDATE REMOVAL
require!(module.is_active) // Must be deployed

// 2. CLEAR GRID TILES
// Mark tiles as unoccupied in bitmap
occupied_bitmap[tile_index] = 0

// 3. DEACTIVATE MODULE
module.is_active = false
module.pos_x = 0
module.pos_y = 0

// 4. UPDATE MOONBASE STATS
used_electricity -= module.electricity_cost
pvp_hp -= module.max_hp

// 5. IF MINING MODULE: UPDATE HASHPOWER
active_hashpower -= module.current_hashpower(level)
GLOBAL.total_active_hashpower -= module.current_hashpower(level)

// 6. NO XP AWARDED (just retracting)
```

**Parameters**:
- `module_index: u8` - Which module to remove

**Result**:
- ✅ Grid space freed
- ✅ Electricity freed
- ✅ Hashpower removed (if mining)
- ✅ Module back in inventory (can reinstall later)

---

### 6. `delete_module`

**Purpose**: Permanently delete an UNDEPLOYED module

**What Happens**:
```rust
// 1. VALIDATE DELETION
require!(!module.is_active) // Must be undeployed

// 2. FIND AVAILABLE MODULE ENTRY
available_modules.find(config_id)
require!(count > 0)

// 3. DECREMENT COUNT
count -= 1
if count == 0:
    remove entry from available_modules

// 4. CLOSE MODULE INSTANCE ACCOUNT
// - Rent returned to user
// - Account data deleted
```

**Parameters**:
- `module_index: u8` - Which module to delete

**Result**:
- ✅ Module permanently deleted
- ✅ Rent refunded to user
- ✅ Available modules count decremented
- ⚠️ NO REFUND of original purchase cost

---

### 7. `upgrade_module`

**Purpose**: Upgrade a module to increase its stats

**What Happens**:
```rust
// 1. VALIDATE UPGRADE
require!(module.upgrade_level < max_upgrades) // Default: 10
require!(user.level >= upgrade_level_requirement)

// 2. CALCULATE PROGRESSIVE COST
// Formula: base_cost * (1.25^next_level)
// Example progression:
// Level 1: 1.00x base cost
// Level 2: 1.25x base cost
// Level 5: 3.05x base cost
// Level 10: 9.31x base cost

// 3. PAY UPGRADE COST
// Example: Basic Miner L0→L1 = 0.005 SOL

// 4. UPGRADE MODULE
module.upgrade_level += 1

// 5. IF DEPLOYED & MINING: UPDATE HASHPOWER
// Growth formula: base_hashpower * (1.15^level)
// Example: 100 base hashpower
// - Level 0: 100
// - Level 1: 115 (+15%)
// - Level 5: 201 (+101%)
// - Level 10: 405 (+305%)
old_hashpower: 100
new_hashpower: 115
user.active_hashpower += 15
GLOBAL.total_active_hashpower += 15

// 6. AWARD XP BASED ON COST
// Same sqrt formula as purchase
```

**Parameters**:
- `module_index: u8` - Which module to upgrade

**Result**:
- ✅ Module level increased
- ✅ Stats improved (hashpower/XP generation)
- ✅ XP awarded
- ✅ Global hashpower updated (if deployed mining module)

---

### 8. `claim_mdoge_tokens`

**Purpose**: Claim your share of mined mDOGE tokens

**What Happens**:
```rust
// 1. PROCESS MINING (update to current slot)
current_slot: 12345
slots_since_last_claim: current_slot - user.moondoge_claim_index

// 2. CALCULATE YOUR SHARE
// Proportional distribution based on hashpower:
your_hashpower: 1000
global_hashpower: 10000
your_share: 10% (1000/10000)

// 3. CALCULATE TOKENS EARNED
slots_passed: 1000
reward_rate: 100 mDOGE per slot
total_mined: 1000 * 100 = 100,000 mDOGE
your_earnings: 100,000 * 10% = 10,000 mDOGE

// 4. SPLIT REWARDS (90/10)
user_amount: 9,000 mDOGE (90%)
loot_amount: 1,000 mDOGE (10% to loot vault)

// 5. TRANSFER TOKENS
// Transfer 9,000 mDOGE to your token account
// Transfer 1,000 mDOGE to loot vault

// 6. UPDATE LOOT TRACKING
loot_rewards.total_mdoge_accumulated += 1,000

// 7. AWARD MINING XP
// Formula: 15 XP per 1000 mDOGE mined
// Example: 10,000 mDOGE = 150 XP

// 8. UPDATE CLAIM INDEX
user.moondoge_claim_index = current_slot
```

**Result**:
- ✅ mDOGE tokens transferred to you
- ✅ 10% added to loot vault
- ✅ XP awarded for mining
- ✅ Claim index updated
- ✅ Daily login processed

**Mining Rate**: Dynamically adjusted every 8 hours based on mDOGE/SOL price

---

### 9. `claim_attraction_xp`

**Purpose**: Claim accumulated XP from attraction modules

**What Happens**:
```rust
// 1. VALIDATE MODULE
require!(module.type == Attraction)
require!(module.is_active)
require!(module.current_hp > 0)

// 2. CALCULATE TIME ELAPSED
current_time: 1000000
last_claim: 996400
elapsed_seconds: 3600
elapsed_hours: 1.0

// 3. CALCULATE XP GENERATION RATE
// Growth formula: base_xp_per_hour * (1.15^level)
base_xp: 50 XP/hour
upgrade_level: 3
current_rate: 50 * (1.15^3) = 76 XP/hour

// 4. APPLY HP EFFICIENCY
// Damaged modules generate less XP
current_hp: 80
max_hp: 100
efficiency: 80% (80/100)
effective_rate: 76 * 0.8 = 61 XP/hour

// 5. CALCULATE ACCUMULATED XP
xp_earned: 61 XP/hour * 1.0 hours = 61 XP

// 6. UPDATE MODULE STATE
module.total_xp_generated += 61
module.last_xp_claim = current_time

// 7. AWARD XP TO USER
user.xp += 61
```

**Parameters**:
- `module_index: u8` - Which attraction module to claim from

**Result**:
- ✅ XP transferred to user
- ✅ Module state updated
- ✅ Can claim again after time passes
- ✅ Daily login processed

**Cooldown**: Minimum 1 minute between claims

---

### 10. `claim_level_up_rewards`

**Purpose**: Process pending level-ups and roll for loot rewards

**What Happens**:
```rust
// 1. CHECK IF ENOUGH XP
current_xp: 500
required_xp: required_xp_new(level) = 120 * (1.35^level)
// Level 0 → 1: 120 XP
// Level 1 → 2: 162 XP
// Level 2 → 3: 219 XP

if current_xp < required_xp:
    return "Not enough XP"

// 2. CALCULATE LEVEL-UPS
// Can gain multiple levels in one transaction
potential_levels: 0
loop:
    if xp >= required_xp_new(level + potential_levels):
        potential_levels += 1
    else:
        break

// 3. PROCESS EACH LEVEL-UP
for each level:
    
    // A. DEDUCT XP
    user.xp -= required_xp_new(current_level)
    
    // B. INCREMENT LEVEL
    user.level += 1
    
    // C. ROLL FOR LOOT
    roll: random(0-9999) // basis points
    
    // D. CALCULATE LOOT CHANCE
    // Base chance varies by level tier:
    // Levels 1-4: 300bp + 20bp per level
    // Level 5, 10: 10,000bp (guaranteed!)
    // Levels 6-14: 300bp + 20bp per level
    // Levels 15-24: 1,500bp (if not milestone)
    // Level 25+: 2,500bp (if not milestone)
    
    // E. APPLY EXCLUSIVITY BONUS
    // If you're at global max level: 150% chance, 300% vault
    // If you're max-1: 140% chance, 250% vault
    // If you're max-2: 130% chance, 200% vault
    // If ≤3 users at level: 125% chance, 175% vault
    // If ≤10 users: 120% chance, 150% vault
    // If ≤25 users: 110% chance, 120% vault
    
    // F. ROLL RESULT
    if roll < final_chance:
        // WON LOOT!
        
        // G. CALCULATE PAYOUT
        // Normal: vault_cut% of total vault (100-800bp)
        // Milestone (level 10, 20, 30...): Try jackpot first!
        
        // H. JACKPOT CHECK (milestones only)
        if level % 10 == 0:
            jackpot_roll: random(0-9999)
            if jackpot_roll < 20: // 0.20% chance
                // Try to award jackpot pot:
                // 1000 SOL, 750 SOL, 690 SOL, 510 SOL, or 420 SOL
                // (whichever vault can afford)
        
        // I. DETERMINE CURRENCY
        // Milestone levels: prefer SOL
        // Regular levels: 50/50 coin flip (SOL vs mDOGE)
        
        // J. TRANSFER LOOT
        if sol_payout > 0:
            transfer_sol(loot_vault → user)
        if mdoge_payout > 0:
            transfer_mdoge(loot_vault → user)
        
        // K. UPDATE VAULT BALANCES
        loot.total_sol_accumulated -= sol_payout
        loot.total_mdoge_accumulated -= mdoge_payout
        loot.total_sol_distributed += sol_payout
        loot.total_mdoge_distributed += mdoge_payout
    
    // L. UPDATE LEVEL STATS (for exclusivity bonuses)
    level_stats.tracked_levels: update counts
    level_stats.max_level_achieved: max(current, new)
```

**Result**:
- ✅ Level increased
- ✅ XP reset (overflow carries to next level)
- 🎰 Loot won (if roll succeeded)
- ✅ Global level stats updated
- ✅ Events emitted

**Loot Limits**:
- Minimum: 0.01 SOL
- Maximum: 100 SOL per payout
- Vault protection: Max 10% of vault per payout

---

### 11. `claim_referral_rewards`

**Purpose**: Claim SOL earned from people who used your referral code

**What Happens**:
```rust
// 1. CALCULATE ACCOUNT BALANCE
account_balance: 1,000,000 lamports
rent_exempt_amount: 890,000 lamports
claimable: 110,000 lamports (0.00011 SOL)

// 2. CALCULATE NEW SOL FOR XP
// Track how much SOL is "new" since last XP claim
total_earned: 500,000 lamports
previously_claimed_for_xp: 400,000 lamports
new_sol: 100,000 lamports

// 3. AWARD XP FOR NEW EARNINGS
// Formula: sqrt(lamports) * 500 / 1e9
sqrt(100,000) = 316
xp: 316 * 500 / 1e9 = ~1 XP

// 4. UPDATE XP TRACKING
rewards.sol_claimed_for_xp = rewards.total_sol_earned

// 5. TRANSFER CLAIMABLE SOL
transfer(rewards_account → user, claimable)

// 6. PROCESS DAILY LOGIN
// Automatic daily login check + streak update
```

**Result**:
- ✅ SOL transferred to you
- ✅ XP awarded for new earnings
- ✅ Tracking updated
- ✅ Daily login processed

**How Referrals Work**:
- Someone uses your code → They pay creation/upgrade fees
- You earn 15% of their fees (if they don't use another referrer)
- Earnings accumulate in your ReferralRewards account
- Claim anytime, get SOL + XP

---

### 12. `update_user_electricity`

**Purpose**: Update your available electricity (called by external program)

**What Happens**:
```rust
// Called by MoonEconomy program when you:
// - Stake mDOGE → Increase electricity
// - Unstake mDOGE → Decrease electricity

// 1. VALIDATE AUTHORITY
require!(caller == global_config.ext_fee_collector)

// 2. UPDATE ELECTRICITY
if to_increase:
    user.available_electricity += amount
    global.total_active_electricity += amount
else:
    user.available_electricity -= amount
    require!(available >= used) // Can't remove in-use electricity
    global.total_active_electricity -= amount

// 3. PROCESS DAILY LOGIN
// Automatic
```

**Parameters**:
- `to_increase: bool` - Add or remove electricity
- `amount: u64` - How much to change

**Result**:
- ✅ Electricity updated
- ✅ Global stats updated
- ✅ Daily login processed

**Note**: Users don't call this directly - MoonEconomy program calls it

---

## XP & Leveling System

### How XP Works

**XP Sources**:
1. **Daily Login**: 10-100+ XP (increases with streak)
2. **Mining**: 15 XP per 1000 mDOGE claimed
3. **Module Purchase**: ~50 XP per 0.01 SOL spent
4. **Module Upgrade**: ~50 XP per 0.01 SOL spent
5. **Module Installation**: 25 XP (flat)
6. **Attraction Modules**: 50-400+ XP/hour (passive, scales with level)
7. **Expansions**: 100-300 XP (based on level requirement)
8. **Referral Earnings**: ~500 XP per SOL earned

### Daily Login Streak System

```
Day 1-7:    10 XP + streak (17 XP on day 7)
Day 8-14:   20 XP + streak (34 XP on day 14)
Day 15-30:  30 XP + streak (60 XP on day 30)
Day 31-60:  50 XP + (streak-20) (90 XP on day 60)
Day 61+:    60 XP + (streak-30) (100 XP max)

MILESTONE BONUSES:
Day 7:   +50 XP
Day 14:  +75 XP
Day 30:  +100 XP
Day 50:  +125 XP
Day 69:  +150 XP (nice)
Day 100: +200 XP
Day 365: +500 XP (1 year!)
Day 1000: +1000 XP
```

**Streak Rules**:
- Login within 24 hours → Continues
- Login within 24-48 hours → Continues (grace period)
- Login after 48+ hours → Resets to 1

### Level Requirements (Exponential Curve)

```
Formula: 120 × (1.35^level)

Level 0 → 1:   120 XP
Level 1 → 2:   162 XP
Level 2 → 3:   219 XP
Level 3 → 4:   295 XP
Level 4 → 5:   399 XP
Level 5 → 6:   538 XP
Level 10 → 11: 2,757 XP
Level 15 → 16: 14,128 XP
Level 20 → 21: 72,377 XP
```

**Why Exponential?**
- Early levels: Fast progression (hook new players)
- Mid levels: Steady grind (engagement)
- High levels: True dedication (status symbol)

---

## Module System

### Module Types

#### 1. **Mining Modules**
- **Purpose**: Generate hashpower for mDOGE mining
- **Stats**:
  - Base Hashpower (scales 1.15x per level)
  - Max HP
  - Power Consumption
- **Example**: Basic Miner
  - Cost: 0.01 SOL
  - Base Hashpower: 100
  - Level 5: 201 hashpower
  - Level 10: 405 hashpower

#### 2. **Attraction Modules**
- **Purpose**: Generate passive XP over time
- **Stats**:
  - Base XP/Hour (scales 1.15x per level)
  - Max HP
  - Power Consumption
- **Example**: Monument
  - Cost: 0.02 SOL
  - Base XP: 50/hour
  - Level 5: 101 XP/hour
  - Level 10: 203 XP/hour

### Module Lifecycle

```
BUY → INVENTORY (undeployed, no benefits)
  ↓
INSTALL → ACTIVE (on grid, consuming electricity, generating rewards)
  ↓
UPGRADE → ENHANCED (better stats, same position)
  ↓
REMOVE → INVENTORY (frees space, stops benefits)
  ↓
DELETE → GONE (permanent, rent refunded)
```

### HP System

Modules have HP that affects efficiency:
- **100% HP**: 100% efficiency
- **50% HP**: 50% efficiency
- **10% HP**: Minimum 10% efficiency (never completely broken)

**Efficiency Impact**:
- Mining: Hashpower reduced
- Attraction: XP/hour reduced

**Note**: HP/repair system not fully implemented (future PvP feature)

---

## Mining & Economy

### How Mining Works

```
GLOBAL POOL:
├─ Total mDOGE: 1,000,000,000 (pre-minted)
├─ Distribution Rate: 100 mDOGE per slot (adjustable)
└─ Adjustment: Every 8 hours based on price oracle

YOUR SHARE:
Your Hashpower:     1,000
Global Hashpower:  10,000
Your Share:           10%

REWARDS PER CLAIM:
Slots Passed: 1,000
Total Mined:  1,000 slots × 100 mDOGE = 100,000 mDOGE
Your Cut:     100,000 × 10% = 10,000 mDOGE
Loot Cut:     10,000 × 10% = 1,000 mDOGE to vault
You Get:      9,000 mDOGE
```

### Dynamic Distribution

Every 8 hours, the system:
1. Fetches mDOGE/SOL price from Raydium pool
2. Calculates 8-hour average price
3. Compares to previous 8-hour average
4. Adjusts distribution rate:
   - Price up 10% → Reduce rate 10%
   - Price down 10% → Increase rate 10%
5. Goal: Maintain stable USD value of rewards

### Electricity System

```
REQUIREMENT:
Each module needs electricity to run

SOURCES:
- Stake mDOGE in MoonEconomy program
- Receive electricity proportional to stake
- 1 mDOGE staked = X electricity (configurable)

USAGE:
Mining Module: 100 electricity
Attraction Module: 50 electricity

LIMITS:
used_electricity ≤ available_electricity
(Can't install modules without enough electricity)
```

---

## Loot System

### Casino-Style Rewards

When you level up, you **ALWAYS** roll for loot. No separate claim needed.

### Loot Probability by Level

```
TIER 1 (Levels 1-4):
Base: 3% + 0.2% per level
Max: 3.8% at level 4

TIER 2 (Level 5, 10, 15, 20...):
Base: 100% (GUARANTEED)
Vault cut: 0.5-8% of vault

TIER 3 (Levels 6-14):
Base: 3% + 0.2% per level
Max: 5.8% at level 14

TIER 4 (Levels 15-24, non-milestone):
Base: 15%

TIER 5 (Level 25+, non-milestone):
Base: 25%
```

### Exclusivity Bonuses

Being at the **top** levels gives massive bonuses:

```
GLOBAL MAX LEVEL:
Chance: 150% (1.5x multiplier)
Vault: 300% (3x multiplier)

MAX - 1 LEVEL:
Chance: 140%
Vault: 250%

MAX - 2 LEVEL:
Chance: 130%
Vault: 200%

≤3 USERS AT LEVEL:
Chance: 125%
Vault: 175%

≤10 USERS AT LEVEL:
Chance: 120%
Vault: 150%

≤25 USERS AT LEVEL:
Chance: 110%
Vault: 120%
```

**Example**:
- You're level 50 (global max)
- Base chance: 25% (2500bp)
- With bonus: 25% × 150% = **37.5% chance**
- Base vault cut: 8% (800bp)
- With bonus: 8% × 300% = **24% of vault**
- BUT capped at 10% max per payout

### Jackpot System (Milestone Levels Only)

At levels 10, 20, 30, 40, 50...

**Jackpot Chance**: 0.20% (20/10,000)

**Jackpot Pots** (awarded if vault can afford):
1. 1,000 SOL
2. 750 SOL
3. 690 SOL
4. 510 SOL
5. 420 SOL

**Requirements**:
- Vault must have 110% of pot (safety buffer)
- Only one pot awarded per jackpot

### Dual Currency System

Loot can be paid in **SOL**, **mDOGE**, or **both**:

```
MILESTONE LEVELS (10, 20, 30...):
Prefer SOL payout
Fallback to mDOGE if SOL vault low

REGULAR LEVELS:
50/50 coin flip between SOL and mDOGE
Award whichever vault has sufficient balance

CONVERSION:
Uses 8-hour average mDOGE/SOL price
Example: Want 1 SOL worth
- If price = 0.001 SOL per mDOGE
- Award 1000 mDOGE instead
```

### Loot Vault Accumulation

```
INPUTS (10% of all distributions):
1. Mining claims: 10% of claimed mDOGE
2. SOL fees: 10% of treasury withdrawals
3. Total accumulated over time

OUTPUTS (loot payouts):
1. Level-up rewards (SOL + mDOGE)
2. Milestone jackpots

TRACKING:
total_mdoge_accumulated: 50,000,000
total_sol_accumulated: 100 SOL
total_mdoge_distributed: 5,000,000
total_sol_distributed: 10 SOL
```

### Payout Limits (Safety)

```
MINIMUM: 0.01 SOL (prevents dust)
MAXIMUM: 100 SOL (prevents vault drain)
VAULT CAP: 10% of total vault (sustainability)
```

---

## Advanced Mechanics

### Grid Placement System

```
GRID: 20 × 15 tiles = 300 tiles max
STARTING: 10 × 8 tiles = 80 tiles

BITMAP STORAGE:
300 tiles = 38 bytes (300 bits)
Each bit: 0 = empty, 1 = occupied

PLACEMENT RULES:
1. Module must fit within current_width × current_height
2. No overlap with existing modules
3. Each module occupies width × height tiles

EXAMPLE:
2×2 Miner at (5, 3):
Occupies tiles: (5,3), (6,3), (5,4), (6,4)
```

### Referral System

```
FLOW:
User A creates moonbase with no referrer
  → Gets referral code (their pubkey)
  → Shares code with friends

User B creates moonbase with User A's code
  → Pays 0.1 SOL creation fee
  → Split: 50% to creation_fee_recipient, 50% processed
  → Of 50%: 15% to User A, 85% to treasury
  → User A earns 0.0075 SOL

User B upgrades module for 0.01 SOL
  → 15% to User A (0.0015 SOL)
  → 85% to treasury

REWARDS:
User A can claim anytime via claim_referral_rewards()
  → Gets SOL + XP for earnings
```

### Hashpower Calculation

```
FORMULA: base_hashpower * (1.15^upgrade_level)

EXAMPLE (100 base hashpower):
Level 0:  100
Level 1:  115 (+15%)
Level 2:  132 (+32%)
Level 3:  152 (+52%)
Level 5:  201 (+101%)
Level 10: 405 (+305%)

GLOBAL SHARE:
Your hashpower / Total hashpower = Your %
1,000 / 10,000 = 10% of all mined tokens
```

---

## Events & Tracking

All major actions emit events for frontend/analytics:

- `UserMoonBaseCreated`
- `MoonbaseExpanded`
- `ModulePurchased`
- `ModuleInstalled`
- `ModuleInstanceRemoved`
- `ModuleDeleted`
- `ModuleInstanceUpgraded`
- `AttractionXPClaimed`
- `MoonDogeTokensClaimed`
- `LevelUp`
- `LootWon`
- `XpGained`
- `DailyLoginReward`
- `ReferralRewardsAdded`
- `ReferralRewardsClaimed`
- `ElectricityUpdated`

---

## Function Call Order (Typical Player Journey)

```
1. create_user_moonbase(referrer, faction)
   └─> MoonBase created at 10×8

2. update_user_electricity() [by external program]
   └─> Staked mDOGE → Got electricity

3. buy_module(mining_module_id)
   └─> Miner in inventory

4. install_module(0, pos_x, pos_y)
   └─> Miner on grid, generating hashpower

5. claim_mdoge_tokens()
   └─> Claimed mined tokens + XP

6. claim_level_up_rewards()
   └─> Leveled up! Rolled for loot, won 0.5 SOL

7. buy_module(attraction_module_id)
   └─> Monument in inventory

8. install_module(1, pos_x2, pos_y2)
   └─> Monument on grid, generating XP/hour

9. claim_attraction_xp(1)
   └─> Claimed passive XP

10. upgrade_module(0)
    └─> Miner level 1, more hashpower

11. expand_moonbase(expansion_id)
    └─> More grid space unlocked

12. claim_referral_rewards()
    └─> Claimed SOL from referrals

REPEAT 5-12 indefinitely →
```

---

## FAQ

**Q: Can I move modules after installing?**
A: Not implemented yet. Remove and reinstall instead.

**Q: What happens to modules when I remove them?**
A: They go back to inventory (undeployed). You can reinstall them anytime.

**Q: Do I lose XP when I level up?**
A: Excess XP carries over. Example: Need 100 XP, have 150, level up with 50 XP remaining.

**Q: Can I get multiple loot rewards in one transaction?**
A: Yes! If you gain multiple levels, each level-up rolls for loot independently.

**Q: What if loot vault is empty?**
A: You still level up, just don't get loot. Vault refills from 10% of all fees/mining.

**Q: How do I get more electricity?**
A: Stake mDOGE in the MoonEconomy program (separate contract).

**Q: Can I have multiple moonbases?**
A: No, one per wallet. But you can use multiple wallets!

**Q: What's the max level?**
A: No hard cap, but XP requirements grow exponentially (1.35x per level).

**Q: Do I have to claim level-ups manually?**
A: Yes, via `claim_level_up_rewards()`. Loot transfers require this explicit call for security.

---

## Technical Details

### Account Structure

```
GlobalConfig (PDA):
- Program settings
- Faction list
- Expansion configs

MoonDogeMining (PDA):
- Global mining state
- Total hashpower
- Distribution rate
- Price oracle data

UserMoonBaseInstance (PDA per user):
- Your moonbase state
- Level, XP, hashpower
- Grid bitmap
- Module counts

ModuleInstance (PDA per module):
- Individual module state
- Position, level, HP
- Runtime stats

ReferralRewards (PDA per user):
- Earned SOL tracking
- Referral count

LootRewards (PDA):
- Loot vault balances
- Distribution tracking

LevelStats (PDA):
- Top level tracking
- Exclusivity bonuses
```

### PDA Derivation

```
user_moonbase:
  seeds: ["user-moonbase", user.pubkey]

module_instance:
  seeds: ["module-instance", user.pubkey, module_index]

referral_rewards:
  seeds: ["referral-rewards", user.pubkey]

module_config:
  seeds: ["module-config", config_id]
```

---

## Summary

MoonBase is a **complex idle game** with:
- ✅ **Mining**: Hashpower-based mDOGE token generation
- ✅ **Building**: Grid-based module placement system
- ✅ **Progression**: XP & leveling with daily login streaks
- ✅ **Gambling**: Casino-style loot rewards on level-up
- ✅ **Economy**: SOL + mDOGE dual token system
- ✅ **Social**: Referral rewards system
- ✅ **Strategy**: Module placement, electricity management, upgrade paths

**Core Loop**: Build base → Mine tokens → Gain XP → Level up → Win loot → Expand → Repeat

**Monetization**: Players pay SOL for modules, upgrades, expansions. 10% goes to loot vault, creating a fun lottery-style reward system.

---

*Last Updated: October 2025*
*Version: 1.0.0*

