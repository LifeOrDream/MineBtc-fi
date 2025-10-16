# ⭐ XP & Leveling System: Complete Technical Documentation

> **Player Progression Mechanics** | Version 1.0.0 | October 15, 2025

---

## Overview

The XP (Experience Points) system drives player progression through an **exponential leveling curve** that rewards diverse activities. Players gain XP from daily engagement, module management, mining, and social interactions.

---

## XP Sources & Amounts

### Fixed XP Rewards

```rust
// Constants
XP_DAILY_LOGIN: 10 XP
XP_MODULE_INSTALL: 50 XP
XP_MODULE_UPGRADE: 30 XP
XP_MINING_1000_MDOGE: 15 XP

// Activities
Daily Login:           10 XP (once per 24 hours)
Install Module:        50 XP (per module deployed)
Upgrade Module:        30 XP (per upgrade level)
Buy Module:            0 XP (inventory only, XP on deploy)
Remove Module:         0 XP (no penalty or reward)
Delete Module:         0 XP
```

### Dynamic XP Rewards

#### Mining XP
```rust
fn calculate_mining_xp(claimed_amount: u64) -> u32 {
    // 15 XP per 1,000 DBTC mined
    ((claimed_amount / 1000) * (XP_MINING_1000_MDOGE as u64)) as u32
}

Examples:
- Claim 5,000 DBTC → 75 XP
- Claim 50,000 DBTC → 750 XP
- Claim 500,000 DBTC → 7,500 XP
```

#### Expansion XP
```rust
fn calculate_expansion_xp(expansion: &ExpansionConfig) -> u32 {
    100 + (expansion.required_level as u32 × 10)
}

Examples:
- Level 5 expansion → 100 + (5 × 10) = 150 XP
- Level 10 expansion → 100 + (10 × 10) = 200 XP
- Level 20 expansion → 100 + (20 × 10) = 300 XP
```

#### Referral XP
```rust
fn calculate_referral_xp(
    total_sol_earned: u64,
    sol_already_counted: u64,
) -> u32 {
    let new_sol = total_sol_earned - sol_already_counted;
    
    // sqrt scaling: 500 XP per SOL (square root of lamports)
    let sqrt_lamports = integer_sqrt(new_sol);
    (sqrt_lamports × 500 / 1_000_000_000) as u32
}

Examples:
- Earn 1 SOL → √(1B) × 500 / 1B ≈ 16 XP
- Earn 4 SOL → √(4B) × 500 / 1B ≈ 32 XP
- Earn 9 SOL → √(9B) × 500 / 1B ≈ 47 XP
- Earn 100 SOL → √(100B) × 500 / 1B ≈ 158 XP

Note: sqrt scaling prevents farming, rewards consistent referral business
```

---

## Level Progression Curve

### Formula

```rust
// Exponential growth
required_xp(level) = 120 × (1.35^level)

XP_BASE: 120
XP_CURVE_NUM: 135 (1.35 in fixed-point)
XP_CURVE_DEN: 100

// Rounded to nearest 10 for clean numbers
```

### Level Table (Complete)

| Level | XP Required | Cumulative XP | Daily Logins | Modules Installed | DBTC Mined |
|-------|-------------|---------------|--------------|-------------------|------------|
| 1 | 162 | 162 | 16 | 3 | 10,800 |
| 2 | 219 | 381 | 22 | 4 | 14,600 |
| 3 | 296 | 677 | 30 | 6 | 19,700 |
| 4 | 399 | 1,076 | 40 | 8 | 26,600 |
| 5 | 539 | 1,615 | 54 | 11 | 35,900 |
| 6 | 728 | 2,343 | 73 | 15 | 48,500 |
| 7 | 983 | 3,326 | 98 | 20 | 65,500 |
| 8 | 1,327 | 4,653 | 133 | 27 | 88,500 |
| 9 | 1,791 | 6,444 | 179 | 36 | 119,400 |
| 10 | 2,418 | 8,862 | 242 | 48 | 161,200 |
| 15 | 9,031 | 36,134 | 903 | 181 | 602,000 |
| 20 | 33,748 | 135,296 | 3,375 | 675 | 2,249,000 |
| 25 | 126,094 | 505,968 | 12,609 | 2,522 | 8,406,000 |
| 30 | 471,193 | 1,892,775 | 47,119 | 9,424 | 31,413,000 |

