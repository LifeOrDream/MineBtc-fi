# DragonHive NFTs Program

The **DragonHive NFTs Program** is the foundational admin program for the DragonHive protocol, managing DragonBee NFTs, breeding mechanics, and DRAGON token operations on Solana.

## 🚀 Overview

This program serves as the core infrastructure for:
- **DragonBee NFT Management**: Minting, evolution, and lifecycle management
- **Breeding System**: Complex genetic inheritance with cooldown mechanics
- **DRAGON Token Integration**: 100B token supply management and rewards
- **Queen Breeding Auctions**: Competitive breeding rights system
- **Kill Rewards Pool**: Token distribution for NFT burning

## 🏗️ Architecture

### Core Components

1. **MPL Core Integration**: Uses Metaplex Core for efficient NFT operations
2. **Genetic System**: 256-bit genetic codes with 8 DragonBee types
3. **Breeding Mechanics**: Cooldown periods, trait inheritance, and mutations
4. **Economic Engine**: SOL fee collection, DRAGON token distribution
5. **Query System**: Comprehensive data retrieval for external programs

### Program Structure

```
dragonhive_nfts/
├── src/
│   ├── lib.rs                 # Main program entry point
│   ├── constants.rs           # Program-wide constants
│   ├── errors.rs              # Custom error definitions
│   ├── events.rs              # Event definitions for indexing
│   ├── state.rs               # Account state definitions
│   ├── utils.rs               # Utility functions
│   └── instructions/
│       ├── admin.rs           # Admin-only functions
│       ├── user.rs            # User-facing functions
│       ├── breeding.rs        # Breeding system functions
│       └── queries.rs         # Query functions
├── Cargo.toml                 # Rust dependencies
└── Anchor.toml               # Anchor configuration
```

## 🧬 DragonBee Genetics System

### Genetic Code Structure (256 bits)

```
Bits 0-3:    DragonBee Type (8 types)
Bits 4-6:    Evolution Stage (8 stages)
Bits 7-146:  Appearance Traits (140 bits, 7 groups × 4 traits × 5 bits)
Bits 147-230: Power Traits (84 bits, 7 groups × 3 traits × 4 bits)
Bits 231-255: Reserved (25 bits)
```

### DragonBee Types

1. **Solar** (Fire) - Harness sun power, fiery temperament
2. **Aqua** (Water) - Water manipulation, fluid movements  
3. **Thunder** (Electric) - Generate electricity, lightning strikes
4. **Terra** (Earth) - Earth connection, rock manipulation
5. **Wind** (Air) - Sky mastery, wind manipulation
6. **Venom** (Poison) - Potent toxins, deadly stings
7. **Frost** (Ice) - Ice environments, freezing abilities
8. **Mystic** (Psychic) - Telepathy, mind control

### Evolution Stages

0. **Larva** - Newborn stage
1. **Pupae** - Early development
2. **Worker** - Basic functionality
3. **Soldier** - Combat ready
4. **Elite** - Enhanced abilities
5. **Royal** - Noble status
6. **Queen** - Leadership role
7. **Dragon** - Ultimate form

## 💰 Economic Model

### Fee Distribution

- **NFT Sales**: 1 SOL per DragonBee
  - 30% → Team
  - 70% → DRAGON buyback + kill rewards pool

- **Breeding Fees**: Dynamic based on evolution stages
  - 30% → Team  
  - 70% → DRAGON buyback + kill rewards pool

- **Queen Auctions**: Winner sets breeding price
  - 80% → Queen owner
  - 20% → Protocol fees

### DRAGON Token Integration

- **Total Supply**: 100B HONEY tokens
- **Kill Rewards**: 10% of buybacks go to kill pool
- **Distribution**: Power-based rewards for burning NFTs

## 🔧 Key Functions

### Admin Functions

- `initialize()` - Initialize program with collection and token vaults
- `update_config()` - Update program parameters
- `mint_genesis_dragonbee()` - Mint initial 15,000 NFTs
- `deposit_honey_tokens()` - Add tokens to reward pool
- `set_queen_bee()` - Set up queen breeding auctions

### User Functions

- `create_user_profile()` - Create user tracking account
- `purchase_dragonbee()` - Buy DragonBee NFT (1 SOL)
- `evolve_dragonbee()` - Evolve to next stage
- `breed_dragonbees()` - Breed two DragonBees
- `kill_dragonbee()` - Burn NFT for HONEY tokens
- `update_dragonbee_stats()` - Update power from game interactions

### Breeding System

