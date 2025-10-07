# NFT Launchpad Program - MoonDoge & Dragon Egg NFTs

## Overview

The NFT Launchpad program manages two Metaplex Core NFT collections for the MoonBase ecosystem:

1. **MoonDoge NFTs** (Limited: 10,000 supply)
   - Contains "money" attribute that increases with mDOGE mining
   - 1 per moonbase maximum
   - Can be attached/detached from moonbase

2. **Dragon Egg NFTs** (Unlimited: Initial 15,000)
   - Contains "power" attribute that increases with hashpower accumulation
   - Contains 32-byte DNA for breeding/evolution
   - Multiple eggs can be incubated per moonbase (max 10)

---

## Architecture

### Collections (Metaplex Core)

```
MoonDoge Collection
├── Collection Asset (Metaplex Core)
├── Individual MoonDoge Assets
└── MoonDogeMetadata Accounts (Program state)

Dragon Egg Collection
├── Collection Asset (Metaplex Core)
├── Individual Dragon Egg Assets
└── DragonEggMetadata Accounts (Program state)
```

### State Accounts

**GlobalConfig**: Program-wide settings and counters
- Authority, treasury, collection addresses
- Total minted counters
- Total SOL collected

**MoonDogeMetadata** (per NFT):
- Mint address
- Owner
- Money (u64) - increases with mDOGE mining
- Attached moonbase (optional)
- Last update timestamp
- Total mDOGE mined

**DragonEggMetadata** (per NFT):
- Mint address
- Owner
- Power (u32) - increases with hashpower
- DNA (32 bytes)
- Incubated moonbase (optional)
- Last update timestamp
- Total hashpower accumulated

**IncubationState** (per moonbase):
- Moonbase owner
- List of incubated egg mints (max 10)
- Last update timestamp
- Total power accumulated

**DogeAttachment** (per moonbase):
- Moonbase owner
- Attached doge mint
- Last update timestamp
- Last mDOGE balance (for tracking delta)

---

## Pricing Tiers

### Moonbase Creation Bundles

| Tier | Cost | Includes |
|------|------|----------|
| Basic | 0.25 SOL | No NFT |
| Doge | 0.5 SOL | + MoonDoge NFT |
| Full | 1.0 SOL | MoonDoge + Dragon Egg |

### Individual Purchases

- **MoonDoge**: 0.5 SOL
- **Dragon Egg**: 0.5 SOL

---

## Instructions

### Admin Instructions

#### `initialize`
Creates the program with both NFT collections.

```rust
pub fn initialize(
    ctx: Context<Initialize>,
    moondoge_collection_name: String,
    moondoge_collection_symbol: String,
    moondoge_collection_uri: String,
    dragon_egg_collection_name: String,
    dragon_egg_collection_symbol: String,
    dragon_egg_collection_uri: String,
) -> Result<()>
```

**Accounts**:
- `global_config` (init, PDA)
- `moondoge_collection` (Metaplex Core collection)
- `dragon_egg_collection` (Metaplex Core collection)
- `sol_treasury` (init, PDA)
- `authority` (signer)
- `system_program`

#### `update_config`
Update program authority or treasury.

#### `pause_program`
Emergency pause/unpause functionality.

---

### User Instructions

#### `mint_nfts_for_moonbase`
**Called by moonbase program during moonbase creation.**

```rust
pub fn mint_nfts_for_moonbase(
    ctx: Context<MintNftsForMoonbase>,
    pricing_tier: u64, // MOONBASE_BASIC_PRICE, MOONBASE_DOGE_PRICE, or MOONBASE_FULL_PRICE
) -> Result<()>
```

Automatically mints appropriate NFTs based on pricing tier:
- 0.25 SOL: No NFTs
- 0.5 SOL: MoonDoge only
- 1.0 SOL: MoonDoge + Dragon Egg

**Accounts**:
- `global_config`
- `sol_treasury`
- `moondoge_mint` (optional, if tier includes)
- `moondoge_metadata` (optional, init if needed)
- `dragon_egg_mint` (optional, if tier includes)
- `dragon_egg_metadata` (optional, init if needed)
- `user` (signer)

#### `purchase_moondoge`
Purchase MoonDoge NFT for 0.5 SOL.

**Accounts**:
- `global_config`
- `sol_treasury`
- `moondoge_mint` (Metaplex Core asset)
- `moondoge_metadata` (init)
- `user` (signer)

#### `purchase_dragon_egg`
Purchase Dragon Egg NFT for 0.5 SOL.

**Accounts**:
- `global_config`
- `sol_treasury`
- `dragon_egg_mint` (Metaplex Core asset)
- `dragon_egg_metadata` (init)
- `user` (signer)

