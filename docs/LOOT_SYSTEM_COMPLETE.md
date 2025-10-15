# 🎰 Loot System: Complete Technical Documentation

> **Casino-Style Reward Distribution** | Version 1.0.0 | October 15, 2025

---

## Overview

The loot system distributes **dual-currency rewards (SOL + DBTC)** to players when they level up. It combines probability-based rolls, guaranteed milestones, and massive jackpots with exclusivity bonuses for early achievers.

---

## Core Mechanics

### Accumulation (10% of Everything)

```rust
// Automatic funding from two sources:

Source 1: Mining Rewards
├─→ User mines DBTC tokens
├─→ 90% goes to user
└─→ 10% goes to Loot DBTC Vault

Source 2: SOL Fees
├─→ User pays SOL (modules, upgrades, expansions)
├─→ 90% goes to treasury/referrer
└─→ 10% goes to Loot SOL Vault

No manual funding required - self-sustaining!
```

### Distribution (On Level-Up)

```rust
// Triggered automatically when user levels up
process_auto_daily_login_and_activity_xp() {
    // ... XP processing
    
    if user leveled up {
        try_roll_loot(user, loot_rewards, level_stats)?;
    }
}
```

---

## Loot Tiers & Probabilities

### Tier Table

| Level Range | Tier | Base Chance | Vault Cut | Guaranteed? | Special |
|-------------|------|-------------|-----------|-------------|---------|
| **1-4** | Minor | 3% + 0.2%×level | 1% | No | Entry |
| **5** | Milestone | 100% | 0.5% | **YES** | First milestone |
| **6-14** | Minor | 3% + 0.2%/lvl | 1% | No | Regular |
| **10** | Milestone | 100% | 0.5% | **YES** | + Jackpot chance |
| **15** | Rare Milestone | 100% | 2% | **YES** | Rare tier entry |
| **15-24** | Rare | 15% | 5% | No | High tier |
| **20** | Rare Milestone | 100% | 2% | **YES** | + Jackpot chance |
| **25+** | Legendary | 25% | 8% | No | Elite tier |
| **30, 35, 40...** | Legendary Milestone | 100% | 4% | **YES** | + Jackpot chance |
| **10, 20, 30...** | Jackpot Wheel | 0.20% | Fixed Pots | Special | Mega wins |

### Probability Examples

**Level 3 (Minor):**
```
Base chance: 3% + (0.2% × 3) = 3.6%
Vault cut: 1%
Guaranteed: No

If vault has 100 SOL:
Payout if won: 1 SOL + equivalent DBTC
Win chance: 3.6%
```

**Level 10 (Milestone + Jackpot):**
```
Milestone roll:
- Chance: 100% (guaranteed)
- Vault cut: 0.5%
- Payout: 0.5 SOL + DBTC

THEN jackpot roll:
- Chance: 0.20%
- Payout: 420-1,000 SOL (fixed pots)
- Requires vault ≥ pot × 1.1
```

**Level 25 (Legendary):**
```
Base chance: 25%
Vault cut: 8%

If vault has 500 SOL:
Payout if won: 40 SOL + equivalent DBTC
Win chance: 25%
```

---

## Exclusivity Bonus System

### How It Works

The system tracks the **top 10 levels** globally and counts how many users are at each level.

```rust
pub struct LevelStats {
    tracked_levels: Vec<LevelEntry>,  // Max 10 entries
    max_level_achieved: u8,
    total_users: u32,
}

pub struct LevelEntry {
    level: u8,
    user_count: u32,
}
```

### Bonus Multipliers

| Rank | Users at Level | Chance Mult | Vault Mult | Description |
|------|----------------|-------------|------------|-------------|
| 🥇 **First** | 1 | ×1.20 | ×2.00 | Only player |
| 🥈 **Top 3** | 2-3 | ×1.15 | ×1.50 | Elite |
| 🥉 **Top 10** | 4-10 | ×1.10 | ×1.25 | Pioneers |
| 🏅 **Top 25** | 11-25 | ×1.05 | ×1.00 | Early adopters |
| 🌍 **Crowd** | 26+ | ×1.00 | ×1.00 | Regular |