### Time Estimates

**Casual Player (10 XP/day from logins only):**
- Level 5: ~162 days (5 months)
- Level 10: ~886 days (2.4 years)
- Level 15: ~10 years
- Level 20+: Impossible with logins alone

**Active Player (100 XP/day from logins + mining + modules):**
- Level 5: ~16 days
- Level 10: ~89 days (3 months)
- Level 15: ~361 days (1 year)
- Level 20: ~1,353 days (3.7 years)
- Level 25: ~5,060 days (13.8 years)

**Hardcore Player (500 XP/day from heavy mining + modules + referrals):**
- Level 5: ~3 days
- Level 10: ~18 days
- Level 15: ~72 days (2.5 months)
- Level 20: ~271 days (9 months)
- Level 25: ~1,012 days (2.8 years)
- Level 30: ~3,786 days (10.4 years)

**Whale Player (2,000 XP/day from massive mining operation):**
- Level 5: ~1 day
- Level 10: ~4 days
- Level 15: ~18 days
- Level 20: ~68 days (2 months)
- Level 25: ~253 days (8.5 months)
- Level 30: ~946 days (2.6 years)

---

## Daily Login System

### Implementation

```rust
pub struct UserMoonBaseInstance {
    last_login_ts: i64,          // Last login timestamp
    daily_login_streak: u16,     // Consecutive days
}

fn process_daily_login(user: &mut UserMoonBaseInstance) -> Result<(u32, u16)> {
    let current_time = Clock::get()?.unix_timestamp;
    let time_since_last = current_time - user.last_login_ts;
    
    const DAY: i64 = 86_400; // seconds
    
    // Check if 24 hours passed
    if time_since_last >= DAY {
        // Award daily login XP
        user.xp += XP_DAILY_LOGIN;
        user.last_login_ts = current_time;
        
        // Update streak
        if time_since_last < DAY × 2 {
            // Within 48 hours = streak continues
            user.daily_login_streak += 1;
        } else {
            // Over 48 hours = streak resets
            user.daily_login_streak = 1;
        }
        
        msg!("✅ Daily login: +10 XP (Streak: {})", user.daily_login_streak);
        
        return Ok((XP_DAILY_LOGIN, user.daily_login_streak));
    }
    
    // No login today
    Ok((0, user.daily_login_streak))
}
```

### Streak Mechanics

```
Day 1: Login → streak = 1
Day 2: Login (24h later) → streak = 2
Day 3: Login (24h later) → streak = 3
Day 4: Skip (no login)
Day 5: Login (72h after Day 3)
       ├─→ Exceeded 48h grace period
       └─→ streak = 1 (RESET!)
```

**Grace Period:**
- 24-48 hours: Streak continues
- Over 48 hours: Streak resets to 1

**Future Enhancement:**
```rust
// Streak bonuses (not currently implemented)
fn daily_login_bonus(streak: u16) -> u32 {
    match streak {
        7 => 20,   // Week bonus
        30 => 100, // Month bonus
        100 => 500, // 100-day achievement
        _ => 0,
    }
}
```

---

## Level-Up Process

### Automatic Level-Up

