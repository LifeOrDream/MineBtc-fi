# Dragon Egg NFT Locking System

## Critical Security Fix: NFT Custody Implementation

### Problem Identified
The original implementation had a **critical security flaw**:
- Dragon Egg NFTs remained in the user's wallet during "incubation"
- Only state variables tracked the incubation status
- Users could transfer/sell the NFT while still receiving power bonuses
- No actual locking mechanism existed

### Solution Implemented
**True NFT Locking via Custody PDA**

When a Dragon Egg is incubated, the NFT is **physically transferred** to a custody PDA. The user cannot transfer or sell it until they remove it from incubation.

---

## How It Works

### 1. Incubation (Lock NFT)

```
User calls: incubate_dragon_egg()

Flow:
1. Verify user owns the NFT (via Metaplex Core owner check)
2. Verify egg is not already incubated
3. Verify moonbase has no egg (max 1 per moonbase)
4. **Transfer NFT from user wallet → custody PDA**
5. Update state variables:
   - user_moonbase.incubated_dragon_egg = Some(egg_metadata_pubkey)
   - incubation_state.incubated_egg = Some(nft_mint)
   - egg_metadata.incubated_moonbase = Some(user_pubkey)
6. Emit event

Result: NFT is LOCKED in custody PDA, user cannot transfer it
```

### 2. Power Growth (Auto during DBTC claims)

```
User calls: claim_dbtc_tokens()

If user_moonbase.incubated_dragon_egg.is_some():
  - Calculate power_increase = claimed_dbtc / POWER_RATE_MULTIPLIER
  - Update egg_metadata.power (capped at MAX_EGG_POWER)
  - Update incubation_state.total_power
  
NFT remains locked during this process
```

### 3. Removal (Unlock NFT)

```
User calls: remove_dragon_egg()

Flow:
1. Verify NFT is currently in custody PDA (not user wallet)
2. Verify egg is incubated
3. **Transfer NFT from custody PDA → user wallet**
   - Uses custody PDA as signer (via invoke_signed)
4. Update state variables (clear all incubation data)
5. Emit event with final power

Result: NFT is UNLOCKED and returned to user's wallet
```

---

## Technical Implementation

### Custody PDA
```rust
pub const DRAGON_EGG_CUSTODY_SEED: &[u8] = b"dragon-egg-custody";

// Derived address
Pubkey::find_program_address(&[DRAGON_EGG_CUSTODY_SEED], &moonbase_program_id)
```

### NFT Transfer Helper
```rust
pub fn transfer_mpl_core_asset(
    asset: &AccountInfo,
    collection: Option<&AccountInfo>,
    payer: &AccountInfo,
    authority: &AccountInfo,  // Current owner/signer
    new_owner: &AccountInfo,  // Destination
    mpl_core_program: &AccountInfo,
) -> Result<()>
```

### Incubate (Lock)
```rust
// Transfer NFT: user → custody PDA
transfer_mpl_core_asset(
    dragon_egg_asset,
    dragon_egg_collection,
    user,           // payer
    user,           // authority (user owns it)
    egg_custody_pda, // new owner (PDA)
    mpl_core_program,
)?;
```

### Remove (Unlock)
```rust
// Get PDA signer seeds
let custody_seeds = &[
    DRAGON_EGG_CUSTODY_SEED,
    &[bump],
];
let signer_seeds = &[&custody_seeds[..]];

// Transfer NFT: custody PDA → user
// Use invoke_signed since PDA needs to sign
cpi_builder
    .asset(dragon_egg_asset)
    .payer(user)
    .authority(egg_custody_pda) // PDA is authority
    .new_owner(user)            // user gets it back
    .invoke_signed(signer_seeds)?;
```

---

## Account Requirements

### IncubateDragonEgg Context
```rust
#[derive(Accounts)]
pub struct IncubateDragonEgg<'info> {
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,
    pub dragon_egg_asset: UncheckedAccount<'info>,         // Mutable
    pub dragon_egg_collection: Option<UncheckedAccount>,   // Optional
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,
    pub incubation_state: Account<'info, IncubationState>,
    pub egg_custody_pda: UncheckedAccount<'info>,          // Custody PDA
    pub mpl_core_program: UncheckedAccount<'info>,         // Metaplex Core
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
```

### RemoveDragonEgg Context
```rust
#[derive(Accounts)]
pub struct RemoveDragonEgg<'info> {
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,
    pub dragon_egg_asset: UncheckedAccount<'info>,         // Mutable
    pub dragon_egg_collection: Option<UncheckedAccount>,   // Optional
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,
    pub incubation_state: Account<'info, IncubationState>,
    pub egg_custody_pda: UncheckedAccount<'info>,          // Custody PDA (signer)
    pub mpl_core_program: UncheckedAccount<'info>,         // Metaplex Core
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
```

