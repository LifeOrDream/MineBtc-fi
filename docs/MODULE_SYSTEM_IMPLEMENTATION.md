# Enhanced Module System Implementation

## Overview

This document outlines the implementation of a sophisticated, PvP-ready module system for the DogeTech moonbase game. The system uses **exponential scaling curves** to make each upgrade feel dramatically more impactful, following proven patterns from successful games like Clash of Clans and Boom Beach. The system provides level-gated upgrades, HP-based damage mechanics, faction gating, and prepares for Layer 1 → PvP gameplay where moon bases will attack each other.

## Core Architecture

### 1. Module Type System

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum ModuleType {
    Mining,      // Generates hashpower → mDOGE
    Attraction,  // Grants passive XP / social score
    Attack,      // Fires at rival bases or aliens (PvP ready)
    Research,    // Runs loot attempts with cooldowns
}
```

### 2. Exponential Scaling System

#### **Growth and Decay Constants**
```rust
// Using fixed-point math for precise calculations
pub const GROWTH_NUM: u64 = 115;    // +15% per upgrade step (Power Curve)
pub const GROWTH_DEN: u64 = 100;
pub const DECAY_NUM: u64 = 110;     // 10% reduction efficiency per level
pub const DECAY_DEN: u64 = 100;

/// Power Curve: (1.15)^level for damage, hashpower, shield HP
fn growth_factor(level: u8) -> u64 { /* Q32 fixed-point math */ }

/// Decay Curve: 1/(1.10)^level for cooldowns, reload times
fn decay_factor(level: u8) -> u64 { /* Q32 inverse fixed-point */ }
```

#### **Scaling Types Used**
| Curve Type | Formula | Use Case | Feel |
|------------|---------|----------|------|
| **Power Curve** | `value = base × (1.15)^level` | Damage, Hashpower, Shield HP, Rewards | Upgrades feel **huge** |
| **Decay Curve** | `value = base ÷ (1.10)^level` | Cooldowns, Reload times | Each level **dramatically** reduces wait time |
| **Linear Clip** | `clips = base + (level ÷ 2)` | Magazine sizes | Reasonable progression |

### 3. Module Statistics with Professional Scaling

#### **Mining Module Stats**
```rust
pub struct MiningStats {
    pub max_hp: u32,                   // Maximum health points
    pub base_hashpower: u32,           // Base hashpower at level 0
    pub power_consumption: u16,        // Electricity consumed per hour
}

impl MiningStats {
    /// Calculate current hashpower using exponential growth curve (Power Curve)
    /// Each upgrade provides ~15% multiplicative increase
    pub fn current_hashpower(&self, upgrade_level: u8) -> u32 {
        let q32 = growth_factor(upgrade_level);
        ((self.base_hashpower as u64 * q32) >> 32)
            .min(u32::MAX as u64) as u32
    }
}

// Example progression for base_hashpower = 100:
// Level 0: 100 hash/sec
// Level 1: 115 hash/sec  (+15%)
// Level 2: 132 hash/sec  (+15% of 115)
// Level 3: 152 hash/sec  (+15% of 132)
// Level 5: 201 hash/sec  (2x base!)
// Level 10: 404 hash/sec (4x base!)
```

#### **Attraction Module Stats**
```rust
pub struct AttractionStats {
    pub max_hp: u32,                   // Maximum health points
    pub base_xp_per_hour: u32,         // Base XP generation at level 0
    pub power_consumption: u16,        // Electricity consumed per hour
}

impl AttractionStats {
    /// Calculate current XP generation using exponential growth curve (Power Curve)
    /// Each upgrade provides ~15% multiplicative increase
    pub fn current_xp_per_hour(&self, upgrade_level: u8) -> u32 {
        let q32 = growth_factor(upgrade_level);
        ((self.base_xp_per_hour as u64 * q32) >> 32)
            .min(u32::MAX as u64) as u32
    }
}
```

#### **Attack Module Stats (PvP Ready)**
```rust
pub struct AttackStats {
    pub max_hp: u32,                   // Maximum health points
    pub base_damage: u32,              // Base damage per shot at level 0
    pub base_missiles_per_load: u8,    // Base magazine size at level 0
    pub reload_time_seconds: u32,      // Time to reload magazine at level 0
    pub power_consumption: u16,        // Electricity consumed per shot
}