```rust
// Every time XP is added
fn add_xp_and_maybe_level_up(
    user: &mut UserMoonBaseInstance,
    xp_amount: u32,
) -> Result<bool> {
    user.xp += xp_amount;
    
    // Try to level up (may level up multiple times)
    let mut leveled_up = false;
    loop {
        let required = required_xp_new(user.level);
        
        if user.xp >= required {
            user.xp -= required;  // Consume XP
            user.level += 1;      // Level up!
            leveled_up = true;
            
            msg!("🎉 LEVEL UP! Now level {}", user.level);
            
            // Try loot roll
            if loot_rewards.is_some() && level_stats.is_some() {
                try_roll_loot(user, loot_rewards, level_stats, doge_btc_mining)?;
            }
        } else {
            break; // Not enough XP for next level
        }
    }
    
    Ok(leveled_up)
}
```

### Required XP Calculation

```rust
fn required_xp_new(level: u8) -> u32 {
    if level == 0 {
        return 0;
    }
    
    let mut num: u64 = XP_BASE;  // 120
    
    // Calculate (1.35)^level
    for _ in 0..level {
        num = (num × XP_CURVE_NUM) / XP_CURVE_DEN;
        // num × 135 / 100
    }
    
    // Round to nearest 10
    let rounded = ((num + 5) / 10) × 10;
    
    rounded.min(u32::MAX as u64) as u32
}
```

**Examples:**
```
Level 1: 120 × 1.35^1 = 162 (rounded: 160)
Level 2: 120 × 1.35^2 = 218.7 (rounded: 220)
Level 5: 120 × 1.35^5 = 538.9 (rounded: 540)
Level 10: 120 × 1.35^10 = 2,418 (rounded: 2,420)
```

### Multi-Level Ups

```rust
// If user gains massive XP (e.g., 1,000 XP)
// And they're at level 3 (needs 296 XP)

Iteration 1:
- Current: Level 3, XP: 0
- Add 1,000 XP → XP: 1,000
- Required for level 4: 296
- 1,000 >= 296? YES!
- Level 3 → 4, XP: 1,000 - 296 = 704
- Roll loot for level 4

Iteration 2:
- Current: Level 4, XP: 704
- Required for level 5: 399
- 704 >= 399? YES!
- Level 4 → 5, XP: 704 - 399 = 305
- Roll loot for level 5 (milestone!)

Iteration 3:
- Current: Level 5, XP: 305
- Required for level 6: 539
- 305 >= 539? NO
- Stop, remain at level 5 with 305 XP

Result: Gained 2 levels in one action!
```

---

## XP Activity Integration

### Mining Integration

```rust
pub fn claim_dbtc_tokens_internal(ctx: Context<ClaimDogeBtc>) -> Result<()> {
    // ... claim tokens
    
    // Calculate mining XP
    let mining_xp = helper::calculate_mining_xp(claimed_amount);
    
    // Process daily login and add XP
    process_daily_login_and_xp(
        user_moonbase,
        mining_xp,
        "Mining",
    )?;
    
    // This may trigger level-ups and loot rolls!
    
    Ok(())
}
```

### Module Installation

```rust
pub fn install_module(ctx: Context<InstallModule>, ...) -> Result<()> {
    // ... place module on grid
    
    // Award XP
    const INSTALL_XP: u32 = 50;
    process_daily_login_and_xp(
        user_moonbase,
        INSTALL_XP,
        "Module Installation",
    )?;
    
    Ok(())
}
```

### Module Upgrade

```rust
pub fn upgrade_module_internal(ctx: Context<UpdateModuleInstance>, ...) -> Result<()> {
    // ... upgrade module
    
    // Award XP
    const UPGRADE_XP: u32 = 30;
    process_daily_login_and_xp(
        user_moonbase,
        UPGRADE_XP,
        "Module Upgrade",
    )?;
    
    Ok(())
}
```

### Expansion Purchase

```rust
pub fn expand_moonbase_internal(ctx: Context<ExpandMoonbase>, ...) -> Result<()> {
    // ... expand base
    
    // Award scaled XP
    let expansion_xp = 100 + (expansion.required_level as u32 × 10);
    process_daily_login_and_xp(
        user_moonbase,
        expansion_xp,
        "Moonbase Expansion",
    )?;
    
    Ok(())
}
```

