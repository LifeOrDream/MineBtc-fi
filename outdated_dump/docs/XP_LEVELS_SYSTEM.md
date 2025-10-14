# 🌟 DogeTech XP & Levels System

## Overview

The XP (Experience Points) and Levels system in DogeTech is a comprehensive player progression mechanic that rewards various in-game activities and provides milestone-based loot rewards. Players advance through levels by earning XP, unlocking new content, and becoming eligible for rare loot distributions.

## 🎯 Core XP Sources

### Daily Activities
- **Daily Login**: `10 XP` per day
  - Maintains login streaks
  - Encourages consistent engagement
  - Processed via `daily_login_internal()`

### Module Management
- **Installing Modules**: `50 XP` per module
  - Rewards base expansion
  - Encourages facility development
- **Upgrading Modules**: `30 XP` per upgrade
  - Rewards progression investment
  - Scales with upgrade complexity

### NFT Activities
- **Locking Doge NFTs**: `20 XP` per NFT locked
  - Rewards strategic NFT deployment
  - Enhances module efficiency

### Mining Operations
- **Mining Rewards**: `15 XP per 1000 mDOGE` mined
  - Rewards active mining participation
  - Scales with mining performance
  - Calculated via `calculate_mining_xp()`

### Social Features
- **Referrals**: `100 XP per successful referral`
  - Rewards community growth
  - Bonus XP based on referral activity
  - Formula: `(referrals_count × 100) + sqrt(total_sol_earned) / 1000`

## 📈 Level Progression Formula

### Exponential Scaling
```rust
required_xp = 100 + (level² × 20)
```

### Level Examples
| Level | Required XP | Cumulative XP | Activities Needed |
|-------|-------------|---------------|-------------------|
| 1     | 120         | 120           | 12 daily logins   |
| 2     | 180         | 300           | 6 module installs |
| 5     | 600         | 1,500         | 100k mDOGE mined  |
| 10    | 2,100       | 8,500         | 85 referrals      |
| 15    | 4,600       | 24,000        | 480 daily logins  |
| 20    | 8,100       | 44,000        | 880 module upgrades |
| 25    | 12,600      | 78,000        | 156 module installs |

## 🎁 Loot Rewards System

### Level Rarity Classification

#### **Minor Levels** (1-14)
- **Probability Rewards**: 2-10% chance
- **Vault Percentage**: 1% of loot vault
- **Frequency**: Common

#### **Rare Levels** (15-24)
- **Probability Rewards**: 15% base chance
- **Vault Percentage**: 5% of loot vault
- **Milestone Rewards**: Guaranteed for rare achievements

#### **Legendary Levels** (25+)
- **Probability Rewards**: 25% base chance
- **Vault Percentage**: 8% of loot vault
- **Milestone Rewards**: Guaranteed with maximum multipliers

### Rarity Bonuses

#### **Exclusivity Multipliers**
- **First Player at Level**: +20% probability, 2x rewards
- **Top 3 Players**: +15% probability, 1.5x rewards
- **Top 10 Players**: +10% probability, 1.25x rewards
- **Top 25 Players**: +5% probability

#### **Milestone Rewards** (Guaranteed)
- **Every 5 Levels**: Major milestone (3% vault)
- **Level 15+**: Rare milestone (5% vault)
- **Level 25+**: Legendary milestone (8% vault)
- **Unique Achievement**: Being first/only at level

### Dynamic Level Tracking

The system tracks the top 25 highest levels dynamically:
- **Real-time Updates**: Level distribution changes as players progress
- **Rarity Calculation**: Based on current player distribution
- **Automatic Adjustment**: Lower levels drop out as higher levels are achieved

## 🔧 Technical Implementation

### Core Functions

#### XP Management
```rust
// Add XP and attempt level up
pub fn add_xp_and_try_level_up(
    user: &mut UserMoonBaseInstance, 
    xp_amount: u32, 
    xp_source: &str
) -> Result<(u32, bool)>

// Calculate required XP for level
pub fn calculate_required_xp(level: u8) -> u32

// Try to level up if enough XP
pub fn try_level_up(user: &mut UserMoonBaseInstance) -> Result<bool>
```

#### Level Statistics
```rust
// Update level distribution tracking
pub fn update_level_stats(
    level_stats: &mut LevelStats,
    user: &Pubkey,
    old_level: u8,
    new_level: u8,
) -> Result<()>

// Check if level is rare
pub fn is_level_rare(level_stats: &LevelStats, level: u8) -> (bool, u32)

// Get user count at specific level
pub fn get_users_at_level(level_stats: &LevelStats, level: u8) -> u32
```

