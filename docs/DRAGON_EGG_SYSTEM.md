# Dragon Egg NFT System

## Overview
The Dragon Egg system integrates Metaplex Core NFTs with the MoonBase hashpower and mining mechanics. Users can mint one Dragon Egg per moonbase, incubate it to gain power based on DBTC tokens mined, and the egg's power grows automatically during token claims.

## Key Features

### 1. **One Egg Per Moonbase Limit**
- Each moonbase can incubate a maximum of 1 Dragon Egg at a time
- Enforced via `IncubationState` account that tracks the currently incubated egg
- Users can remove an egg and incubate a different one if desired

### 2. **Automatic Power Updates**
- Dragon Egg power is **automatically updated** when users claim DBTC tokens
- No need for separate backend calls or manual updates
- Formula: `power_increase = claimed_dbtc_amount / POWER_RATE_MULTIPLIER`
- Power is capped at `MAX_EGG_POWER` (100,000)

### 3. **NFT Integration**
- Dragon Eggs are Metaplex Core NFTs minted during moonbase creation
- Users can choose pricing tier when creating moonbase:
  - **Tier 0**: 0.5 SOL - No NFT minted
  - **Tier 1**: 1.42 SOL - Dragon Egg NFT included
- NFTs have unique DNA attributes and randomized URIs from the global pool

## Program Flow

### Moonbase Creation with Dragon Egg
```
User calls: create_user_moonbase(pricing_tier: 1)
  ├─> Pay 1.42 SOL
  ├─> Create UserMoonBaseInstance account
  ├─> Mint Dragon Egg NFT via Metaplex Core CPI
  ├─> Create DragonEggMetadata PDA
  ├─> Initialize with DNA, power=0, timestamp
  └─> Emit DragonEggMinted event
```

### Incubating Dragon Egg
```
User calls: incubate_dragon_egg()
  ├─> Verify NFT ownership (Metaplex Core asset)
  ├─> Check egg not already incubated
  ├─> Check moonbase has no egg (max 1 per moonbase)
  ├─> Set IncubationState.incubated_egg = Some(egg_pubkey)
  ├─> Set DragonEggMetadata.incubated_moonbase = Some(user_pubkey)
  └─> Update timestamps
```

### Power Growth (Automatic)
```
User calls: claim_dbtc_tokens()
  ├─> Calculate claimable DBTC tokens
  ├─> Transfer tokens to user
  ├─> [IF dragon_egg_metadata provided as optional account]
  │   ├─> Verify egg is incubated in this moonbase
  │   ├─> Calculate power_increase = claimed_amount / POWER_RATE_MULTIPLIER
  │   ├─> Update egg.power (capped at MAX_EGG_POWER)
  │   ├─> Update egg.total_hashpower_accumulated
  │   └─> Update incubation_state.total_power
  ├─> Award mining XP
  └─> Process daily login
```

### Removing Dragon Egg
```
User calls: remove_dragon_egg()
  ├─> Verify NFT ownership
  ├─> Check egg is currently incubated
  ├─> Set IncubationState.incubated_egg = None
  ├─> Set DragonEggMetadata.incubated_moonbase = None
  ├─> Update timestamps
  └─> Egg retains its accumulated power (can be re-incubated later)
```

## Account Structures

### DragonEggMetadata (PDA)
```rust
pub struct DragonEggMetadata {
    pub mint: Pubkey,                      // NFT mint address
    pub owner: Pubkey,                     // Original owner (user)
    pub incubated_moonbase: Option<Pubkey>, // Currently incubated in which moonbase
    pub power: u32,                        // Current power level (0 to MAX_EGG_POWER)
    pub dna: u64,                          // Unique DNA for attributes
    pub total_hashpower_accumulated: u64,  // Total hashpower contributed
    pub minted_at: i64,                    // Timestamp when minted
    pub last_update_ts: i64,               // Last power update timestamp
    pub bump: u8,
}
```
**Seeds**: `[DRAGON_EGG_METADATA_SEED, user.key(), moonbase_count.to_le_bytes()]`

### IncubationState (PDA)
```rust
pub struct IncubationState {
    pub moonbase_owner: Pubkey,           // Owner of the moonbase
    pub incubated_egg: Option<Pubkey>,    // Currently incubated egg (max 1)
    pub total_power: u64,                 // Total power from incubated egg
    pub last_update_ts: i64,              // Last update timestamp
    pub bump: u8,
}
```
**Seeds**: `[INCUBATION_STATE_SEED, user_moonbase.key()]`

## Power Calculation

### Formula
```
power_increase = claimed_dbtc_amount / POWER_RATE_MULTIPLIER
```
Where `POWER_RATE_MULTIPLIER = 1000`