### Referral Claiming

```rust
pub fn claim_referral_rewards_internal(ctx: Context<ClaimReferralRewards>) -> Result<()> {
    // Calculate new SOL earned since last XP claim
    let new_sol = rewards.total_sol_earned - rewards.sol_claimed_for_xp;
    
    // sqrt scaling
    let sol_bonus_xp = if new_sol > 0 {
        let sqrt_lamports = integer_sqrt(new_sol);
        sqrt_lamports × 500 / 1_000_000_000
    } else {
        0
    };
    
    // Award XP
    process_daily_login_and_xp(
        user_moonbase,
        sol_bonus_xp,
        "Referral SOL Earnings",
    )?;
    
    // Mark SOL as counted
    rewards.sol_claimed_for_xp = rewards.total_sol_earned;
    
    Ok(())
}
```

---

## Daily Login Deep Dive

### Automatic Processing

**Every function that awards XP automatically processes daily login first:**

```rust
fn process_daily_login_and_xp(
    user: &mut UserMoonBaseInstance,
    activity_xp: u32,
    source: &str,
) -> Result<()> {
    // 1. Process daily login (if 24h passed)
    let (login_xp, streak) = helper::process_daily_login(user)?;
    
    // 2. Combine with activity XP
    let total_xp = login_xp + activity_xp;
    
    // 3. Award and maybe level up (with loot)
    if total_xp > 0 {
        helper::add_xp_and_maybe_level_up(
            user,
            total_xp,
            source,
            // ... loot context
        )?;
    }
    
    Ok(())
}
```

**Benefits:**
- No separate "daily login" function needed
- Can't forget to claim
- Every action checks for daily reset
- Streak tracking is automatic

### Streak Tracking

```rust
pub struct UserMoonBaseInstance {
    last_login_ts: i64,
    daily_login_streak: u16,  // Max 65,535 days
}

// In process_daily_login():
let time_since = current_time - user.last_login_ts;

if time_since >= 86_400 {  // 24 hours
    user.xp += 10;
    user.last_login_ts = current_time;
    
    if time_since < 172_800 {  // 48 hours
        user.daily_login_streak += 1;  // Continue streak
    } else {
        user.daily_login_streak = 1;   // Reset streak
    }
}
```

**Streak Scenarios:**

```
Scenario 1: Consistent Daily Login
Day 1: Login at 9am → streak = 1
Day 2: Login at 10am (25h later) → streak = 2
Day 3: Login at 8am (22h later) → streak = 3
Day 4: Login at 11am (27h later) → streak = 4
All within 48h windows ✅

Scenario 2: Missed Day
Day 1: Login at 9am → streak = 1
Day 2: Login at 10am → streak = 2
Day 3: No login ⏰
Day 4: Login at 11am (49h after Day 2) → streak = 1 (RESET!)
Exceeded 48h ❌

Scenario 3: Grace Period
Day 1: Login at 9am → streak = 1
Day 2: Login at 6pm (33h later) → streak = 2
Within 48h grace period ✅
```

---

## Level-Up Rewards Integration

### Claim Level-Up Rewards Function

```rust
pub fn claim_level_up_rewards_internal(ctx: Context<ClaimLevelUpRewards>) -> Result<()> {
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let old_level = user_moonbase.level;
    
    msg!("🎉 Processing level-ups for user {} (Level: {}, XP: {})", 
         ctx.accounts.user.key(), old_level, user_moonbase.xp);
    
    // Process any pending level-ups with loot system
    process_auto_daily_login_and_activity_xp(
        user_moonbase,
        0, // No new XP, just convert existing XP to levels
        "Level-Up Claim",
        &mut ctx.accounts.loot_rewards,
        &mut ctx.accounts.level_stats,
        &ctx.accounts.doge_btc_mining,
        // ... vault accounts for loot distribution
    )?;
    
    let levels_gained = user_moonbase.level.saturating_sub(old_level);
    msg!("✅ Level-up complete: {} -> {} (+{} levels)", old_level, user_moonbase.level, levels_gained);
    
    Ok(())
}
```

