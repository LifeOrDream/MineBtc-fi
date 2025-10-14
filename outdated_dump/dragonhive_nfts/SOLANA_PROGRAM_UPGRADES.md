# Solana Program Upgrades Guide

## Overview

Solana programs deployed with the **BPF Upgradeable Loader** can be upgraded while maintaining the same program ID. This is crucial for adding features like breeding, evolution, etc. to your NFT Launchpad.

## How Upgrades Work

### Program Accounts Structure

When you deploy an upgradeable program, Solana creates 3 accounts:

```
┌─────────────────────────────────────────────────┐
│  Program Account (Your Program ID)             │
│  - Points to Program Data Account              │
│  - Executable                                   │
└─────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│  Program Data Account                           │
│  - Contains actual bytecode                     │
│  - Stores upgrade authority                     │
│  - Can be replaced                              │
└─────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│  Upgrade Authority (Your Wallet)                │
│  - Can upgrade the program                      │
│  - Can be transferred or revoked               │
└─────────────────────────────────────────────────┘
```

### Upgrade Process

```bash
# 1. Build new version
anchor build

# 2. Upgrade the program (keeps same program ID)
anchor upgrade target/deploy/nfts_launchpad.so \
  --program-id <PROGRAM_ID> \
  --provider.cluster devnet
```

**Key Point**: The program ID stays the same, only the bytecode changes!

## What You CAN Change ✅

### 1. Add New Instructions
```rust
// OLD VERSION - v1.0
pub mod nfts_launchpad {
    pub fn purchase_moondoge(ctx: Context<PurchaseMoonDoge>) -> Result<()> { ... }
    pub fn purchase_dragon_egg(ctx: Context<PurchaseDragonEgg>) -> Result<()> { ... }
}

// NEW VERSION - v2.0 (ADD breeding/evolution)
pub mod nfts_launchpad {
    // Old functions still work
    pub fn purchase_moondoge(ctx: Context<PurchaseMoonDoge>) -> Result<()> { ... }
    pub fn purchase_dragon_egg(ctx: Context<PurchaseDragonEgg>) -> Result<()> { ... }
    
    // NEW: Add breeding functionality
    pub fn breed_dragons(
        ctx: Context<BreedDragons>,
        parent1_mint: Pubkey,
        parent2_mint: Pubkey,
    ) -> Result<()> { ... }
    
    // NEW: Add evolution functionality
    pub fn evolve_dragon(
        ctx: Context<EvolveDragon>,
        egg_mint: Pubkey,
    ) -> Result<()> { ... }
}
```

### 2. Modify Instruction Logic
```rust
// OLD: Simple power calculation
pub fn update_dragon_egg_power(ctx: Context<UpdateDragonEggPower>, total_hashpower: u64) -> Result<()> {
    let power_increase = total_hashpower / total_eggs;
    egg_metadata.power += power_increase;
    Ok(())
}

// NEW: Enhanced power calculation with multipliers
pub fn update_dragon_egg_power(ctx: Context<UpdateDragonEggPower>, total_hashpower: u64) -> Result<()> {
    let base_increase = total_hashpower / total_eggs;
    let rarity_multiplier = get_rarity_multiplier(&egg_metadata.dna);
    let power_increase = base_increase * rarity_multiplier;
    egg_metadata.power += power_increase;
    Ok(())
}
```

### 3. Add New Account Types
```rust
// NEW: Add breeding state account
#[account]
pub struct BreedingState {
    pub parent1: Pubkey,
    pub parent2: Pubkey,
    pub breeding_start_ts: i64,
    pub breeding_duration: i64,
    pub child_egg: Option<Pubkey>,
    pub bump: u8,
}

// NEW: Add evolution stages
#[account]
pub struct DragonEvolution {
    pub egg_mint: Pubkey,
    pub evolution_stage: u8,  // 0 = egg, 1 = hatchling, 2 = adult, 3 = legendary
    pub evolution_history: Vec<i64>,  // timestamps of evolutions
    pub bump: u8,
}
```

