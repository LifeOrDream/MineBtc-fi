# Metaplex Core Integration - How NFTs Work

## 🔑 Key Concept: Two Separate Account Systems

### The NFT (Metaplex Core Asset) ≠ Our Metadata Account

```
┌─────────────────────────────────────────────────────────────┐
│                    METAPLEX CORE ASSET                      │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Address: 0xABC123... (the NFT mint/asset itself)   │   │
│  │  Owner: user_wallet (can trade, transfer, sell)     │   │
│  │  Name: "MoonDoge #42"                                │   │
│  │  URI: "https://arweave.net/moondoge/42"              │   │
│  │  Collection: moondoge_collection                     │   │
│  │  ─────────────────────────────────────────────────── │   │
│  │  STANDARD NFT DATA (ownership, metadata)             │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                            ↕️ LINKED BY MINT ADDRESS
┌─────────────────────────────────────────────────────────────┐
│                OUR METADATA ACCOUNT (PDA)                   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Address: PDA(["moondoge-metadata", mint])           │   │
│  │  mint: 0xABC123... (links to Metaplex asset above)   │   │
│  │  money: 1000 (game-specific attribute)               │   │
│  │  attached_moonbase: Some(user_moonbase)              │   │
│  │  total_btc_mined: 50000                              │   │
│  │  ─────────────────────────────────────────────────── │   │
│  │  GAME-SPECIFIC DATA (money, attachments, stats)      │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## 📋 How They Work Together

### 1. **Metaplex Core Asset (The Actual NFT)**

This is the **real NFT** that:
- ✅ Shows up in wallets (Phantom, Backpack, etc.)
- ✅ Can be traded on marketplaces (Magic Eden, Tensor)
- ✅ Has standard NFT properties (name, image, collection)
- ✅ Stores **ownership** (who owns it right now)

**Account Structure:**
```rust
// Simplified Metaplex Core Asset structure
pub struct BaseAssetV1 {
    pub key: Key,              // Account type discriminator
    pub owner: Pubkey,         // ← WHO OWNS THE NFT
    pub update_authority: UpdateAuthority,
    pub name: String,          // "MoonDoge #42"
    pub uri: String,           // "https://arweave.net/..."
    pub seq: Option<u64>,
    // ... more fields
}
```

**Created by:** Metaplex Core program (`CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d`)

### 2. **Our Metadata Account (Game-Specific State)**

This is **our custom data** that:
- ❌ Does NOT show up in regular wallets
- ❌ Cannot be traded separately
- ✅ Stores game attributes (money, power, DNA)
- ✅ Stores game state (attached moonbase, incubation)

**Account Structure:**
```rust
pub struct MoonDogeMetadata {
    pub mint: Pubkey,              // ← LINKS TO METAPLEX ASSET
    pub money: u64,                // Game attribute
    pub attached_moonbase: Option<Pubkey>,  // Game state
    pub total_btc_mined: u64,      // Game stat
    // ... more game-specific fields
}
```

**Created by:** Our NFT Launchpad program

---

## 🔗 How They Connect

### The Mint Address is the Link

```
User's Wallet:
└── Metaplex Core Asset
    ├── Address (mint): 0xABC123...
    ├── Owner: user_wallet
    └── URI: "https://..."

Our Program:
└── MoonDogeMetadata (PDA)
    ├── Derived from: ["moondoge-metadata", 0xABC123...]
    ├── mint: 0xABC123...  ← SAME ADDRESS!
    └── money: 1000
```

**The connection:**
1. Metaplex asset has an address (its "mint")
2. Our metadata PDA is derived from that mint
3. We store the mint in our metadata to link back
4. When checking ownership, we read from Metaplex asset

---

## 🏗️ How NFTs Are Actually Minted

### Current Implementation (Placeholder)

```rust
// In mint_nfts_for_moonbase or purchase_moondoge:

// 1. Get mint address (passed in or generated)
let mint = ctx.accounts.moondoge_mint.key();

// 2. Create our metadata account
let doge_metadata = &mut ctx.accounts.moondoge_metadata;
doge_metadata.mint = mint;
doge_metadata.money = 0;
// ...

// 3. TODO: Create actual Metaplex Core asset via CPI
// THIS IS THE MISSING PIECE!
```

**What's missing:** We're not actually creating the Metaplex Core asset yet!

### Production Implementation (What You Need to Add)

```rust
use mpl_core::{
    instructions::CreateV1Builder,
    types::{DataState, Plugin, PluginAuthority},
};

// In mint_nfts_for_moonbase:

// 1. Generate metadata
let index = global_config.total_moondoges_minted;
let name = generate_moondoge_name(index);
let uri = global_config.get_random_moondoge_uri(Clock::get()?.slot, index)?;

// 2. Create Metaplex Core Asset via CPI
let create_asset_ix = CreateV1Builder::new()
    .asset(ctx.accounts.moondoge_mint.key())       // The NFT mint
    .collection(Some(global_config.moondoge_collection)) // Collection
    .authority(Some(ctx.accounts.authority.key()))  // Update authority
    .payer(ctx.accounts.user.key())                 // Who pays
    .owner(Some(ctx.accounts.user.key()))           // Initial owner
    .name(name.clone())                             // NFT name
    .uri(uri.clone())                               // Metadata URI
    .plugins(vec![])                                // Optional plugins
    .instruction();

// 3. Execute CPI to Metaplex Core program
solana_program::program::invoke(
    &create_asset_ix,
    &[
        ctx.accounts.moondoge_mint.to_account_info(),
        ctx.accounts.moondoge_collection.to_account_info(),
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.user.to_account_info(),
        ctx.accounts.system_program.to_account_info(),
    ],
)?;

