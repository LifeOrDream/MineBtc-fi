# URI System Update - Random Selection from Pool

## ✅ What Changed

Updated the NFT URI system to use a **pool of URIs stored in GlobalConfig** with **random selection on mint**, instead of generating URIs with a formula.

---

## 🎯 How It Works Now

### 1. **GlobalConfig Stores URI Pools**

```rust
pub struct GlobalConfig {
    // ... existing fields ...
    
    /// Available DogeBtc URIs (randomly selected on mint)
    pub moondoge_uris: Vec<String>,
    
    /// Available Dragon Egg URIs (randomly selected on mint)
    pub dragon_egg_uris: Vec<String>,
    
    // ...
}
```

**Capacity:** Up to 10 URIs per type (configurable)

### 2. **Random Selection Methods**

```rust
impl GlobalConfig {
    /// Select random DogeBtc URI based on slot and index
    pub fn get_random_moondoge_uri(&self, slot: u64, index: u64) -> Result<String> {
        let random_index = (slot + index) % self.moondoge_uris.len();
        Ok(self.moondoge_uris[random_index].clone())
    }
    
    /// Select random Dragon Egg URI based on slot, index, and DNA
    pub fn get_random_dragon_egg_uri(&self, slot: u64, index: u64, dna: &[u8; 32]) -> Result<String> {
        let dna_seed = u64::from_le_bytes([dna[0], dna[1], ..., dna[7]]);
        let random_index = (slot + index + dna_seed) % self.dragon_egg_uris.len();
        Ok(self.dragon_egg_uris[random_index].clone())
    }
}
```

**Randomness Sources:**
- **Slot**: Current blockchain slot (changes every 400ms)
- **Index**: Total NFTs minted (unique per NFT)
- **DNA** (eggs only): Unique DNA for extra randomness

---

## 🔧 Admin Functions

### Add URIs to Pool

```rust
// Add DogeBtc URIs
pub fn add_moondoge_uris(
    ctx: Context<UpdateConfig>,
    uris: Vec<String>,
) -> Result<()>

// Add Dragon Egg URIs
pub fn add_dragon_egg_uris(
    ctx: Context<UpdateConfig>,
    uris: Vec<String>,
) -> Result<()>
```

**Example:**
```typescript
await program.methods
  .addDogeBtcUris([
    "https://arweave.net/moondoge/variant1",
    "https://arweave.net/moondoge/variant2",
    "https://arweave.net/moondoge/variant3",
  ])
  .accounts({
    globalConfig,
    authority: adminKeypair.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

### Clear URI Pool

```rust
// Clear DogeBtc URIs
pub fn clear_moondoge_uris(
    ctx: Context<UpdateConfig>,
) -> Result<()>

// Clear Dragon Egg URIs
pub fn clear_dragon_egg_uris(
    ctx: Context<UpdateConfig>,
) -> Result<()>
```

**Use case:** Reset URIs before adding new set

---

## 💡 How Minting Uses URIs

### Before (Old):
```rust
// Generated URI with formula
let uri = format!("https://arweave.net/moondoge/{}", index);
```

### After (New):
```rust
// Random selection from pool
let uri = global_config.get_random_moondoge_uri(
    Clock::get()?.slot,
    index
)?;
```

---

## 🎮 Example Usage Flow

### Setup Phase (Admin)

```typescript
// 1. Deploy program
// 2. Initialize with collections
await program.methods.initialize(...).rpc();

// 3. Upload NFT images/metadata to Arweave/IPFS
const moonDogeUris = [
  "https://arweave.net/abc123/moondoge_red",
  "https://arweave.net/abc123/moondoge_blue",
  "https://arweave.net/abc123/moondoge_gold",
  "https://arweave.net/abc123/moondoge_rainbow",
];

const dragonEggUris = [
  "https://arweave.net/def456/egg_fire",
  "https://arweave.net/def456/egg_ice",
  "https://arweave.net/def456/egg_lightning",
  "https://arweave.net/def456/egg_nature",
];

// 4. Add URIs to program
await program.methods
  .addDogeBtcUris(moonDogeUris)
  .accounts({ globalConfig, authority })
  .rpc();

await program.methods
  .addDragonEggUris(dragonEggUris)
  .accounts({ globalConfig, authority })
  .rpc();
```

### Minting Phase (Users)

```typescript
// User mints DogeBtc #0
// → Slot: 12345, Index: 0
// → Random: (12345 + 0) % 4 = 1
// → URI: "https://arweave.net/abc123/moondoge_blue"

// User mints DogeBtc #1
// → Slot: 12350, Index: 1
// → Random: (12350 + 1) % 4 = 3
// → URI: "https://arweave.net/abc123/moondoge_rainbow"