### 4. Add New Events
```rust
// NEW: Breeding events
#[event]
pub struct DragonsBred {
    pub parent1: Pubkey,
    pub parent2: Pubkey,
    pub child_egg: Pubkey,
    pub child_dna: [u8; 32],
    pub breeding_timestamp: i64,
}

#[event]
pub struct DragonEvolved {
    pub egg_mint: Pubkey,
    pub old_stage: u8,
    pub new_stage: u8,
    pub evolution_timestamp: i64,
}
```

### 5. Add New Errors
```rust
#[error_code]
pub enum NftLaunchpadError {
    // ... existing errors ...
    
    // NEW: Breeding errors
    #[msg("Cannot breed dragons - cooldown not finished")]
    BreedingCooldownActive,
    
    #[msg("Incompatible dragon DNA for breeding")]
    IncompatibleDNA,
    
    // NEW: Evolution errors
    #[msg("Dragon not ready to evolve - insufficient power")]
    InsufficientPowerForEvolution,
}
```

## What You CANNOT Change ❌

### 1. Existing Account Structures (Without Migration)

**BREAKING CHANGE** - This will cause errors:
```rust
// OLD VERSION
#[account]
pub struct DragonEggMetadata {
    pub mint: Pubkey,           // 32 bytes
    pub power: u32,             // 4 bytes
    pub dna: [u8; 32],          // 32 bytes
    pub bump: u8,               // 1 byte
}  // Total: 69 bytes

// ❌ WRONG: Cannot just add fields to existing accounts
#[account]
pub struct DragonEggMetadata {
    pub mint: Pubkey,           
    pub power: u32,             
    pub dna: [u8; 32],          
    pub evolution_stage: u8,    // ❌ This breaks existing accounts!
    pub bump: u8,               
}
```

**Why this breaks**: Existing `DragonEggMetadata` accounts are only 69 bytes. The program expects 70 bytes now, causing deserialization errors.

### 2. Account Discriminators

Anchor auto-generates discriminators. Don't manually change them:
```rust
// Discriminator is hash of "account:DragonEggMetadata"
// Changing account name changes discriminator = breaks everything
```

### 3. PDA Seeds (Without Migration)

**BREAKING CHANGE**:
```rust
// OLD
#[account(
    seeds = [b"dragon_egg_metadata", mint.key().as_ref()],
    bump
)]

// ❌ WRONG: Changes PDA address
#[account(
    seeds = [b"dragon_metadata_v2", mint.key().as_ref()],  // Different address!
    bump
)]
```

## How to Add New Features Properly ✅

### Strategy 1: Create New Accounts (Recommended)

```rust
// Keep existing DragonEggMetadata unchanged
#[account]
pub struct DragonEggMetadata {
    pub mint: Pubkey,
    pub power: u32,
    pub dna: [u8; 32],
    pub incubated_moonbase: Option<Pubkey>,
    pub last_update_ts: i64,
    pub total_hashpower_accumulated: u64,
    pub created_at: i64,
    pub bump: u8,
}

// Add NEW account for evolution features
#[account]
pub struct DragonEvolutionData {
    pub egg_mint: Pubkey,           // Links to DragonEggMetadata
    pub evolution_stage: u8,
    pub evolution_power: u64,
    pub breed_count: u8,
    pub last_breed_ts: i64,
    pub special_traits: [u8; 16],
    pub bump: u8,
}

// Usage in instruction
#[derive(Accounts)]
pub struct EvolveDragon<'info> {
    #[account(mut)]
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,  // Existing
    
    #[account(
        init,
        payer = user,
        space = DragonEvolutionData::LEN,
        seeds = [b"dragon_evolution", dragon_egg_metadata.mint.as_ref()],
        bump
    )]
    pub evolution_data: Account<'info, DragonEvolutionData>,  // New!
    
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
```

### Strategy 2: Version-Based Accounts

```rust
#[account]
pub struct GlobalConfig {
    pub authority: Pubkey,
    // ... existing fields ...
    pub config_version: u8,  // Add version field to new accounts
}

// Create v2 config with new features
#[account]
pub struct GlobalConfigV2 {
    pub authority: Pubkey,
    // ... all v1 fields ...
    pub breeding_enabled: bool,
    pub evolution_enabled: bool,
    pub breeding_fee: u64,
    pub evolution_fee: u64,
    pub config_version: u8,  // = 2
}
```