### Calculation

```rust
fn get_exclusivity_bonus(level: u8, stats: &LevelStats) -> ExclusivityBonus {
    let user_count = get_users_at_level(stats, level);
    
    match user_count {
        0 => ExclusivityBonus { chance_mult: 100, vault_mult: 100, rank: 99 },
        1 => ExclusivityBonus { chance_mult: 120, vault_mult: 200, rank: 0 },  // FIRST!
        2..=3 => ExclusivityBonus { chance_mult: 115, vault_mult: 150, rank: 1 },
        4..=10 => ExclusivityBonus { chance_mult: 110, vault_mult: 125, rank: 2 },
        11..=25 => ExclusivityBonus { chance_mult: 105, vault_mult: 100, rank: 3 },
        _ => ExclusivityBonus { chance_mult: 100, vault_mult: 100, rank: 99 },
    }
}

// Apply bonuses
final_chance = base_chance × chance_mult / 100
final_vault_cut = vault_bp × vault_mult / 100
```

### Example: First to Level 30

**Base values:**
- Tier: Legendary
- Base chance: 25% (2,500 bp)
- Vault cut: 8% (800 bp)

**Exclusivity bonus (only player at level 30):**
- Chance mult: ×1.20
- Vault mult: ×2.00

**Final values:**
- Chance: 25% × 1.20 = **30%** (3,000 bp)
- Vault cut: 8% × 2.00 = **16%** (1,600 bp)

**If vault has 1,000 SOL:**
- Payout: **160 SOL** + equivalent DBTC
- Win probability: **30%**
- Expected value: 48 SOL

**PLUS jackpot roll:**
- Chance: 0.20% × 1.20 = **0.24%**
- Potential: 1,000 SOL jackpot
- Expected value: 2.4 SOL

**Total expected value: ~50 SOL for reaching level 30 first!**

---

## Jackpot System

### Trigger Conditions

```rust
// Must meet ALL conditions:
1. Level divisible by 10 (10, 20, 30, 40...)
2. Pass probability check (0.20% base)
3. Vault has sufficient funds (110% of pot)
```

### Fixed Pots (Priority Order)

```rust
const JACKPOT_POTS: [(u64, u64); 5] = [
    (1_000_000_000_000, 1_100_000_000_000),  // 1,000 SOL pot, needs 1,100
    (750_000_000_000, 825_000_000_000),      // 750 SOL pot, needs 825
    (690_000_000_000, 759_000_000_000),      // 690 SOL pot, needs 759
    (510_000_000_000, 561_000_000_000),      // 510 SOL pot, needs 561
    (420_000_000_000, 462_000_000_000),      // 420 SOL pot, needs 462
];
```

### Selection Logic

```rust
fn try_jackpot(combined_vault: u64, roll: u16) -> (u64, bool) {
    // 1. Probability check (20 basis points = 0.20%)
    if roll > 20 {
        return (0, false); // Didn't win
    }
    
    // 2. Try pots in descending order (largest first)
    for (pot, required_vault) in JACKPOT_POTS {
        if combined_vault >= required_vault {
            return (pot, true); // WIN!
        }
    }
    
    // 3. Vault too small
    return (0, false); // Probability hit but insufficient funds
}
```

**Example:**
```
User reaches level 20
Roll: 15 (out of 10,000) ← Jackpot hit! (< 20)

Combined vault: 800 SOL
├─→ Check 1,000 SOL pot: needs 1,100 SOL ❌
├─→ Check 750 SOL pot: needs 825 SOL ❌
├─→ Check 690 SOL pot: needs 759 SOL ✅

Result: User wins 690 SOL!
```

### Probability Over Time