#### `attach_moondoge`
Attach MoonDoge to user's moonbase (1 per moonbase max).

**Accounts**:
- `moondoge_metadata` (mut)
- `doge_attachment` (init)
- `user` (signer)

#### `detach_moondoge`
Detach MoonDoge from moonbase.

**Accounts**:
- `moondoge_metadata` (mut)
- `doge_attachment` (mut, close)
- `user` (signer)

#### `update_moondoge_money`
Update MoonDoge money based on mDOGE mined.

**Formula**: `money_increase = (mdoge_mined * MONEY_RATE_MULTIPLIER) / 1,000,000`

**Accounts**:
- `moondoge_metadata` (mut)
- `doge_attachment` (mut)
- `user` (not signer, for PDA)

#### `incubate_dragon_egg`
Add Dragon Egg to moonbase incubation (max 10 eggs).

**Accounts**:
- `dragon_egg_metadata` (mut)
- `incubation_state` (init if needed)
- `user` (signer)

#### `remove_dragon_egg`
Remove Dragon Egg from incubation.

**Accounts**:
- `dragon_egg_metadata` (mut)
- `incubation_state` (mut)
- `user` (signer)

#### `update_dragon_egg_power`
Update Dragon Egg power based on hashpower.

**Formula**: `power_increase = (total_hashpower / total_eggs) * time_elapsed / POWER_RATE_MULTIPLIER`

**Accounts**:
- `dragon_egg_metadata` (mut)
- `incubation_state` (mut)
- `user` (not signer, for PDA)

---

## Integration with MoonBase Program

### Moonbase Creation Flow

```rust
// In moonbase program's create_user_moonbase function:

// 1. Validate pricing tier
let pricing_tier = match payment_amount {
    250_000_000 => "basic",
    500_000_000 => "doge",
    1_000_000_000 => "full",
    _ => return Err(error)
};

// 2. Create moonbase account (existing logic)
// ... your existing moonbase creation code ...

// 3. Call NFT launchpad via CPI to mint NFTs
if pricing_tier != "basic" {
    let cpi_program = ctx.accounts.nft_launchpad_program.to_account_info();
    let cpi_accounts = MintNftsForMoonbase {
        global_config: ctx.accounts.nft_global_config.to_account_info(),
        sol_treasury: ctx.accounts.nft_sol_treasury.to_account_info(),
        moondoge_mint: if pricing_tier == "doge" || pricing_tier == "full" {
            Some(ctx.accounts.moondoge_mint.to_account_info())
        } else { None },
        moondoge_metadata: if pricing_tier == "doge" || pricing_tier == "full" {
            Some(ctx.accounts.moondoge_metadata.to_account_info())
        } else { None },
        dragon_egg_mint: if pricing_tier == "full" {
            Some(ctx.accounts.dragon_egg_mint.to_account_info())
        } else { None },
        dragon_egg_metadata: if pricing_tier == "full" {
            Some(ctx.accounts.dragon_egg_metadata.to_account_info())
        } else { None },
        user: ctx.accounts.user.to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
    
    nfts_launchpad::cpi::mint_nfts_for_moonbase(cpi_ctx, payment_amount)?;
}
```

### Backend Update Flows

**Update MoonDoge Money** (periodically, e.g., every hour):
```javascript
// 1. Query moonbase to get mDOGE mined since last update
const mdogeMined = await getMdogeDelta(moonbaseOwner);

// 2. Call update_moondoge_money
await program.methods
  .updateMoonDogeMoney(new BN(mdogeMined))
  .accounts({
    moonDogeMetadata: moonDogeMetadataPDA,
    dogeAttachment: dogeAttachmentPDA,
    user: moonbaseOwner,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

**Update Dragon Egg Power** (periodically, e.g., every hour):
```javascript
// 1. Query moonbase to get total hashpower
const totalHashpower = await getMoonbaseHashpower(moonbaseOwner);

// 2. For each incubated egg, call update_dragon_egg_power
const incubationState = await getIncubationState(moonbaseOwner);