### Strategy 3: Use Reserved Space (For Future Upgrades)

```rust
// When creating NEW accounts, include reserved space
#[account]
pub struct DragonBreedingState {
    pub parent1: Pubkey,        // 32
    pub parent2: Pubkey,        // 32
    pub breeding_start_ts: i64, // 8
    pub child_egg: Option<Pubkey>, // 33
    pub bump: u8,               // 1
    
    pub reserved: [u8; 64],     // Reserve 64 bytes for future features!
}

impl DragonBreedingState {
    pub const LEN: usize = 8 + 32 + 32 + 8 + 33 + 1 + 64;
}
```

## Impact on Other Programs

### Do Other Programs Need to Upgrade?

**Short Answer**: Only if you make **breaking changes**.

### Non-Breaking Changes (No upgrade needed)

```rust
// Your NFT Launchpad v1.0
pub fn purchase_moondoge(ctx: Context<PurchaseMoonDoge>) -> Result<()> { ... }

// Moonbase program calls it:
nfts_launchpad::cpi::purchase_moondoge(cpi_ctx)?;

// Your NFT Launchpad v2.0 - ADD new function (non-breaking)
pub fn purchase_moondoge(ctx: Context<PurchaseMoonDoge>) -> Result<()> { ... }
pub fn breed_dragons(ctx: Context<BreedDragons>) -> Result<()> { ... }  // NEW

// ✅ Moonbase program still works! No upgrade needed.
```

### Breaking Changes (Requires upgrade)

```rust
// NFT Launchpad v1.0
pub fn purchase_moondoge(ctx: Context<PurchaseMoonDoge>) -> Result<()> { ... }

// NFT Launchpad v2.0 - CHANGE signature (breaking!)
pub fn purchase_moondoge(
    ctx: Context<PurchaseMoonDoge>,
    new_required_param: u64,  // ❌ Breaking change!
) -> Result<()> { ... }

// ❌ Moonbase program breaks - must be upgraded to pass new parameter
```

## Best Practices for Upgradeable Programs

### 1. Design for Extensibility
```rust
// Use feature flags in GlobalConfig
#[account]
pub struct GlobalConfig {
    pub authority: Pubkey,
    pub features_enabled: u64,  // Bitflags for features
    // Bit 0: breeding enabled
    // Bit 1: evolution enabled
    // Bit 2: PvP enabled
    // etc.
}
```

### 2. Version Your Instructions
```rust
// Keep old versions for backward compatibility
pub fn purchase_moondoge_v1(ctx: Context<PurchaseMoonDogeV1>) -> Result<()> { ... }
pub fn purchase_moondoge_v2(ctx: Context<PurchaseMoonDogeV2>, extra_param: u64) -> Result<()> { ... }
```

### 3. Use Migration Instructions
```rust
pub fn migrate_dragon_egg_to_v2(
    ctx: Context<MigrateDragonEgg>,
) -> Result<()> {
    let old_metadata = &ctx.accounts.old_dragon_egg_metadata;
    let new_metadata = &mut ctx.accounts.new_dragon_egg_metadata;
    
    // Copy old data
    new_metadata.mint = old_metadata.mint;
    new_metadata.power = old_metadata.power;
    new_metadata.dna = old_metadata.dna;
    
    // Initialize new fields
    new_metadata.evolution_stage = 0;
    new_metadata.breed_count = 0;
    
    // Close old account
    // (return lamports to user)
    
    Ok(())
}
```

### 4. Document Breaking Changes
```rust
// In your Anchor.toml or README
/*
BREAKING CHANGES:
v2.0.0:
  - purchase_moondoge now requires pricing_tier parameter
  - DragonEggMetadata account structure changed (migration required)
  
v1.0.0:
  - Initial release
*/
```

### 5. Test Upgrades on Devnet
```bash
# Always test upgrade path:
# 1. Deploy v1 to devnet
anchor deploy --provider.cluster devnet

# 2. Create some test data (NFTs, etc.)
# 3. Build v2
anchor build

# 4. Upgrade
anchor upgrade target/deploy/nfts_launchpad.so \
  --program-id <PROGRAM_ID> \
  --provider.cluster devnet

# 5. Verify old data still works
# 6. Test new features
```

## Adding Breeding & Evolution Example