**When to call:**
- User accumulated XP but hasn't leveled yet
- Want to manually trigger level-up and loot roll
- Check if any levels are pending

**Note:** Most level-ups happen automatically during other actions (mining, installing modules, etc.)

---

## XP Optimization Strategies

### Fast Leveling (1-10)

**Focus: Module spam**
```
Buy and install 20 modules:
- Cost: ~1-2 SOL
- XP: 20 × 50 = 1,000 XP
- Reaches: Level ~6-7

Daily login for 30 days:
- XP: 30 × 10 = 300 XP

Mine 100,000 DBTC:
- XP: (100,000 / 1,000) × 15 = 1,500 XP

Total: ~2,800 XP → Level 8-9
Time: ~1 month
```

### Mid-Game Leveling (10-20)

**Focus: Mining + Upgrades**
```
Upgrade 10 modules to level 5:
- Each upgrade: 30 XP
- Total upgrades: 10 × 5 = 50
- XP: 50 × 30 = 1,500 XP

Daily logins for 90 days:
- XP: 90 × 10 = 900 XP

Mine 5,000,000 DBTC (heavy mining):
- XP: (5M / 1,000) × 15 = 75,000 XP

Total: ~77,400 XP → Level 17-18
Time: ~3 months
```

### End-Game Leveling (20-30)

**Focus: Massive Mining + Referrals**
```
Heavy mining (50M DBTC):
- XP: (50M / 1,000) × 15 = 750,000 XP

Referrals earning 100 SOL:
- XP: √(100 × 10^9) × 500 / 10^9 ≈ 158 XP
- (Plus base: 10 referrals × 100 = 1,000 XP)

Daily logins for 365 days:
- XP: 365 × 10 = 3,650 XP

Total: ~755,000 XP → Level 24-26
Time: ~1 year of hardcore grinding
```

---

## State Updates & Events

### XP Gain Event

```rust
#[event]
pub struct XpGained {
    pub owner: Pubkey,
    pub xp_amount: u32,
    pub xp_source: String,    // "Mining", "Daily Login", etc.
    pub total_xp: u32,
    pub current_level: u8,
}
```

### Level-Up Event

```rust
#[event]
pub struct LevelUp {
    pub owner: Pubkey,
    pub old_level: u8,
    pub new_level: u8,
    pub total_xp: u32,
    pub xp_remaining: u32,
}
```

### Level Stats Update Event

```rust
#[event]
pub struct LevelStatsUpdated {
    pub user: Pubkey,
    pub old_level: u8,
    pub new_level: u8,
    pub total_users: u32,
    pub users_at_new_level: u32,
}
```

---

## Frontend Integration

### Display Current Progress

```typescript
const moonbase = await program.account.userMoonBaseInstance.fetch(pda);

const currentLevel = moonbase.level;
const currentXP = moonbase.xp;
const requiredXP = calculateRequiredXP(currentLevel);
const progress = (currentXP / requiredXP) * 100;

console.log(`Level ${currentLevel}: ${currentXP} / ${requiredXP} XP (${progress.toFixed(1)}%)`);
```

### Calculate Required XP

```typescript
function calculateRequiredXP(level: number): number {
  if (level === 0) return 0;
  
  let num = 120; // XP_BASE
  
  // Calculate (1.35)^level
  for (let i = 0; i < level; i++) {
    num = (num * 135) / 100;
  }
  
  // Round to nearest 10
  return Math.round(num / 10) * 10;
}
```

### XP Sources Breakdown