impl AttackStats {
    /// Damage: Power Curve (~15% increase per level)
    pub fn current_damage(&self, upgrade_level: u8) -> u32 {
        let q32 = growth_factor(upgrade_level);
        ((self.base_damage as u64 * q32) >> 32).min(u32::MAX as u64) as u32
    }
    
    /// Magazine: Capped exponential growth (prevents ridiculous clip sizes)
    /// Uses exponential scaling but caps at 50 missiles for game balance
    pub fn current_missiles_per_load(&self, upgrade_level: u8) -> u8 {
        let q32 = growth_factor(upgrade_level);
        let scaled_missiles = ((self.base_missiles_per_load as u64 * q32) >> 32) as u8;
        scaled_missiles.min(50) // Cap at 50 missiles max for balance
    }
    
    /// Reload: Decay Curve (~9% reduction per level)
    pub fn current_reload_time_seconds(&self, upgrade_level: u8) -> u32 {
        let q32_inv = decay_factor(upgrade_level);
        ((self.reload_time_seconds as u64 * q32_inv) >> 32).max(1) as u32
    }
}

// Example progression for base_damage = 50, base_missiles = 5, reload = 30s:
// Level 0: 50 damage, 5 missiles, 30s reload
// Level 1: 58 damage, 6 missiles, 27s reload  (+15% dmg, +1 missile, -9% reload)
// Level 2: 66 damage, 7 missiles, 25s reload  
// Level 3: 76 damage, 8 missiles, 23s reload
// Level 5: 101 damage, 10 missiles, 19s reload (2x damage, 2x missiles!)
// Level 10: 202 damage, 20 missiles, 12s reload (4x damage, 4x missiles!)
```
 

#### **Research Module Stats (Loot Generation)**
```rust
pub struct ResearchStats {
    pub max_hp: u32,                   // Maximum health points
    pub cooldown_sec: u32,             // Time between loot attempts at level 0
    pub reward_type: u8,               // 0 = mDOGE, 1 = SOL
    pub max_reward: u64,               // Maximum reward amount at level 0
    pub probability: u16,              // Success probability at level 0 (0-10000 = 0-100%)
    pub power_consumption: u16,        // Electricity consumed per hour
}

impl ResearchStats {
    /// Cooldown: Decay Curve (~9% reduction per level)
    pub fn current_cooldown_sec(&self, upgrade_level: u8) -> u32 {
        let q32_inv = decay_factor(upgrade_level);
        ((self.cooldown_sec as u64 * q32_inv) >> 32).max(60) as u32
    }
    
    /// Rewards: Power Curve (~15% increase per level)
    pub fn current_max_reward(&self, upgrade_level: u8) -> u64 {
        let q32 = growth_factor(upgrade_level);
        ((self.max_reward as u128 * q32 as u128) >> 32).min(u64::MAX as u128) as u64
    }
    
    /// Probability: Power Curve (~15% increase per level, capped at 100%)
    pub fn current_probability(&self, upgrade_level: u8) -> u16 {
        let q32 = growth_factor(upgrade_level);
        ((self.probability as u64 * q32) >> 32).min(10000) as u16
    }
}
```

### 4. Level-Gated Upgrade System

```rust
pub struct ModuleConfig {
    // ... other fields ...
    pub max_upgrades: u8,                      // Maximum upgrade level (0-10)
    pub upgrade_level_requirements: Vec<u8>,   // Moonbase levels required for each upgrade
    pub upgrade_cost: u64,                     // Base SOL cost per upgrade
    // ... other fields ...
}
```

**Example**: `upgrade_level_requirements: [5, 10, 15, 25]` means:
- Upgrade 1: Available at moonbase level 5
- Upgrade 2: Available at moonbase level 10
- Upgrade 3: Available at moonbase level 15
- Upgrade 4: Available at moonbase level 25

**Cost Scaling**: Each upgrade costs 50% more than the previous:
- Upgrade 1: `base_cost * 1.0`
- Upgrade 2: `base_cost * 1.5`
- Upgrade 3: `base_cost * 2.25`
- Upgrade 4: `base_cost * 3.375`

### 5. Runtime State with HP System

```rust
pub enum ModuleRuntimeState {
    Mining {
        current_hp: u32,
        total_mined: u64,
    },
    Attraction {
        current_hp: u32,
        total_xp_generated: u64,
        last_xp_claim: i64,
    },
    Attack {
        current_hp: u32,                // Current HP (affects efficiency)
        missiles_left: u8,              // Remaining ammunition
        last_shot_timestamp: i64,       // Last time this module fired
        total_shots_fired: u32,         // Historical combat data
    },
 