#### Loot Distribution
```rust
// Calculate milestone rewards
pub fn calculate_milestone_loot_reward(
    level_achieved: u8,
    vault_mdoge_balance: u64,
    vault_sol_balance: u64,
    users_at_level: u32,
) -> (u64, u64, String)

// Calculate probability rewards
pub fn calculate_probability_loot_reward(
    user_level: u8,
    vault_mdoge_balance: u64,
    vault_sol_balance: u64,
    users_at_level: u32,
    random_seed: u64,
) -> (u64, u64, u32, bool)
```

### Data Structures

#### User Progress
```rust
pub struct UserMoonBaseInstance {
    pub level: u8,              // Current level (0-255)
    pub xp: u32,                // Current XP points
    pub last_login_ts: i64,     // Daily login tracking
    pub daily_login_streak: u16, // Login streak counter
    // ... other fields
}
```

#### Level Statistics
```rust
pub struct LevelStats {
    pub tracked_levels: Vec<LevelEntry>, // Top 25 levels
    pub total_users: u32,               // Total players
    pub max_level_achieved: u8,         // Highest level reached
    pub min_tracked_level: u8,          // Lowest tracked level
    pub last_update_timestamp: i64,     // Last update time
}

pub struct LevelEntry {
    pub level: u8,      // Level number
    pub user_count: u32, // Players at this level
}
```

## 📊 Events & Tracking

### XP Events
```rust
#[event]
pub struct XpGained {
    pub owner: Pubkey,
    pub xp_amount: u32,
    pub xp_source: String,
    pub total_xp: u32,
}

#[event]
pub struct LevelUp {
    pub owner: Pubkey,
    pub new_level: u8,
    pub total_xp: u32,
}
```

### Loot Events
```rust
#[event]
pub struct MilestoneLootAwarded {
    pub recipient: Pubkey,
    pub level_achieved: u8,
    pub mdoge_amount: u64,
    pub sol_amount: u64,
    pub milestone_type: String,
    pub users_at_level: u32,
}

#[event]
pub struct ProbabilityLootAwarded {
    pub recipient: Pubkey,
    pub level: u8,
    pub mdoge_amount: u64,
    pub sol_amount: u64,
    pub probability_percentage: u32,
    pub users_at_level: u32,
}
```

## 🎮 Strategic Considerations

### For Players

#### **Early Game (Levels 1-10)**
- Focus on daily logins for consistent XP
- Install basic modules for quick XP gains
- Build mining infrastructure for passive XP

#### **Mid Game (Levels 11-20)**
- Optimize mining operations for mDOGE → XP conversion
- Upgrade modules strategically
- Consider referral program participation

#### **End Game (Levels 21+)**
- Compete for legendary level status
- Maximize rare level achievement bonuses
- Focus on being first to reach new levels

### For Game Economy

#### **XP Inflation Control**
- Exponential level requirements prevent easy progression
- Diminishing returns on repetitive activities
- Social activities (referrals) provide sustainable XP

#### **Loot Distribution Balance**
- Percentage-based rewards scale with vault size
- Rarity bonuses reward early achievers
- Probability system prevents guaranteed farming

## 🔮 Advanced Features

### Daily Login System
- **Streak Tracking**: Consecutive login bonuses
- **Streak Reset**: 48-hour grace period
- **XP Consistency**: Reliable daily progression path

### Referral XP Calculation
```rust
// Base XP from referral count + bonus from SOL earned
let base_xp = (referrals_count as u32) * 100;
let sol_bonus_xp = integer_sqrt(total_sol_earned) / 1000;
total_referral_xp = base_xp + sol_bonus_xp;
```

### Mining XP Integration
- **Automatic Calculation**: XP awarded during token claims
- **Scaling Rewards**: Higher mining = more XP
- **Performance Incentive**: Rewards active miners

## 📈 Future Enhancements

### Planned Features
1. **Seasonal XP Multipliers**: Limited-time XP boosts
2. **Achievement System**: Special XP bonuses for milestones
3. **Faction Bonuses**: XP multipliers based on faction choice
4. **PvP XP**: Combat-based progression rewards
5. **Research XP**: Module-specific research achievements

### Balancing Considerations
- **Level Cap**: Currently unlimited, may add soft caps
- **XP Sources**: May add new activities or adjust existing rates
- **Loot Scaling**: Vault percentages may adjust based on economy
- **Rarity Thresholds**: Level classifications may evolve

---

## 🚀 Getting Started

To interact with the XP system:

1. **Create Moonbase**: Starts at level 0 with 0 XP
2. **Daily Login**: Call `daily_login()` for consistent XP
3. **Build & Upgrade**: Install and upgrade modules for major XP gains
4. **Mine Actively**: Claim mDOGE regularly for mining XP
5. **Level Up**: Automatic when sufficient XP is reached
6. **Loot Eligibility**: Higher levels = better loot chances

The XP/Levels system is designed to reward both casual daily players and hardcore grinders, with the rarest rewards going to the most dedicated pioneers who reach legendary levels first! 🌟 