```typescript
// Show user where XP comes from
const xpSources = {
  dailyLogin: {
    amount: 10,
    frequency: "Daily",
    total: streakDays * 10,
  },
  mining: {
    amount: 15,
    per: "1,000 DBTC",
    total: (totalDBTCMined / 1000) * 15,
  },
  moduleInstalls: {
    amount: 50,
    count: modulesInstalled,
    total: modulesInstalled * 50,
  },
  moduleUpgrades: {
    amount: 30,
    count: totalUpgrades,
    total: totalUpgrades * 30,
  },
  expansions: {
    amount: "100-300",
    count: expansionsPurchased,
    total: expansionXP,
  },
  referrals: {
    amount: "Variable",
    solEarned: referralSOL,
    total: calculateReferralXP(referralSOL),
  },
};
```

### Predict Next Level

```typescript
function timeToNextLevel(
  currentXP: number,
  currentLevel: number,
  xpPerDay: number
): number {
  const required = calculateRequiredXP(currentLevel);
  const remaining = required - currentXP;
  const daysNeeded = Math.ceil(remaining / xpPerDay);
  
  return daysNeeded;
}

// Example usage
const daysToLevel11 = timeToNextLevel(
  moonbase.xp,
  moonbase.level,
  150 // User's avg XP/day
);

console.log(`You'll reach level ${moonbase.level + 1} in ~${daysToLevel11} days`);
```

---

## Testing Scenarios

### Test 1: Daily Login Streak
```
Day 0: Create moonbase (level 0, xp 0, streak 0)
Day 1: Login → +10 XP, streak = 1
Day 2: Login → +10 XP, streak = 2
Day 3: Skip
Day 4: Login (72h later) → +10 XP, streak = 1 (reset)

Expected: Total 30 XP, streak = 1
```

### Test 2: Module Installation XP
```
Install 10 modules:
- Expected: 10 × 50 = 500 XP
- Should trigger level-ups (multiple!)
- Level 0 → Level 3-4
```

### Test 3: Massive Mining XP
```
Mine and claim 1,000,000 DBTC:
- Expected: (1M / 1,000) × 15 = 15,000 XP
- Should reach level 10+ from level 0
- Multiple loot rolls
```

### Test 4: Referral XP
```
Referrer earns 16 SOL from referrals:
- Expected: √(16 × 10^9) × 500 / 10^9 ≈ 63 XP
- Claim triggers XP award
- Can only claim this XP once (tracked in sol_claimed_for_xp)
```

---

## Common Patterns

### Processing XP with Loot

```rust
// Most common pattern in codebase
process_daily_login_and_xp(
    user_moonbase,
    xp_amount,
    "Activity Name",
)?;

// This automatically:
// 1. Checks if 24h passed → award 10 XP
// 2. Adds activity XP
// 3. Tries to level up
// 4. Rolls loot if leveled up
// 5. Emits events
```

### Processing XP Without Loot

```rust
// For functions that don't have loot context
let (_login_xp, _streak) = helper::process_daily_login(user)?;
user.xp += activity_xp;

// No automatic level-up
// User calls claim_level_up_rewards() later
```

---

## Level Gating

### Module Requirements

```rust
pub struct ModuleConfig {
    min_level: u8,  // Required to buy this module
    upgrade_level_requirements: Vec<u8>,  // Required per upgrade
}

// Example
ModuleConfig {
    name: "Advanced Mining Rig",
    min_level: 10,  // Can't buy until level 10
    upgrade_level_requirements: [15, 20, 25],  // Upgrade gates
}

// Buying check
require!(
    user.level >= module_config.min_level,
    ErrorCode::UserLevelTooLow
);

// Upgrade check
let next_level = module.upgrade_level + 1;
let required = module_config.upgrade_level_requirements[next_level - 1];
require!(
    user.level >= required,
    ErrorCode::UserLevelTooLow
);
```

### Expansion Requirements

```rust
pub struct ExpansionConfig {
    required_level: u8,
}