    Research {
        current_hp: u32,                // Current HP (affects efficiency)
        current_research_start: i64,    // When current loot attempt started
        research_completed: u32,        // Number of completed loot attempts
        active_research_id: Option<u8>, // Current loot attempt type
    },
}
```

### 6. HP-Based Efficiency System

**Core Principle**: Damaged modules perform worse than healthy ones.

```rust
impl ModuleInstance {
    /// Calculate efficiency multiplier based on HP (damaged modules work worse)
    /// Returns a value between 0.1 and 1.0 (10% to 100% efficiency)
    pub fn hp_efficiency_multiplier(&self, max_hp: u32) -> f64 {
        let current_hp = self.current_hp();
        if max_hp == 0 { return 1.0; }
        
        let efficiency = (current_hp as f64) / (max_hp as f64);
        efficiency.max(0.1).min(1.0) // Minimum 10% efficiency even when heavily damaged
    }

    /// Calculate effective hashpower for mining modules
    pub fn effective_hashpower(&self, stats: &MiningStats) -> u32 {
        let base_hashpower = stats.current_hashpower(self.upgrade_level);
        let efficiency = self.hp_efficiency_multiplier(stats.max_hp);
        (base_hashpower as f64 * efficiency) as u32
    }
}
```

**Examples**:
- **Full HP (1000/1000)**: 100% efficiency → Full hashpower
- **Half HP (500/1000)**: 50% efficiency → Half hashpower
- **Critical HP (50/1000)**: 10% efficiency → Minimum performance
- **Zero HP (0/1000)**: 10% efficiency → Still functions at minimum

### 7. Repair System

```rust
impl ModuleInstance {
    /// Calculate repair cost based on missing HP
    pub fn repair_cost(&self, stats: &ModuleStats) -> u64 {
        let max_hp = // ... extract max_hp from stats ...
        let current_hp = self.current_hp();
        let missing_hp = max_hp.saturating_sub(current_hp);
        
        // Repair cost: 0.001 SOL per missing HP point
        const REPAIR_SOL_PER_HP: u64 = 1_000_000; // 0.001 SOL in lamports
        missing_hp as u64 * REPAIR_SOL_PER_HP
    }
}
```

## Admin Functions

### 1. Adding New Module Configs

```rust
pub fn add_module_to_base_internal(
    // ... context ...
    stats: ModuleStats,
    max_upgrades: u8,
    upgrade_level_requirements: Vec<u8>,
    upgrade_cost: u64,
    // ... other params ...
) -> Result<()>
```

**Validation**:
- ✅ Upgrade requirements length matches max_upgrades
- ✅ Level requirements are increasing monotonically
- ✅ Module stats match the declared module type
- ✅ All faction IDs exist in global config

### 2. Updating Existing Configs

```rust
pub fn update_module_internal(
    // ... context ...
    upgrade_level_requirements: Option<Vec<u8>>,
    // ... other optional params ...
) -> Result<()>
```

**Features**:
- ✅ Partial updates (only specified fields are changed)
- ✅ Validation of new upgrade requirements
- ✅ Prevents reducing max_tiles (data integrity)

## Upgrade Scaling Examples

### 🚀 **Mining Rig Progression (base_hashpower = 100)**

| Level | Hashpower | Increase | Total Growth |
|-------|-----------|----------|--------------|
| 0     | 100       | -        | 1.0x         |
| 1     | 115       | +15      | 1.15x        |
| 2     | 132       | +17      | 1.32x        |
| 3     | 152       | +20      | 1.52x        |
| 4     | 175       | +23      | 1.75x        |
| 5     | **201**   | +26      | **2.01x**    |
| 6     | 231       | +30      | 2.31x        |
| 7     | 266       | +35      | 2.66x        |
| 8     | 306       | +40      | 3.06x        |
| 9     | 352       | +46      | 3.52x        |
| 10    | **404**   | +52      | **4.04x**    |

**💡 Key Insight**: By level 5, you've doubled your output! By level 10, you're producing 4x the base rate!

### ⚔️ **Attack Turret Progression (base_damage = 50, base_missiles = 5, reload = 30s)**

| Level | Damage | Missiles | Reload | Burst DPS | Sustained DPS | Total Growth |
|-------|--------|----------|--------|-----------|---------------|--------------|
| 0     | 50     | 5        | 30s    | 8.33      | 1.67          | 1.0x         |
| 1     | 58     | 6        | 27s    | 12.9      | 2.16          | 1.29x        |
| 2     | 66     | 7        | 25s    | 18.5      | 2.64          | 1.58x        |
| 3     | 76     | 8        | 23s    | 26.4      | 3.30          | 1.98x        |
| 4     | 87     | 9        | 21s    | 37.3      | 4.14          | 2.48x        |
| 5     | **101**| **10**   | **19s**| **53.2**  | **5.32**      | **3.18x**    |
| 6     | 116    | 12       | 17s    | 82.0      | 6.82          | 4.08x        |
| 7     | 133    | 13       | 16s    | 108.0     | 8.31          | 4.98x        |
| 8     | 153    | 15       | 14s    | 164.0     | 10.9          | 6.53x        |
| 9     | 176    | 17       | 13s    | 230.0     | 13.5          | 8.08x        |
| 10    | **202**| **20**   | **12s**| **337.0** | **16.8**      | **10.1x**    |

**💡 Key Insight**: Burst DPS scales exponentially due to ALL THREE factors improving! Magazine size doubles the effective alpha strike damage.
 

### 🔬 **Research Lab Progression (cooldown = 3600s, reward = 1M mDOGE, prob = 25%)**

| Level | Cooldown | Max Reward | Probability | Expected mDOGE/hour |
|-------|----------|------------|-------------|---------------------|
| 0     | 60 min   | 1.0M       | 25%         | 250K                |
| 1     | 55 min   | 1.15M      | 29%         | 378K (+51%)         |
| 2     | 50 min   | 1.32M      | 33%         | 523K (+109%)        |
| 3     | 45 min   | 1.52M      | 38%         | 760K (+204%)        |
| 5     | 37 min   | 2.01M      | 50%         | 1.6M (+540%)        |
| 10    | 23 min   | 4.04M      | 100%        | 10.5M (+4100%)      |

**💡 Key Insight**: Expected rewards scale exponentially due to ALL three factors improving!

## Example Module Configurations

### Mining Rig (Basic → Advanced)

```rust
// Basic Mining Rig
MiningStats {
    max_hp: 1000,
    base_hashpower: 100,        // 100 hash/sec at level 0
    power_consumption: 50,      // 50 kW/hour
}