// User mints Dragon Egg #0
// → Slot: 12355, Index: 0, DNA: [205, 31, ...]
// → DNA seed: u64 from first 8 bytes
// → Random: (12355 + 0 + dna_seed) % 4 = 2
// → URI: "https://arweave.net/def456/egg_lightning"
```

**Result:** Each NFT gets a random variant from the pool!

---

## 🎨 Use Cases

### 1. **Trait Variants**
```
Pool of 5 DogeBtc variants:
- Red Suit DogeBtc
- Blue Suit DogeBtc
- Gold Suit DogeBtc
- Space Suit DogeBtc
- Cyber Suit DogeBtc

Each mint randomly selects one!
```

### 2. **Rarity Distribution**
```
Pool with different rarities:
- Common URI (40% in pool)
- Uncommon URI (30% in pool)
- Rare URI (20% in pool)
- Legendary URI (10% in pool)

Random selection naturally creates rarity!
```

### 3. **Seasonal Updates**
```
// Add Halloween variants
await program.methods.clearDogeBtcUris().rpc();
await program.methods.addDogeBtcUris(halloweenUris).rpc();

// Add Christmas variants later
await program.methods.clearDogeBtcUris().rpc();
await program.methods.addDogeBtcUris(christmasUris).rpc();
```

### 4. **DNA-Based Selection (Eggs)**
```
Eggs with similar DNA tend to get similar URIs:
- DNA [1, 2, 3, ...] → Fire variant
- DNA [50, 51, 52, ...] → Ice variant
- DNA [100, 101, ...] → Lightning variant

Creates visual correlation with DNA!
```

---

## 🔒 Validation

### URI Length Validation
```rust
for uri in &uris {
    require!(
        uri.len() <= MAX_URI_LENGTH,  // 200 chars
        NftLaunchpadError::UriTooLong
    );
}
```

### Empty Pool Check
```rust
require!(
    !self.moondoge_uris.is_empty(),
    NftLaunchpadError::InvalidMetadata
);
```

**Protection:** Can't mint if no URIs in pool!

---

## 📊 Storage Costs

### Account Size Impact

**Before:** 
- GlobalConfig: ~200 bytes

**After:**
- GlobalConfig: ~4,500 bytes
  - 10 DogeBtc URIs × 204 bytes = 2,040 bytes
  - 10 Dragon Egg URIs × 204 bytes = 2,040 bytes
  - Overhead + existing fields = ~420 bytes

**Rent:** ~0.032 SOL for GlobalConfig (one-time)

### Optimization Options

If too expensive, reduce max URIs:
```rust
// Instead of 10 max URIs:
4 + (5 * (4 + MAX_URI_LENGTH)) +  // Only 5 URIs

// Or use shorter URIs with IPFS CIDs:
"Qm..." // 46 chars instead of 200
```

---

## 🚀 Benefits

### 1. **Flexibility**
- ✅ Change URI pool without redeploying
- ✅ Add seasonal variants
- ✅ Update metadata URI format

### 2. **Randomness**
- ✅ Each mint gets random variant
- ✅ Natural rarity distribution
- ✅ Unpredictable but deterministic

### 3. **Variety**
- ✅ Support multiple art styles
- ✅ Create trait variations
- ✅ DNA-correlated visuals (eggs)

### 4. **Control**
- ✅ Admin manages URI pool
- ✅ Can clear/update anytime
- ✅ Validate URIs before adding

---

## 📝 Migration from Old System

If you have URIs generated with old formula:

```typescript
// Old URIs were: https://arweave.net/moondoge/{index}
// Upload actual images with those paths, then:

const legacyUris = Array.from({ length: 1000 }, (_, i) => 
  `https://arweave.net/moondoge/${i}`
);

// Add in batches (vector size limits)
for (let i = 0; i < legacyUris.length; i += 10) {
  await program.methods
    .addDogeBtcUris(legacyUris.slice(i, i + 10))
    .rpc();
}
```

---

## 🧪 Testing

```typescript
// Test random distribution
const uris = [
  "https://test.com/variant1",
  "https://test.com/variant2",
  "https://test.com/variant3",
];

await program.methods.addDogeBtcUris(uris).rpc();

// Mint 100 NFTs, track distribution
const distribution = { 0: 0, 1: 0, 2: 0 };
for (let i = 0; i < 100; i++) {
  const nft = await mintDogeBtc();
  const uriIndex = uris.indexOf(nft.uri);
  distribution[uriIndex]++;
}

console.log("Distribution:", distribution);
// Should be roughly even: { 0: 33, 1: 34, 2: 33 }
```

---

## ✨ Summary

**What:**
- URI pools in GlobalConfig
- Random selection on mint
- Admin functions to manage URIs

**Why:**
- Flexibility (update URIs anytime)
- Variety (multiple art variants)
- Randomness (unpredictable mints)

**How:**
- Store URIs in vectors
- Select using (slot + index + DNA) % pool_size
- Admin adds/clears URIs as needed

**Result:**
Each NFT gets a random URI from the pool, creating natural variety and allowing easy updates! 🎨