---

## Security Guarantees

✅ **NFT Cannot Be Transferred While Incubated**
- NFT owner is the custody PDA, not the user
- User wallet cannot sign for the NFT transfer
- Metaplex Core enforces owner authority

✅ **NFT Cannot Be Sold While Incubated**
- NFT is not in user's wallet
- Cannot list on marketplaces
- Cannot transfer to escrow accounts

✅ **No Double-Incubation**
- Each moonbase can only have 1 egg (`user_moonbase.incubated_dragon_egg`)
- Each egg can only be in 1 moonbase (`egg_metadata.incubated_moonbase`)

✅ **Ownership Verification**
- **Incubate**: Verifies user owns the NFT before accepting it
- **Remove**: Verifies custody PDA owns the NFT before returning it
- **Claim**: Verifies egg is incubated before granting power bonuses

---

## State Tracking

### UserMoonBaseInstance
```rust
pub struct UserMoonBaseInstance {
    // ... other fields
    pub incubated_dragon_egg: Option<Pubkey>, // DragonEggMetadata pubkey
}
```

### DragonEggMetadata
```rust
pub struct DragonEggMetadata {
    pub mint: Pubkey,                      // NFT mint address
    pub power: u32,                        // Current power (grows with DBTC)
    pub dna: [u8; 32],                     // Unique DNA
    pub incubated_moonbase: Option<Pubkey>, // User pubkey (if incubated)
    pub last_update_ts: i64,
    pub created_at: i64,
    pub bump: u8,
}
```

### IncubationState
```rust
pub struct IncubationState {
    pub moonbase_owner: Pubkey,
    pub incubated_egg: Option<Pubkey>,    // NFT mint (if egg is incubated)
    pub total_power: u64,
    pub last_update_ts: i64,
    pub bump: u8,
}
```

---

## Frontend Integration Guide

### 1. Check if NFT is Locked
```typescript
// Fetch the Dragon Egg NFT asset
const eggAsset = await fetchAssetV1(umi, eggMintPubkey);

// Check owner
if (eggAsset.owner === custodyPdaPubkey) {
  // NFT is locked (incubated)
} else if (eggAsset.owner === userWalletPubkey) {
  // NFT is in user wallet (not incubated)
}
```

### 2. Incubate (Lock)
```typescript
const tx = await program.methods
  .incubateDragonEgg()
  .accounts({
    userMoonbase: userMoonbasePda,
    dragonEggAsset: eggMintPubkey,
    dragonEggCollection: collectionPubkey, // or null
    dragonEggMetadata: eggMetadataPda,
    incubationState: incubationStatePda,
    eggCustodyPda: custodyPda,
    mplCoreProgram: MPL_CORE_PROGRAM_ID,
    user: wallet.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

### 3. Remove (Unlock)
```typescript
const tx = await program.methods
  .removeDragonEgg()
  .accounts({
    userMoonbase: userMoonbasePda,
    dragonEggAsset: eggMintPubkey,
    dragonEggCollection: collectionPubkey, // or null
    dragonEggMetadata: eggMetadataPda,
    incubationState: incubationStatePda,
    eggCustodyPda: custodyPda,
    mplCoreProgram: MPL_CORE_PROGRAM_ID,
    user: wallet.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

---

## Error Codes

| Code | Description |
|------|-------------|
| `EggAlreadyIncubated` | Trying to incubate an egg that's already incubated |
| `MaxEggsReached` | Moonbase already has 1 egg (max limit) |
| `EggNotIncubated` | Trying to remove/claim bonuses for non-incubated egg |
| `NftNotOwnedByUser` | NFT ownership verification failed |
| `InvalidAccount` | Invalid NFT account or metadata |
| `InvalidMplCoreProgram` | Wrong Metaplex Core program provided |

---

## Migration Notes

**If upgrading from old system:**
1. Old "incubated" eggs (not in custody PDA) will need to be "removed" and re-incubated
2. Check `egg_metadata.incubated_moonbase` for migration status
3. Frontend should warn users about the migration requirement

**For new deployments:**
- All eggs start in user wallets
- Incubation always locks the NFT in custody PDA
- No migration needed

---

## Testing Checklist

- [ ] User can incubate egg (NFT transfers to custody PDA)
- [ ] User cannot transfer incubated egg
- [ ] User cannot sell incubated egg
- [ ] Power grows correctly during DBTC claims
- [ ] User can remove egg (NFT returns to wallet)
- [ ] User can transfer egg after removal
- [ ] Cannot incubate same egg twice
- [ ] Cannot incubate 2 eggs in same moonbase
- [ ] Custody PDA can sign for NFT return

---

**Last Updated**: October 15, 2025  
**Security Level**: ✅ **CRITICAL FIX IMPLEMENTED**  
**Status**: Ready for testing and deployment