- `bid_queen_breeding()` - Bid in queen auction
- `finalize_queen_auction()` - Complete auction
- `breed_with_queen()` - Breed with auction winner's queen

### Query Functions

- `get_dragonbee_info()` - Get complete NFT information
- `get_user_dragonbees()` - Get user's collection
- `get_genetic_analysis()` - Analyze genetic traits
- `get_breeding_cooldown()` - Check breeding status
- `get_kill_reward_estimate()` - Estimate burn rewards

## 🎮 Game Integration

The program is designed to integrate with other DragonHive programs:

1. **Moonbase Game**: Updates DragonBee power through gameplay
2. **AMM/Launchpad**: Uses HONEY tokens for operations
3. **Breeding Games**: Alien invasion defense mechanics

### Integration Points

- **Power Updates**: Game programs can increase DragonBee power
- **Token Rewards**: HONEY tokens earned through gameplay
- **Status Tracking**: In-game status prevents other operations

## 🔒 Security Features

### Access Control

- **Admin Authority**: Controlled by program authority
- **Owner Verification**: DragonBees can only be modified by owners
- **PDA Security**: All critical accounts use Program Derived Addresses

### Validation

- **Genetic Integrity**: Validates genetic data consistency
- **Cooldown Enforcement**: Prevents rapid breeding exploitation  
- **Rate Limiting**: Limits operations per user per day
- **Power Caps**: Prevents excessive power increases

## 🚀 Deployment Guide

### Prerequisites

- Rust 1.70+
- Solana CLI 1.16+
- Anchor Framework 0.30+

### Build & Deploy

```bash
# Install dependencies
npm install

# Build the program
anchor build

# Deploy to devnet
anchor deploy --provider.cluster devnet

# Initialize the program
anchor run initialize
```

### Configuration

1. **Create DRAGON Token Mint**: Deploy SPL Token with 100B supply
2. **Initialize Program**: Call `initialize()` with token mint
3. **Mint Genesis NFTs**: Create initial 15,000 DragonBees
4. **Set Up Auctions**: Configure queen breeding system

## 📊 Program Accounts

### Global State

- **GlobalConfig**: Program configuration and statistics
- **UserProfile**: Individual user tracking and DragonBee ownership

### NFT Management  

- **DragonBeeMetadata**: Individual NFT data and genetics
- **QueenBreedingAuction**: Queen breeding auction state
- **BreedingCooldown**: Breeding cooldown tracking

### Economic

- **DRAGON Vault**: Token storage for rewards
- **SOL Treasury**: Fee collection and distribution
- **Kill Rewards Pool**: Accumulated tokens for burning rewards

## 🔮 Future Extensions

The program architecture supports future enhancements:

1. **Marketplace Integration**: Built-in NFT trading
2. **Staking Rewards**: DRAGON token staking for benefits
3. **Cross-Chain Bridge**: Multi-chain DragonBee support
4. **DAO Governance**: Community-driven parameter updates
5. **Advanced Genetics**: More complex trait systems

## 📚 Integration Examples

### Purchase DragonBee

```typescript
await program.methods
  .purchaseDragonbee()
  .accounts({
    globalConfig: globalConfigPda,
    userProfile: userProfilePda,
    dragonbeeMetadata: dragonbeeMetadataPda,
    dragonbeeMint: newKeypair.publicKey,
    collectionMint: collectionMint,
    solTreasury: solTreasuryPda,
    buyer: wallet.publicKey,
    mplCoreProgram: MPL_CORE_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
  })
  .signers([newKeypair])
  .rpc();
```

### Breed DragonBees

```typescript
await program.methods
  .breedDragonbees(parent1Mint, parent2Mint)
  .accounts({
    globalConfig: globalConfigPda,
    userProfile: userProfilePda,
    parent1Metadata: parent1MetadataPda,
    parent2Metadata: parent2MetadataPda,
    offspringMetadata: offspringMetadataPda,
    offspringMint: offspringKeypair.publicKey,
    // ... other accounts
  })
  .signers([offspringKeypair])
  .rpc();
```

## 🤝 Contributing

The DragonHive NFTs program is designed for extensibility. Key areas for contribution:

1. **Genetic Algorithms**: Enhanced breeding mechanics
2. **Economic Models**: Advanced tokenomics features  
3. **Game Integration**: New interaction patterns
4. **Security Audits**: Code review and testing
5. **Documentation**: Usage examples and guides

## 📄 License

This program is part of the DragonHive protocol. See license terms for usage rights.

---

**Built with ❤️ for the Solana ecosystem**