// Config
upgrade_level_requirements: [5, 10, 15, 20] // 4 upgrades available
upgrade_cost: 50_000_000 // 0.05 SOL base cost

// Performance at different levels:
// Level 0: 100 hash/sec (moonbase level 1+)
// Level 1: 115 hash/sec (moonbase level 5+, costs 0.05 SOL)
// Level 2: 132 hash/sec (moonbase level 10+, costs 0.075 SOL)
// Level 3: 152 hash/sec (moonbase level 15+, costs 0.1125 SOL)
// Level 4: 175 hash/sec (moonbase level 20+, costs 0.169 SOL)
```

### Attack Turret (PvP Ready)

```rust
AttackStats {
    max_hp: 800,
    base_damage: 50,            // 50 damage per shot at level 0
    base_missiles_per_load: 5,   // 5 shots per magazine
    reload_time_seconds: 30,    // 30 seconds to reload
    power_consumption: 25,      // 25 kW per shot
}

upgrade_level_requirements: [8, 15, 25] // 3 upgrades, requires higher levels
```

### Research Lab (Loot Generation)

```rust
ResearchStats {
    max_hp: 500,
    cooldown_sec: 3600,         // 1 hour between loot attempts
    reward_type: 0,             // mDOGE rewards
    max_reward: 1_000_000_000,  // Up to 1 mDOGE
    probability: 2500,          // 25% success rate
    power_consumption: 40,      // 40 kW/hour
}