```
If 1,000 players reach level 10:
Expected jackpots: 1,000 × 0.20% = 2 jackpots
Average pot: ~600 SOL
Total cost: ~1,200 SOL

If 100 players reach level 20:
Expected jackpots: 100 × 0.20% = 0.2 jackpots
Approx 1 jackpot every 5 players

If 10 players reach level 30:
Expected jackpots: 10 × 0.20% = 0.02 jackpots
Approx 1 jackpot every 50 players
Very rare but MASSIVE!
```

---

## Dual-Currency Distribution

### Why Both SOL + DBTC?

**Economic Benefits:**
1. **Reduces DBTC sell pressure** (users get SOL, don't need to sell)
2. **Increases DBTC utility** (valuable loot currency)
3. **Balanced rewards** (liquid SOL + growth DBTC)
4. **Value alignment** (DBTC price matters for rewards)

### Value Calculation

```rust
// 1. Determine SOL payout amount
sol_payout = vault_sol_balance × vault_cut_bp / 10,000

// 2. Get current DBTC price (from 8-hour oracle)
dbtc_price = doge_btc_mining.avg_price_8h  // SOL per DBTC (10^9 scale)

// 3. Calculate equivalent DBTC
dbtc_payout = (sol_payout × 10^9) / dbtc_price

// 4. Both are paid out
transfer_sol(user, sol_payout)?;
transfer_dbtc(user, dbtc_payout)?;
```

**Example:**
```
Vault has: 100 SOL
Vault cut: 8% (legendary tier)
DBTC price: 0.001 SOL/DBTC

SOL payout: 100 × 0.08 = 8 SOL
DBTC equivalent: (8 × 10^9) / (0.001 × 10^9) = 8,000 DBTC

User receives:
- 8 SOL (liquid, immediate value)
- 8,000 DBTC (growth potential, staking utility)

Total value: 16 SOL equivalent
```

### Currency Selection Algorithm

**System intelligently picks which vault to use:**

```rust
fn pick_best_currency(
    sol_vault: u64,
    dbtc_vault: u64,
    desired_sol: u64,
    desired_dbtc: u64,
) -> (u64, u64) {
    
    // Strategy 1: Prefer SOL (better UX)
    if sol_vault >= desired_sol && desired_sol <= sol_vault / 10 {
        return (desired_sol, 0); // Pay all in SOL
    }
    
    // Strategy 2: Use DBTC if SOL low
    if dbtc_vault >= desired_dbtc && desired_dbtc <= dbtc_vault / 10 {
        return (0, desired_dbtc); // Pay all in DBTC
    }
    
    // Strategy 3: Split payment (both vaults)
    let safe_sol = min(sol_vault / 10, desired_sol);
    let remaining_value_in_sol = desired_sol - safe_sol;
    let dbtc_for_remainder = sol_to_dbtc(remaining_value_in_sol);
    
    if dbtc_vault >= dbtc_for_remainder {
        return (safe_sol, dbtc_for_remainder);
    }
    
    // Strategy 4: Best effort with vault limits
    let final_sol = min(sol_vault / 10, desired_sol);
    let final_dbtc = min(dbtc_vault / 10, desired_dbtc);
    
    return (final_sol, final_dbtc);
}
```

**Example:**
```
Desired payout: 100 SOL + 100,000 DBTC
SOL vault: 50 SOL (low!)
DBTC vault: 10,000,000 DBTC (high)

Result:
- SOL: 5 SOL (50 × 0.1 = 5 max safe)
- DBTC: 95,000 DBTC (remaining value converted)

User gets partial SOL + more DBTC
Still receives full value!
```

---

## Safety Limits

### Vault Protection

```rust
// Normal loot (non-jackpot)
MIN_PAYOUT: 0.01 SOL (10,000,000 lamports)
MAX_PAYOUT: 100 SOL (100,000,000,000 lamports)
MAX_VAULT_PCT: 10% (never drain >10% in one payout)

// Jackpot (exception)
JACKPOT_BUFFER: 110% (requires 10% safety margin)
Can exceed 10% limit IF vault is large enough
```

### Clamping Function

```rust
fn clamp_payout(vault: u64, desired: u64) -> u64 {
    desired
        .max(10_000_000)         // Floor: 0.01 SOL minimum
        .min(100_000_000_000)    // Ceiling: 100 SOL maximum
        .min(vault / 10)         // Protection: 10% vault max
}

// For jackpots: Skip the 10% check
fn clamp_jackpot(vault: u64, pot: u64, required: u64) -> Option<u64> {
    if vault >= required {
        Some(pot)  // Full pot paid (may exceed 10%)
    } else {
        None  // Not enough funds
    }
}
```

---

## Complete Loot Flow (Phase by Phase)

### Phase 1: RNG Generation

```rust
// Deterministic but unpredictable RNG
let slot = Clock::get()?.slot;
let seed = keccak::hashv(&[
    &slot.to_le_bytes(),
    &user.owner.to_bytes(),
]);

let roll = u16::from_le_bytes([seed.0[0], seed.0[1]]); // 0-65,535
let roll_bp = (roll % 10_000) as u32; // 0-9,999 basis points
```

**Properties:**
- Uses on-chain slot (cannot be predicted ahead of time)
- Hashes with user pubkey (unique per user)
- Modulo to basis points (0.01% precision)
- Different roll each transaction/slot

---

### Phase 2: Tier Determination

```rust
let (base_chance, vault_bp) = match user.level {
    1..=4 => {
        let chance = 300 + (20 * user.level as u32);  // 3.2%, 3.4%, 3.6%, 3.8%
        (chance, 100)  // 1% vault
    },
    5 | 10 => (10_000, 50),  // Guaranteed, 0.5% vault
    6..=14 => {
        let chance = 300 + (20 * user.level as u32);  // 3.8% to 5.8%
        (chance, 100)  // 1% vault
    },
    15 | 20 => (10_000, 200),  // Guaranteed, 2% vault
    15..=24 => {
        if level % 5 == 0 {
            (10_000, 200)  // Milestone
        } else {
            (1_500, 500)  // Rare: 15%, 5% vault
        }
    },
    _ => {  // 25+
        if level % 5 == 0 {
            (10_000, 800)  // Legendary milestone: 100%, 8% vault
        } else {
            (2_500, 800)  // Legendary: 25%, 8% vault
        }
    }
};
```

---

### Phase 3: Exclusivity Multipliers

```rust
// Get user count at this level
let user_count = get_users_at_level(level_stats, user.level);

// Determine bonus bracket
let bonus = match user_count {
    1 => ExclusivityBonus {
        chance_mult: 120,  // +20% to chance
        vault_mult: 200,   // 2x vault cut
        rank: 0,
    },
    2..=3 => ExclusivityBonus {
        chance_mult: 115,
        vault_mult: 150,
        rank: 1,
    },
    4..=10 => ExclusivityBonus {
        chance_mult: 110,
        vault_mult: 125,
        rank: 2,
    },
    11..=25 => ExclusivityBonus {
        chance_mult: 105,
        vault_mult: 100,
        rank: 3,
    },
    _ => ExclusivityBonus {
        chance_mult: 100,
        vault_mult: 100,
        rank: 99,
    },
};

// Apply multipliers
final_chance_bp = (base_chance × bonus.chance_mult) / 100
final_vault_bp = (vault_bp × bonus.vault_mult) / 100
```

**Example:**
```
User reaches level 27 (legendary tier)
User count at level 27: 3 users (top 3!)

Base values:
- Chance: 25% (2,500 bp)
- Vault: 8% (800 bp)

Exclusivity (top 3):
- Chance mult: ×1.15
- Vault mult: ×1.50

Final values:
- Chance: 2,500 × 1.15 / 100 = 2,875 bp (28.75%)
- Vault: 800 × 1.50 / 100 = 1,200 bp (12%)

If vault has 500 SOL:
Payout: 500 × 0.12 = 60 SOL + DBTC
Chance: 28.75%
```

---

### Phase 4: Probability Check

```rust
// Roll the dice
if roll_bp >= final_chance_bp {
    // LOST
    msg!("❌ Loot roll failed: {} >= {}", roll_bp, final_chance_bp);
    return Ok((0, 0));
}

// WON!
msg!("✨ Loot roll succeeded: {} < {}", roll_bp, final_chance_bp);
```

**Example:**
```
final_chance_bp = 2,875 (28.75%)
roll_bp = 1,234

Check: 1,234 < 2,875? YES!
Result: User wins loot
```

---

### Phase 5: Jackpot Check (Milestone Levels Only)

```rust
if user.level % 10 == 0 {  // Levels 10, 20, 30...
    // Calculate combined vault (SOL + DBTC in SOL terms)
    let dbtc_price = doge_btc_mining.avg_price_8h;
    
    // Use u128 to prevent overflow
    let dbtc_sol_equivalent = ((loot.total_dbtc_accumulated as u128)
        .saturating_mul(dbtc_price as u128)
        .saturating_div(1_000_000_000u128)) as u64;
    
    let combined_vault = loot.total_sol_accumulated + dbtc_sol_equivalent;
    
    // Try jackpot
    let (jackpot_amount, hit) = try_jackpot(combined_vault, roll_bp as u16);
    
    if hit {
        desired_sol_payout = jackpot_amount;
        jackpot = true;
        msg!("🎊 JACKPOT! Amount: {} SOL", jackpot_amount);
    }
}
```

**Jackpot Probabilities:**
```
Base: 0.20% (20 in 10,000)
With exclusivity (first): 0.24% (24 in 10,000)
With exclusivity (top 3): 0.23% (23 in 10,000)

Expected frequency:
- 1,000 attempts: ~2 jackpots
- 100 attempts: ~0.2 jackpots
- 10 attempts: ~0.02 jackpots
```

---

### Phase 6: Normal Payout Calculation

```rust
// If not jackpot, calculate normal payout
if !jackpot {
    // Calculate desired SOL amount
    desired_sol_payout = (loot.total_sol_accumulated × final_vault_bp) / 10_000;
    
    // Apply safety clamps
    desired_sol_payout = clamp_payout(
        loot.total_sol_accumulated,
        desired_sol_payout
    );
    
    // Calculate equivalent DBTC
    let dbtc_price = get_avg_price_in_sol(doge_btc_mining)?;
    
    desired_dbtc = ((desired_sol_payout as u128)
        .saturating_mul(1_000_000_000u128)
        .saturating_div(dbtc_price as u128)) as u64;
}
```

---

### Phase 7: Currency Selection & Transfer

```rust
// Pick best currency mix
let (sol_payout, dbtc_payout) = pick_best_available(
    loot.total_sol_accumulated,
    loot.total_dbtc_accumulated,
    desired_sol_payout,
    desired_dbtc,
);

// Transfer SOL (if any)
if sol_payout > 0 {
    transfer_loot_sol_to_user(
        loot_sol_vault,
        user,
        system_program,
        sol_payout,
        sol_vault_bump,
    )?;
    
    loot.total_sol_distributed += sol_payout;
    loot.total_sol_accumulated -= sol_payout;
}

// Transfer DBTC (if any)
if dbtc_payout > 0 {
    transfer_loot_dbtc_to_user(
        token_program,
        loot_dbtc_vault,
        user_token_account,
        loot_dbtc_vault_authority,
        token_mint,
        dbtc_payout,
        dbtc_vault_authority_bump,
    )?;
    
    loot.total_dbtc_distributed += dbtc_payout;
    loot.total_dbtc_accumulated -= dbtc_payout;
}
```

---

### Phase 8: Event Emission

```rust
emit!(LootWon {
    owner: user.owner,
    level: user.level,
    sol_amount: sol_payout,
    dbtc_amount: dbtc_payout,
    loot_tier: tier_name, // "minor", "rare", "legendary", "jackpot"
    exclusivity_rank: bonus.rank,
    chance_percentage: final_chance_bp,
    jackpot: jackpot,
});
```

---

## Complete Example: Level 20 Achievement

**User reaches level 20 (rare milestone + jackpot chance)**

### Step 1: Tier Determination
```
Level 20 = Rare Milestone
Base chance: 100% (guaranteed)
Base vault: 2% (200 bp)
```

### Step 2: Exclusivity Check
```
Users at level 20: 5 users (top 10)
Exclusivity bonus:
- Chance mult: ×1.10
- Vault mult: ×1.25
```

### Step 3: Final Values
```
Chance: 100% × 1.10 = 100% (still guaranteed)
Vault: 2% × 1.25 = 2.5% (250 bp)
```

### Step 4: Milestone Payout
```
SOL vault: 200 SOL
SOL payout: 200 × 0.025 = 5 SOL

DBTC price: 0.002 SOL/DBTC
DBTC payout: (5 × 10^9) / (0.002 × 10^9) = 2,500 DBTC

User receives:
✅ 5 SOL
✅ 2,500 DBTC
```

### Step 5: Jackpot Roll
```
Roll: 3,456 (out of 10,000)
Jackpot threshold: 20 (with ×1.10 = 22)

3,456 >= 22? YES
Jackpot MISSED

User keeps milestone rewards (5 SOL + 2,500 DBTC)
```

### Step 6: Vault Updates
```
Loot SOL vault: 200 → 195 SOL
Loot DBTC vault: 1,000,000 → 997,500 DBTC
total_sol_distributed: += 5 SOL
total_dbtc_distributed: += 2,500 DBTC
```

---

## Loot Vault Accumulation

### Sources

```rust
// 1. Mining Rewards (DBTC)
fn claim_dogebtc_tokens() {
    let mined_amount = user.claimable_dbtc;
    let user_receives = mined_amount × 0.9;  // 90%
    let loot_receives = mined_amount × 0.1;   // 10%
    
    transfer(user, user_receives)?;
    transfer(loot_dbtc_vault, loot_receives)?;
    
    loot.total_dbtc_accumulated += loot_receives;
}

// 2. SOL Fees (from treasury)
fn process_referral_payment() {
    let treasury_amount = cost × 0.85;  // After 15% referral
    let loot_amount = treasury_amount × 0.1;  // 10% of treasury
    let treasury_keeps = treasury_amount × 0.9;
    
    transfer(loot_sol_vault, loot_amount)?;
    
    loot.total_sol_accumulated += loot_amount;
}
```

### Accumulation Rates (Estimates)

**Assumptions:**
- 1,000 active users
- Average 10,000 DBTC mined per user per day
- Average 0.1 SOL spent per user per day

**Daily Accumulation:**
```
DBTC: 1,000 users × 10,000 DBTC × 0.1 = 1,000,000 DBTC/day

SOL: 1,000 users × 0.1 SOL × 0.85 × 0.1 = 8.5 SOL/day
```

**Monthly Accumulation:**
```
DBTC: 30,000,000 DBTC/month
SOL: 255 SOL/month
```

**At DBTC price of 0.001 SOL:**
```
Combined value: 255 + (30M × 0.001) = 255 + 30 = 285 SOL/month
```

---

## Milestone vs Probability Rewards

### Milestones (Guaranteed)

```
Every 5 levels starting at 5:
Level 5:  100% chance, 0.5% vault
Level 10: 100% chance, 0.5% vault + jackpot roll
Level 15: 100% chance, 2% vault
Level 20: 100% chance, 2% vault + jackpot roll
Level 25: 100% chance, 4% vault
Level 30: 100% chance, 4% vault + jackpot roll
... continues every 5 levels
```

**Psychology:**
- Provides **guaranteed progression** feel
- Prevents frustration from bad RNG
- "At least I'll get SOMETHING every 5 levels"
- Creates predictable dopamine spikes

### Probability Rewards (RNG)

```
All non-milestone levels:
Levels 1-4:   3.2-3.8% chance
Levels 6-14:  3.8-5.8% chance
Levels 16-24: 15% chance (rare tier)
Levels 26+:   25% chance (legendary tier)
```

**Psychology:**
- **"Maybe this time!"** gambling excitement
- Variable rewards create stronger addiction
- Big wins feel earned and special
- Loss doesn't hurt because milestones exist

---

## Loot Tier Names

```rust
fn get_loot_tier_name(level: u8) -> String {
    match level {
        1..=4 => "Minor".to_string(),
        5 | 10 if !jackpot => "Milestone-5".to_string(),
        6..=14 => "Minor".to_string(),
        15 | 20 if !jackpot => "Rare Milestone".to_string(),
        15..=24 if level % 5 == 0 => "Rare Milestone".to_string(),
        15..=24 => "Rare".to_string(),
        _ if level % 5 == 0 => "Legendary Milestone".to_string(),
        _ => "Legendary".to_string(),
    }
}

// If jackpot hit: "JACKPOT"
```

---

## Economic Analysis

### Expected Value by Level

| Level | Tier | Chance | Vault Cut | Expected SOL (per 100 SOL vault) |
|-------|------|--------|-----------|----------------------------------|
| 3 | Minor | 3.6% | 1% | 0.036 SOL |
| 5 | Milestone | 100% | 0.5% | 0.5 SOL |
| 10 | Milestone | 100% | 0.5% | 0.5 SOL + 0.002 jackpot EV |
| 15 | Rare MS | 100% | 2% | 2 SOL |
| 18 | Rare | 15% | 5% | 0.75 SOL |
| 20 | Rare MS | 100% | 2% | 2 SOL + 0.002 jackpot EV |
| 25 | Legendary | 25% | 8% | 2 SOL |
| 30 | Leg MS | 100% | 4% | 4 SOL + 0.002 jackpot EV |
| 35 | Leg MS | 100% | 4% | 4 SOL |
| 40 | Leg MS | 100% | 4% | 4 SOL + 0.002 jackpot EV |

**Total EV from 0 to 30:**
```
Minor wins (1-14): ~1.5 SOL
Milestones (5, 10, 15, 20, 25, 30): ~13 SOL
Rare wins (15-24): ~7 SOL
Legendary wins (25-30): ~5 SOL
Jackpots (10, 20, 30): ~0.006 SOL (extremely rare)

Total expected: ~26.5 SOL
(Assumes 100 SOL vault average, first player bonuses)
```

---

## Sustainability Analysis

### Healthy Vault State

**Indicators:**
```
✅ Accumulation > Distribution (vault growing)
✅ Vault balance > 100 SOL (supports regular wins)
✅ Vault balance > 1,000 SOL (supports all jackpots)
✅ No depletions in last 30 days
```

**Warning Signs:**
```
⚠️ Accumulation < Distribution (vault shrinking)
⚠️ Vault balance < 50 SOL (risk of dry runs)
⚠️ Multiple "insufficient funds" events
⚠️ Jackpots unavailable despite milestone hits
```

### Rebalancing Strategies

**If vault depleting:**

1. **Reduce vault cuts** (quick fix)
   ```rust
   vault_bp = vault_bp × 50 / 100  // 50% reduction
   ```

2. **Increase accumulation** (long-term fix)
   ```rust
   LOOT_REWARDS_PERCENTAGE = 20  // From 10%
   ```

3. **Add manual funding** (emergency)
   ```rust
   // Admin sends SOL directly to loot vault
   transfer(admin, loot_sol_vault, 1000 × 10^9)?;
   ```

4. **Implement dynamic scaling**
   ```rust
   fn vault_health_multiplier(vault: u64, target: u64) -> u64 {
       min(100, (vault × 100) / target)
   }
   
   // If vault at 50% of target, reduce payouts to 50%
   ```

---

## Frontend Integration

### Query Loot Availability

```typescript
// Check if user will get loot on next level
const userMoonbase = await program.account.userMoonBaseInstance.fetch(pda);
const nextLevel = userMoonbase.level + 1;

// Determine tier
let tier, baseChance, vaultCut;
if (nextLevel <= 4) {
  tier = "Minor";
  baseChance = 3 + (0.2 * nextLevel);
  vaultCut = 1;
} else if (nextLevel === 5 || nextLevel === 10) {
  tier = "Milestone";
  baseChance = 100;
  vaultCut = 0.5;
} 
// ... etc

// Fetch level stats for exclusivity
const levelStats = await program.account.levelStats.fetch(levelStatsPda);
const usersAtLevel = levelStats.trackedLevels.find(e => e.level === nextLevel)?.userCount || 0;

// Calculate bonus
let chanceMultiplier, vaultMultiplier;
if (usersAtLevel === 0) {
  chanceMultiplier = 1.20;
  vaultMultiplier = 2.00;
} else if (usersAtLevel <= 3) {
  chanceMultiplier = 1.15;
  vaultMultiplier = 1.50;
}
// ... etc

// Final values
const finalChance = baseChance * chanceMultiplier;
const finalVaultCut = vaultCut * vaultMultiplier;

// Show in UI
console.log(`Next level (${nextLevel}): ${finalChance}% chance for ${finalVaultCut}% of vault`);
```

### Display Loot History

```typescript
// Listen for LootWon events
connection.onLogs(programId, (logs) => {
  const events = parseEventsFromLogs(logs);
  
  events.forEach(event => {
    if (event.name === "LootWon") {
      showNotification({
        title: event.jackpot ? "🎊 JACKPOT!" : "✨ Loot Won!",
        message: `${event.solAmount / 10**9} SOL + ${event.dbtcAmount} DBTC`,
        level: event.level,
        tier: event.lootTier,
      });
    }
  });
});
```

---

## Testing Scenarios

### Test 1: Minor Loot (Level 3)
```
Expected:
- ~3.6% chance
- ~1% vault cut
- If vault = 100 SOL: ~1 SOL payout
- Run 100 times: ~3-4 wins
```

### Test 2: Milestone (Level 5)
```
Expected:
- 100% chance (always wins)
- 0.5% vault cut
- If vault = 100 SOL: 0.5 SOL payout
- Run 100 times: 100 wins
```

### Test 3: First to Level 25
```
Expected:
- 25% × 1.20 = 30% chance
- 8% × 2.00 = 16% vault cut
- If vault = 500 SOL: 80 SOL payout
- Run 100 times: ~30 wins, avg 24 SOL/win
```

### Test 4: Jackpot at Level 30
```
Expected:
- Milestone: 100% for 4% vault
- Jackpot: 0.24% chance
- Run 1,000 times: ~2-3 jackpots
- Avg jackpot: ~650 SOL
```

---

## Summary

### Key Features
✅ Dual-currency rewards (SOL + DBTC)  
✅ Tier-based probabilities (minor/rare/legendary)  
✅ Guaranteed milestones (every 5 levels)  
✅ Jackpot wheels (levels 10, 20, 30...)  
✅ Exclusivity bonuses (first player = 2x rewards)  
✅ Safety limits (10% vault protection)  
✅ Automatic accumulation (10% of all activity)  

### Economic Health
⚠️ **Requires parameter tuning before launch**  
- Reduce vault cuts by 10x OR increase accumulation to 20%  
- Pre-seed vaults with 2,000-5,000 SOL  
- Monitor accumulation vs distribution ratio  

### Player Experience
🎮 **Highly engaging gambling-style rewards**  
- Variable dopamine (maybe this time!)  
- Guaranteed progression (milestones)  
- Big win moments (jackpots)  
- Racing dynamics (exclusivity)  

---

**The loot system is technically sound and highly engaging. With proper economic parameters, it can drive massive user retention and excitement!**