for (const eggMint of incubationState.incubatedEggs) {
  await program.methods
    .updateDragonEggPower(new BN(totalHashpower))
    .accounts({
      dragonEggMetadata: eggMetadataPDA,
      incubationState: incubationStatePDA,
      user: moonbaseOwner,
      systemProgram: SystemProgram.programId,
    })
    .rpc();
}
```

---

## Game Mechanics

### MoonDoge Money Accumulation

- **Trigger**: Attached doge accumulates money when moonbase mines mDOGE
- **Formula**: `money += (mdoge_mined * 0.01%) = mdoge_mined * 100 / 1,000,000`
- **Max Money**: `u64::MAX / 1000` (plenty of headroom)
- **Use Cases**: Future game features, marketplace trading, upgrades

### Dragon Egg Power Accumulation

- **Trigger**: Incubated eggs accumulate power based on moonbase hashpower
- **Formula**: `power += (total_hashpower / total_eggs) * hours_elapsed / 1000`
- **Max Power**: 1,000,000
- **Use Cases**: Breeding (higher power = better offspring), evolution, PvP battles

---

## Constants

```rust
// Supply limits
MAX_MOONDOGE_SUPPLY: 10,000
INITIAL_DRAGON_EGG_SUPPLY: 15,000

// Pricing
MOONBASE_BASIC_PRICE: 0.25 SOL
MOONBASE_DOGE_PRICE: 0.5 SOL
MOONBASE_FULL_PRICE: 1.0 SOL
MOONDOGE_PRICE: 0.5 SOL
DRAGON_EGG_PRICE: 0.5 SOL

// Gameplay
MAX_EGGS_PER_MOONBASE: 10
MAX_DOGES_PER_MOONBASE: 1
POWER_RATE_MULTIPLIER: 1000
MONEY_RATE_MULTIPLIER: 100
UPDATE_FREQUENCY_SECONDS: 3600 (1 hour)

// Attributes
BASE_EGG_POWER: 100
MAX_EGG_POWER: 1,000,000
BASE_DOGE_MONEY: 0
MAX_DOGE_MONEY: u64::MAX / 1000
```

---

## PDA Seeds

```rust
GLOBAL_CONFIG: ["global-config"]
SOL_TREASURY: ["sol-treasury"]
MOONDOGE_METADATA: ["moondoge-metadata", mint]
DRAGON_EGG_METADATA: ["dragon-egg-metadata", mint]
INCUBATION_STATE: ["incubation-state", moonbase_owner]
DOGE_ATTACHMENT: ["doge-attachment", moonbase_owner]
```

---

## Events

### Collection Events
- `ProgramInitialized`
- `ConfigUpdated`
- `CollectionStatsUpdated`

### MoonDoge Events
- `MoonDogeMinted`
- `MoonDogeAttached`
- `MoonDogeDetached`
- `MoonDogeMoneyUpdated`

### Dragon Egg Events
- `DragonEggMinted`
- `DragonEggIncubated`
- `DragonEggRemoved`
- `DragonEggPowerUpdated`

### Economic Events
- `SOLFeesCollected`
- `MoonbaseCreatedWithNfts`

---

## Error Codes

```rust
Unauthorized
InvalidAuthority
MoonDogeNotFound
DragonEggNotFound
MaxMoonDogeSupplyReached
MaxDragonEggSupplyReached
NftNotOwnedByUser
DogeAlreadyAttached
EggAlreadyIncubated
MoonbaseAlreadyHasDoge
MaxEggsReached
DogeNotAttached
EggNotIncubated
InsufficientSOLBalance
InvalidPaymentAmount
InvalidPricingTier
ProgramPaused
ArithmeticOverflow
... (see errors.rs for complete list)
```

---

## Future Extensions

1. **Breeding System**: Use Dragon Egg DNA to breed new eggs with combined traits
2. **Evolution System**: Evolve eggs into dragons when power threshold reached
3. **PvP Integration**: Use egg power in battles
4. **Marketplace**: Trade NFTs with accumulated attributes
5. **Staking**: Stake MoonDoge for additional benefits
6. **Rarities**: DNA-based rarity system for eggs

---

## Testing

```bash
# Build
anchor build

# Test
anchor test

# Deploy (devnet)
anchor deploy --provider.cluster devnet
```

---

## Security Considerations

1. **Supply Limits**: MoonDoge capped at 10k, enforced on-chain
2. **Ownership Checks**: All operations verify NFT ownership
3. **Attachment Limits**: Max 1 doge, max 10 eggs per moonbase
4. **PDA Derivation**: All accounts use canonical PDAs
5. **SOL Treasury**: Separate PDA for collected fees
6. **Pause Mechanism**: Emergency pause functionality
7. **Arithmetic Safety**: Saturating math used throughout

---

## Notes

- This implementation uses placeholder logic for Metaplex Core asset creation
- In production, you'll need to add proper Metaplex Core CPIs for:
  - Creating collection assets
  - Minting individual assets
  - Transferring assets
- URI generation should use actual IPFS/Arweave endpoints
- Consider adding indexing for efficient NFT queries
- Backend automation recommended for power/money updates