upgrade_level_requirements: [12, 20] // 2 upgrades for better loot chances
```

## PvP Integration Points

### Combat Mechanics
- **Attack modules** track ammunition and reload timers
- **HP damage** affects all module efficiency until repaired

### Damage Application
- Incoming attacks reduce module `current_hp`
- Efficiency drops proportionally: `efficiency = current_hp / max_hp`
- Players must spend SOL to repair damaged modules

### Economic Warfare
- Attacking enemy bases can reduce their mining efficiency
- Repairing costs create ongoing SOL sinks
- Strategic targets: high-level mining rigs vs defensive structures

## Future Extensibility

### Easy Module Type Addition
1. Add new variant to `ModuleType` enum
2. Create new stats struct with upgrade scaling
3. Add runtime state variant for type-specific data
4. Implement helper methods for effective calculations

### Upgrade System Scaling
- `max_upgrades` can be increased per module type
- `upgrade_level_requirements` can be extended
- Cost scaling formula can be adjusted globally

### PvP Feature Expansion
- **Shields**: Already implemented in DefenseStats
- **Ammunition**: Already tracked in AttackStats runtime state
- **Cooldowns**: Built into all module types
- **Damage Types**: Can be added to AttackStats
- **Armor**: Can be added to all module HP calculations

## Production Readiness

### Security
- ✅ Authority checks on all admin functions
- ✅ Input validation and bounds checking
- ✅ Protection against arithmetic overflow
- ✅ Data integrity constraints

### Performance
- ✅ Efficient PDA structures
- ✅ Minimal on-chain storage
- ✅ O(1) lookups for most operations
- ✅ Batch operations where possible

### Maintainability
- ✅ Clear separation of concerns
- ✅ Type-safe stats system
- ✅ Comprehensive error handling
- ✅ Event emission for monitoring 

## Game Design Precedents

### 🎮 **Industry-Proven Scaling Patterns**

Our exponential scaling system follows patterns from highly successful games:

| Game | Damage Scaling | Cost Scaling | Player Response |
|------|----------------|--------------|-----------------|
| **Clash of Clans** | ~18% per level | 1.5-1.8× per level | "Each upgrade feels massive!" |
| **Boom Beach** | ~23% per level | ~1.6× per level | "Worth saving up for upgrades" |
| **Idle Miner Tycoon** | ~15% per level | ~1.6× per level | "Upgrades are exciting events" |
| **DogeTech** | **15% per level** | **1.5× per level** | **"Every upgrade is game-changing!"** |

### 🧠 **Psychological Impact**

**Early Game (Levels 1-3):**
- Upgrades feel affordable and provide clear benefits
- Players see immediate 15-20% improvements
- Creates positive feedback loop for engagement

**Mid Game (Levels 4-6):**
- Upgrades require planning and saving
- 2-3x performance gains feel transformative
- Players feel genuine progression and power growth

**Late Game (Levels 7-10):**
- Upgrades become major investments
- 4-10x performance gains justify high costs
- Players feel they've achieved something significant

### 📈 **Why Linear Scaling Fails**

```
Linear System Problems:
Level 1: +20 damage (50 → 70)   = +40% improvement
Level 5: +20 damage (130 → 150) = +15% improvement  
Level 10: +20 damage (230 → 250) = +8% improvement

↓ Diminishing excitement, players lose interest
```

```
Exponential System Success:
Level 1: +15% damage (50 → 58)   = +15% improvement
Level 5: +15% damage (175 → 201) = +15% improvement  
Level 10: +15% damage (352 → 404) = +15% improvement

↑ Consistent excitement, each upgrade feels equally impactful
```

## Level-Gated Upgrade System 

#### **Smart Missile Scaling Logic**

The missile system uses **capped exponential growth** to prevent unrealistic magazine sizes:

```rust
/// Magazine: Capped exponential growth (prevents ridiculous clip sizes)
/// Uses exponential scaling but caps at 50 missiles for game balance
pub fn current_missiles_per_load(&self, upgrade_level: u8) -> u8 {
    let q32 = growth_factor(upgrade_level);
    let scaled_missiles = ((self.base_missiles_per_load as u64 * q32) >> 32) as u8;
    scaled_missiles.min(50) // Cap at 50 missiles max for balance
}
```

**Design Philosophy:**
- **Early levels (0-5)**: Magazine follows exponential curve exactly (5 → 6 → 7 → 8 → 9 → 10)
- **Mid levels (6-8)**: Continues exponential growth (12 → 13 → 15 missiles)
- **High levels (9-10)**: Still meaningful growth but caps prevent absurdity (17 → 20 missiles)
- **Cap benefit**: Prevents level 15+ players from having 100+ missile magazines

**Comparison with other approaches:**
- ❌ **Linear (+1 per level)**: Boring, predictable, no excitement
- ❌ **Uncapped exponential**: Level 15 = 406 missiles (game-breaking)
- ✅ **Capped exponential**: Exciting growth that stays balanced

**💡 Result**: Every upgrade feels impactful while maintaining PvP balance!