Here's how you'd add breeding without breaking changes:

```rust
// Step 1: Add new accounts (no changes to existing ones)
#[account]
pub struct BreedingPair {
    pub parent1_mint: Pubkey,
    pub parent2_mint: Pubkey,
    pub breeding_start_ts: i64,
    pub breeding_end_ts: i64,
    pub is_complete: bool,
    pub child_mint: Option<Pubkey>,
    pub bump: u8,
}

// Step 2: Add new instructions
pub fn initiate_breeding(
    ctx: Context<InitiateBreeding>,
    parent1_mint: Pubkey,
    parent2_mint: Pubkey,
) -> Result<()> {
    // Verify ownership of both parent NFTs
    verify_nft_ownership(&ctx.accounts.parent1_asset, &ctx.accounts.user.key())?;
    verify_nft_ownership(&ctx.accounts.parent2_asset, &ctx.accounts.user.key())?;
    
    // Create breeding pair account
    let breeding_pair = &mut ctx.accounts.breeding_pair;
    breeding_pair.parent1_mint = parent1_mint;
    breeding_pair.parent2_mint = parent2_mint;
    breeding_pair.breeding_start_ts = Clock::get()?.unix_timestamp;
    breeding_pair.breeding_end_ts = breeding_pair.breeding_start_ts + BREEDING_DURATION;
    breeding_pair.is_complete = false;
    breeding_pair.bump = ctx.bumps.breeding_pair;
    
    Ok(())
}

pub fn complete_breeding(
    ctx: Context<CompleteBreeding>,
) -> Result<()> {
    let breeding_pair = &ctx.accounts.breeding_pair;
    
    require!(
        Clock::get()?.unix_timestamp >= breeding_pair.breeding_end_ts,
        NftLaunchpadError::BreedingNotComplete
    );
    
    // Generate child DNA from parents
    let parent1_metadata = &ctx.accounts.parent1_metadata;
    let parent2_metadata = &ctx.accounts.parent2_metadata;
    
    let child_dna = breed_dna(
        &parent1_metadata.dna,
        &parent2_metadata.dna,
        Clock::get()?.slot,
    );
    
    // Mint new dragon egg with hybrid DNA
    // ... (similar to existing minting logic)
    
    Ok(())
}

// Step 3: Add DNA breeding logic (new utility function)
pub fn breed_dna(parent1_dna: &[u8; 32], parent2_dna: &[u8; 32], slot: u64) -> [u8; 32] {
    let mut child_dna = [0u8; 32];
    
    // Crossover genetics (50% from each parent)
    for i in 0..32 {
        child_dna[i] = if (slot + i as u64) % 2 == 0 {
            parent1_dna[i]
        } else {
            parent2_dna[i]
        };
    }
    
    // Add mutation (5% chance per gene)
    for i in 0..32 {
        if (slot * (i as u64 + 1)) % 20 == 0 {
            child_dna[i] = child_dna[i].wrapping_add(1);
        }
    }
    
    child_dna
}
```

## Upgrade Authority Management

### Check Current Authority
```bash
solana program show <PROGRAM_ID> --url devnet
```

### Transfer Authority
```bash
solana program set-upgrade-authority <PROGRAM_ID> \
  --new-upgrade-authority <NEW_AUTHORITY_PUBKEY> \
  --url devnet
```

### Revoke Upgrades (Make Immutable)
```bash
solana program set-upgrade-authority <PROGRAM_ID> \
  --new-upgrade-authority none \
  --url mainnet-beta
```

**⚠️ WARNING**: Once revoked, the program can NEVER be upgraded again!

## Summary

✅ **You CAN**:
- Add new instructions (breeding, evolution, etc.)
- Modify instruction logic
- Add new account types
- Add new events and errors
- Upgrade without changing program ID

❌ **You CANNOT** (without migration):
- Change existing account structures
- Change PDA seeds
- Remove existing instructions
- Change instruction signatures

🎯 **Best Practice**:
- Always add new accounts for new features
- Use reserved space in new accounts
- Keep old functions unchanged
- Add versioned alternatives
- Test upgrades on devnet first

Your NFT Launchpad is perfectly positioned to add breeding and evolution features without breaking anything! 🚀

