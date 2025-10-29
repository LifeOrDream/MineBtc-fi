# Dragon Egg NFT Collection Setup Guide

## Overview

Dragon Eggs are **Metaplex Core NFTs** that users receive when creating a moonbase with the premium tier. These NFTs can be:
- **Incubated** in moonbases to gain power based on hashpower
- **Transferred** between users
- **Locked** in custody when incubated (prevents transfer while gaining power)
- **Released** from incubation to claim accumulated power

## Architecture

### 1. **Metaplex Core Collection**
- Created using `@metaplex-foundation/mpl-core`
- Stores collection-level metadata (name, URI)
- Acts as parent for all Dragon Egg NFTs

### 2. **MoonBase Program Integration**
- `GlobalConfig` stores the collection address
- `set_dragon_egg_collection()` - Sets the collection address (admin only)
- `add_dragon_egg_uris()` - Adds URI pool for random egg metadata
- Dragon Eggs are minted via CPI when users create moonbases

### 3. **NFT Lifecycle**
```
User Creates Moonbase (Premium Tier)
  ↓
Dragon Egg NFT Minted (from collection)
  ↓
User Owns Egg → Can Transfer or Incubate
  ↓
Incubate in Moonbase → NFT Locked in Custody PDA
  ↓
Egg Gains Power (based on moonbase hashpower × time)
  ↓
Remove from Incubation → NFT Returned, Power Finalized
```

## Setup Instructions

### Step 1: Install Dependencies

```bash
cd setup_scripts
npm install @metaplex-foundation/mpl-core @metaplex-foundation/umi @metaplex-foundation/umi-bundle-defaults @metaplex-foundation/umi-web3js-adapters
```

### Step 2: Configure Dragon Eggs

Update `config.json` with your Dragon Egg collection details:

```json
{
  "dragon_eggs": {
    "collection_name": "DogeTech Dragon Eggs",
    "collection_uri": "https://arweave.net/your-collection-metadata.json",
    "uris": [
      "https://arweave.net/dragon-egg-1.json",
      "https://arweave.net/dragon-egg-2.json",
      "https://arweave.net/dragon-egg-3.json"
    ]
  }
}
```

**Important**: 
- `collection_uri` should point to collection metadata JSON
- `uris` array should contain individual Dragon Egg metadata URIs
- URIs are randomly selected when eggs are minted

### Step 3: Prepare Metadata Files

#### Collection Metadata (collection_uri)
```json
{
  "name": "DogeTech Dragon Eggs",
  "description": "Mystical Dragon Eggs that gain power in moonbases",
  "image": "https://arweave.net/dragon-egg-collection-image.png",
  "external_url": "https://minebtc.fun/",
  "attributes": []
}
```

#### Individual Egg Metadata (each URI in uris array)
```json
{
  "name": "Dragon Egg",
  "description": "A mystical egg that grows stronger in moonbases",
  "image": "https://arweave.net/dragon-egg-image-{variant}.png",
  "attributes": [
    { "trait_type": "Type", "value": "Dragon Egg" },
    { "trait_type": "Rarity", "value": "Legendary" }
  ]
}
```

### Step 4: Run Collection Creation Script

```bash
node setup_scripts/4_create_dragon_egg_collection.js
```

This will:
1. ✅ Create the Metaplex Core collection
2. ✅ Set the collection address in MoonBase program
3. ✅ Add Dragon Egg URIs to the URI pool

## What Gets Created

### 1. Metaplex Core Collection
- **Address**: Generated during creation (deterministic keypair)
- **Update Authority**: Deployer wallet
- **Name**: From `config.dragon_eggs.collection_name`
- **URI**: From `config.dragon_eggs.collection_uri`

### 2. MoonBase Program Configuration
- `global_config.dragon_egg_collection` - Set to collection address
- `global_config.dragon_egg_uris` - Pool of metadata URIs
- `global_config.total_dragon_eggs_minted` - Counter (starts at 0)

## How Dragon Eggs Work in Game

### Minting (Automatic)
When a user creates a moonbase with `PRICE_TWO` (1.42 SOL):
```javascript
// In initialize_user_moonbase
if (pricing_tier == PRICE_TWO) {
  // Generate unique DNA
  let dna = generate_dragon_egg_dna(slot, user_key, moonbase_count);
  
  // Select random URI from pool
  let uri = global_config.get_random_dragon_egg_uri(slot, moonbase_count, dna);
  
  // Mint NFT via CPI to Metaplex Core
  create_mpl_core_asset(asset, collection, authority, payer, owner, ...);
}
```

### Incubation
Users can incubate eggs in their moonbase:
```rust
incubate_dragon_egg(ctx) -> Result<()>
```
- NFT is locked in custody PDA
- Egg gains power based on: `total_hashpower × time_elapsed / POWER_RATE_MULTIPLIER`
- Maximum power cap enforced

### Removal
Users can remove eggs from incubation:
```rust
remove_dragon_egg(ctx) -> Result<()>
```
- NFT is returned to user
- Final power is calculated and stored
- Egg can be transferred or incubated again

## Deployment State Tracking

The script updates `deployments/{cluster}.json` with:

```json
{
  "dragon_egg_collection_created": {
    "collection_address": "...",
    "collection_name": "DogeTech Dragon Eggs",
    "collection_uri": "https://...",
    "update_authority": "...",
    "timestamp": "2025-10-28T..."
  },
  "dragon_egg_collection_set_in_program": {
    "collection_address": "...",
    "global_config_pda": "...",
    "tx_signature": "...",
    "timestamp": "2025-10-28T..."
  },
  "dragon_egg_uris_added": {
    "uris_count": 5,
    "uris": ["...", "...", "..."],
    "tx_signature": "...",
    "timestamp": "2025-10-28T..."
  }
}
```

## Troubleshooting

### Issue: "Collection already exists"
**Solution**: The script is idempotent. It will skip creation and use the existing collection.

### Issue: "Insufficient balance"
**Solution**: Ensure deployer wallet has at least 0.1 SOL for collection creation.

### Issue: "MoonBase program not initialized"
**Solution**: Run `3_init_moonbase.js` before this script.

### Issue: "Invalid URI format"
**Solution**: Ensure all URIs in `config.dragon_eggs.uris` are valid HTTPS URLs or Arweave links.

## Testing Dragon Egg Minting

After setup, test by creating a user moonbase with premium tier:

```javascript
await moonbaseProgram.methods
  .createUserMoonbase(
    null,           // referrer
    0,              // faction_id
    1420000000      // PRICE_TWO (1.42 SOL) - includes Dragon Egg
  )
  .accounts({
    // ... accounts including dragon_egg_collection
  })
  .rpc();
```

## Security Considerations

1. **Update Authority**: The deployer retains update authority over the collection
2. **Collection Verification**: All eggs are verified members of the collection
3. **Custody Model**: Incubated eggs are held by a PDA, not transferable
4. **Power Calculation**: Server-side validation ensures power increases are legitimate

## Next Steps

After running this script:
1. ✅ Dragon Egg collection is live
2. ✅ MoonBase program is configured
3. ✅ Users can create moonbases with Dragon Eggs
4. ✅ Frontend can display Dragon Egg NFTs
5. ✅ Incubation system is ready to use