// Example
ExpansionConfig {
    name: "Northern Sector",
    required_level: 15,  // Level gate
    cost_sol: 5_000_000_000,
}

// Purchase check
require!(
    user.level >= expansion.required_level,
    ErrorCode::UserLevelTooLow
);
```

---

## Strategic XP Paths

### Path A: Module Spammer (Fast Early Levels)

**Investment: ~2 SOL**
```
Buy 40 modules (0.05 SOL each):
- Cost: 2 SOL
- XP: 0 (not installed yet)

Install all 40 modules:
- XP: 40 × 50 = 2,000 XP
- Levels: 0 → ~7

Time: 1 day
Result: Fast level 7, but limited long-term growth
```

### Path B: Grind Miner (Slow but Sustainable)

**Investment: Time + electricity**
```
Daily routine:
- Login: 10 XP
- Mine 50,000 DBTC: 750 XP
- Total: 760 XP/day

30 days:
- XP: 22,800 XP
- Levels: 0 → ~11

Time: 1 month
Result: Steady progression, sustainable
```

### Path C: Whale Speedrun (Expensive but Fast)

**Investment: ~50-100 SOL**
```
Day 1:
- Buy 50 modules: ~2.5 SOL, 0 XP (inventory)
- Install all: 2,500 XP → Level 8
- Upgrade 20 to level 2: ~1.5 SOL, 600 XP → Level 9
- Expansion: 2 SOL, 200 XP → Level 9+

Week 1:
- Heavy mining (5M DBTC): 75,000 XP → Level 17
- More upgrades: 1,000 XP
- Daily logins: 70 XP

Week 2-4:
- Continue mining: 300,000 XP
- Referrals (100 SOL earned): ~1,150 XP
- More expansions: 500 XP

Total: ~380,000 XP → Level 22-23
Time: 1 month
Cost: ~50 SOL
Result: Legendary tier in 1 month!
```

---

## XP Events for Analytics

### Tracking XP Sources

```typescript
// Subscribe to XP events
connection.onLogs(programId, (logs) => {
  const events = parseEventsFromLogs(logs);
  
  events.forEach(event => {
    if (event.name === "XpGained") {
      trackXP({
        user: event.owner,
        amount: event.xpAmount,
        source: event.xpSource,
        timestamp: Date.now(),
      });
    }
  });
});

// Analytics queries
const xpBySource = await db.query(`
  SELECT xp_source, SUM(xp_amount) as total
  FROM xp_events
  WHERE user = $1
  GROUP BY xp_source
`, [userPubkey]);

// Results:
// Mining:     145,000 XP (72%)
// Modules:    35,000 XP (17%)
// Daily:      2,100 XP (1%)
// Referrals:  20,000 XP (10%)
```

---

## Summary

### Key Features
✅ **Exponential curve** (1.35^level) - gets harder over time  
✅ **Multiple XP sources** - diverse activities rewarded  
✅ **Automatic daily login** - no separate call needed  
✅ **Streak tracking** - 48-hour grace period  
✅ **Integrated with loot** - level-ups trigger rewards  
✅ **Level gating** - unlocks content progressively  
✅ **No XP cap** - infinite progression possible  

### Balance Analysis
✅ **Early levels** (1-10): Fast and rewarding (days to weeks)  
✅ **Mid levels** (10-20): Challenging but achievable (months)  
✅ **Late levels** (20-30): Hardcore grinding (years)  
✅ **Exclusivity bonuses**: Rewards first achievers heavily  

### Economic Impact
✅ **Retention driver**: Long progression keeps players engaged  
✅ **Monetization**: Module purchases for XP  
✅ **Social features**: Referrals provide XP  
✅ **Mining incentive**: XP from mining encourages claims  

---

**The XP system is well-balanced for long-term engagement with clear progression milestones and diverse earning methods!**