### Example
- User claims **10,000 DBTC tokens**
- Power increase = 10,000 / 1,000 = **10 power**
- If current power is 500, new power = 510 (capped at 100,000)

### Growth Rate
- Higher hashpower → More DBTC tokens mined → Faster power growth
- Power growth is **proportional to mining output**, not just time
- Incentivizes active mining and module upgrades

## Optional Accounts in Claim

The `claim_dbtc_tokens` instruction accepts **optional accounts** for Dragon Egg:
- `dragon_egg_asset` (Metaplex Core asset)
- `dragon_egg_metadata` (DragonEggMetadata PDA)
- `incubation_state` (IncubationState PDA)

**If provided**: Power is automatically updated during claim.
**If not provided**: Claim works normally, no power update.

This allows:
- Users without eggs to claim normally
- Users with eggs to get automatic power updates
- Efficient transaction construction (only pass accounts when needed)

## Error Codes

| Error | Description |
|-------|-------------|
| `EggAlreadyIncubated` | Trying to incubate an egg that's already incubated |
| `MaxEggsReached` | Moonbase already has 1 egg (max limit) |
| `EggNotIncubated` | Trying to remove/update an egg that's not incubated |
| `NftNotOwnedByUser` | NFT ownership verification failed |
| `InvalidMetadata` | Invalid or empty Dragon Egg URI pool |

## Admin Functions

### Adding Dragon Egg URIs
```
Admin calls: add_dragon_egg_uris(uris: Vec<String>)
  ├─> Verify admin authority
  ├─> Validate URI lengths (< MAX_URI_LENGTH)
  ├─> Append URIs to GlobalConfig.dragon_egg_uris
  └─> Random URI selected during NFT minting
```

### Setting Dragon Egg Collection
```
Admin calls: set_dragon_egg_collection(collection_pubkey: Pubkey)
  ├─> Verify admin authority
  ├─> Set GlobalConfig.dragon_egg_collection
  └─> All new eggs will be part of this collection
```

## Constants

```rust
// Dragon Egg Configuration
pub const MAX_EGGS_PER_MOONBASE: u8 = 1;
pub const MAX_EGG_POWER: u32 = 100_000;
pub const POWER_RATE_MULTIPLIER: u64 = 1000;

// Pricing
pub const MOONBASE_EGG_PRICE: u64 = 1_420_000_000; // 1.42 SOL
pub const PRICE_ONE: u64 = 500_000_000;            // 0.5 SOL (no NFT)
pub const PRICE_TWO: u64 = 1_420_000_000;          // 1.42 SOL (with Egg)

// URI Management
pub const MAX_DRAGON_EGG_URIS: usize = 100;
pub const MAX_URI_LENGTH: usize = 200;

// PDA Seeds
pub const DRAGON_EGG_METADATA_SEED: &str = "dragon_egg_metadata";
pub const INCUBATION_STATE_SEED: &str = "incubation_state";
pub const DRAGON_EGG_COLLECTION_SEED: &str = "dragon_egg_collection";
```

## Best Practices

### For Users
1. **Incubate immediately**: Eggs gain power only when incubated and DBTC is claimed
2. **Claim regularly**: Power updates happen during claims, not passively
3. **Upgrade modules**: Higher hashpower → More DBTC → Faster power growth
4. **Monitor power cap**: Power stops growing at MAX_EGG_POWER (100,000)

### For Frontend Integration
1. **Pass optional accounts**: Always include Dragon Egg accounts in `claim_dbtc_tokens` if user has an incubated egg
2. **Check incubation status**: Query `IncubationState` to see if moonbase has an egg
3. **Display power growth**: Show power updates after each claim
4. **NFT verification**: Use Metaplex Core SDK to verify ownership before incubation

### For Backend
- No periodic power updates needed (automatic during claims)
- `update_dragon_egg_power` function is deprecated but kept for compatibility
- Monitor `DragonEggMinted` events for analytics
- Track power distribution across eggs via `DragonEggMetadata.power`

## Future Enhancements

Potential upgrades to the Dragon Egg system:
- **Power Utility**: Use accumulated power for special abilities or bonuses
- **Evolution**: Eggs hatch into Dragons at certain power thresholds
- **Trading**: Transfer eggs between users (with power retention)
- **Breeding**: Combine two eggs to create a new one with hybrid DNA
- **Staking**: Stake eggs for additional DBTC yield
- **Power Decay**: Require active mining to maintain power (prevents dead eggs)

---

**Last Updated**: October 15, 2025
**Program Version**: 1.0.0
**Author**: MoonBase Team