// 4. Create our metadata account
let doge_metadata = &mut ctx.accounts.moondoge_metadata;
doge_metadata.mint = ctx.accounts.moondoge_mint.key();
doge_metadata.money = BASE_DOGE_MONEY;
// ...

msg!("✅ Created Metaplex Core asset: {}", ctx.accounts.moondoge_mint.key());
msg!("✅ Created game metadata: {}", ctx.accounts.moondoge_metadata.key());
```

---

## 🔄 The Complete Flow

### Minting Flow

```
1. User calls mint_nfts_for_moonbase(pricing_tier)
   ↓
2. Program calculates: mint doge? mint egg?
   ↓
3. FOR EACH NFT TO MINT:
   
   a) Generate metadata (name, URI from pool)
      ↓
   b) CPI to Metaplex Core: create asset
      • Creates Metaplex Core asset account
      • Sets owner = user
      • Sets name, URI, collection
      ↓
   c) Create our metadata PDA
      • Derived from mint address
      • Stores game attributes (money, power)
      • Links to asset via mint field
      ↓
   d) Emit events
   
4. Return to user
```

### Trading Flow (Why Our Fix Matters)

```
1. Alice owns MoonDoge NFT
   • Metaplex asset owner = Alice ✅
   • Our metadata mint = asset_address ✅
   ↓
2. Alice sells to Bob on Magic Eden
   • Magic Eden calls Metaplex transfer
   • Metaplex asset owner = Bob ✅
   • Our metadata unchanged (doesn't store owner!)
   ↓
3. Bob calls attach_moondoge()
   • We check Metaplex asset owner ✅
   • Metaplex says: owner = Bob
   • Bob is signer, check passes ✅
   • Attachment proceeds!
```

---

## 📦 Account Relationships

### When Attaching MoonDoge

```rust
#[derive(Accounts)]
pub struct AttachMoonDoge<'info> {
    /// The actual NFT (Metaplex Core)
    /// CHECK: We verify ownership from this account
    pub moondoge_asset: UncheckedAccount<'info>,
    
    /// Our game metadata (linked to asset via mint)
    #[account(
        mut,
        seeds = [MOONDOGE_METADATA_SEED, moondoge_metadata.mint.as_ref()],
        bump = moondoge_metadata.bump,
        constraint = moondoge_metadata.mint == moondoge_asset.key()  // ← LINK!
    )]
    pub moondoge_metadata: Account<'info, MoonDogeMetadata>,
    
    // ...
}
```

**The constraint ensures:**
- The metadata we're using actually corresponds to the NFT
- Can't use Alice's metadata with Bob's NFT
- Prevents account substitution attacks

---

## 🎯 Why This Design?

### Separation of Concerns

**Metaplex Core handles:**
- ✅ NFT ownership (wallets, marketplaces)
- ✅ Standard metadata (name, image, collection)
- ✅ Transfer, trading, burning

**Our metadata handles:**
- ✅ Game attributes (money, power, DNA)
- ✅ Game state (attachments, incubation)
- ✅ Game stats (BTC mined, hashpower)

### Benefits

1. **Composability**: NFT works with all Solana tools
2. **Flexibility**: We can update game data without touching NFT
3. **Marketplace Compatible**: Standard NFT, trades anywhere
4. **Storage Efficiency**: Only game-specific data in our accounts
5. **Separation**: Core NFT and game logic are decoupled

---

## 🔧 What You Need to Add

### 1. **Install Metaplex Core SDK**

```toml
# In Cargo.toml
[dependencies]
mpl-core = { version = "0.7", features = ["cpi"] }
```

### 2. **Import Metaplex Types**

```rust
use mpl_core::{
    instructions::{CreateV1Builder, TransferV1Builder},
    types::{DataState, Plugin, PluginAuthority, UpdateAuthority},
    ID as MPL_CORE_PROGRAM_ID,
};
```

### 3. **Update Account Contexts**

```rust
#[derive(Accounts)]
pub struct PurchaseMoonDoge<'info> {
    // ... existing accounts ...
    
    /// CHECK: Metaplex Core program
    #[account(address = MPL_CORE_PROGRAM_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
    
    /// CHECK: MoonDoge collection (Metaplex Core)
    pub moondoge_collection: UncheckedAccount<'info>,
}
```

### 4. **Implement Asset Creation**

See the production implementation example above.

---

## 📝 Summary

### The Key Points

1. **Metaplex Core Asset = The NFT**
   - Actual NFT that shows in wallets
   - Stores ownership (source of truth)
   - Tradeable on marketplaces

2. **Our Metadata = Game Data**
   - Separate PDA account
   - Game-specific attributes
   - Linked via mint address

3. **Connection**
   - Mint address links them
   - We read ownership from Metaplex
   - We store game data in our PDA

4. **Current Status**
   - ✅ Metadata accounts work
   - ✅ Ownership verification works
   - ❌ Actual Metaplex asset creation pending
   - 📝 Need to add Metaplex Core CPI

### Next Steps

1. Add `mpl-core` dependency
2. Implement `CreateV1Builder` CPI calls
3. Update account contexts with collection
4. Test full minting flow
5. Verify NFTs show in wallets

---

## 🚀 When Fully Implemented

```
User mints MoonDoge NFT
   ↓
Metaplex Core creates NFT
   ↓
NFT shows in Phantom wallet ✅
   ↓
Our program creates metadata PDA
   ↓
User can attach to moonbase ✅
   ↓
User can trade NFT on Magic Eden ✅
   ↓
New owner can use NFT in game ✅
```

**This is the correct architecture for Solana NFT games!** 🎮

