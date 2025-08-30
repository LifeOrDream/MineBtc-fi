# DogeTech Production Deployment System

## 🚀 Overview

This directory contains the production-grade deployment system for **DogeTech**, a comprehensive Solana-based lunar mining game with MoonDoge token economy. The system implements a professional deployment flow with proper error handling, state management, and recovery mechanisms.

## 📁 Project Structure

```
setup_scripts/
├── config.json                    # Master configuration file
├── helper.js                      # Core helper functions
├── init_mdoge_token.js            # Token deployment script
├── init_mdoge_SOL_pool.js         # Pool creation script
├── init_moonBase.js               # MoonBase program initialization
├── deployments/                   # Deployment state files
│   ├── devnet.json               # Devnet deployment state
│   ├── testnet.json              # Testnet deployment state
│   └── mainnet.json              # Mainnet deployment state
└── README.md                      # This documentation
```

## ⚙️ Configuration System

The `config.json` file contains all deployment parameters:

### Network Configuration
- **cluster**: Target network (devnet/testnet/mainnet)
- **rpc_url**: RPC endpoint URL
- **commitment**: Transaction commitment level

### Token Configuration
- **name**: Token name (MoonDoge)
- **symbol**: Token symbol (MDOGE)
- **initial_supply**: Total token supply (21B)
- **burn_tax_bps**: Burn tax in basis points
- **decimals**: Token decimal places

### Game Configuration
- **factions**: Player faction definitions
- **modules**: Game module configurations
- **expansions**: Base expansion options
- **pvp**: PvP system parameters

## 🔧 Dependencies

```bash
npm install @solana/web3.js @solana/spl-token @coral-xyz/anchor @metaplex-foundation/mpl-token-metadata
```

## 🎯 Deployment Flow

### Phase 1: Token Deployment

```bash
node init_mdoge_token.js
```

**What it does:**
- Creates mDOGE token mint with Token 2022 extensions
- Implements burn tax mechanism (1% configurable)
- Mints initial supply (21 billion tokens)
- Creates metadata with Metaplex standards
- Saves deployment state to `deployments/{network}.json`

**Key Features:**
- ✅ Production-grade error handling
- ✅ Automatic airdrop for devnet
- ✅ State persistence and recovery
- ✅ Comprehensive logging
- ✅ Token 2022 compatibility

### Phase 2: Pool Creation

```bash
node init_mdoge_SOL_pool.js
```

**What it does:**
- Sets up Raydium CP-Swap pool configuration
- Configures trading fees and parameters
- Prepares liquidity provision
- Integrates with Raydium infrastructure

**Note:** This script prepares the configuration for Raydium integration. Actual pool creation requires coordination with the Raydium team for production deployment.

### Phase 3: MoonBase Initialization

```bash
node init_moonBase.js
```

**What it does:**
- Initializes the MoonBase Solana program
- Sets up mining system with token vaults
- Configures referral and rewards systems
- Adds factions, expansions, and modules
- Initializes PvP matchmaker
- Deposits mining tokens

**Components Initialized:**
1. **Core Program**: Global configuration and authorities
2. **Mining System**: Token vaults and reward distribution
3. **Referral System**: Multi-level referral tracking
4. **Config Stores**: Module and game configurations
5. **Loot & Stats**: Reward pools and level tracking
6. **PvP System**: Matchmaker and combat mechanics
7. **Factions**: Player faction system (4 factions)
8. **Expansions**: Base expansion options (4 tiers)
9. **Modules**: Game modules with stats and upgrades
10. **Token Deposits**: Initial mining token allocation

## 📊 State Management

### Deployment State Tracking

Each deployment creates a state file with:

```json
{
  "network": "devnet",
  "lastUpdated": "2024-01-01T00:00:00.000Z",
  "version": "2.0.0",
  "mdoge_mint_address": "...",
  "moonbase_program_initialized": {
    "globalConfig_address": "...",
    "moonDogeMining_address": "...",
    "timestamp": "..."
  },
  "mining_vault_initialized": { ... },
  "factions_added": { ... },
  "modules_added": { ... }
}
```

### Recovery Mechanisms

- **Idempotent Operations**: Scripts can be safely re-run
- **State Validation**: Prerequisites are checked before each step
- **Error Recovery**: Graceful handling of partial deployments
- **Progress Tracking**: Visual progress indicators

## 🎮 Game Features

### Factions System
1. **Luna Engineers** - Master builders of infrastructure
2. **Solar Miners** - Elite cryptocurrency miners
3. **Cosmic Traders** - Interplanetary commerce specialists
4. **Orbital Guards** - Defenders of lunar territories

### Module Types
- **Mining Modules**: Generate mDOGE tokens
- **Attraction Modules**: Provide XP over time
- **Research Modules**: Casino-style reward mechanics
- **Attack Modules**: PvP combat capabilities

### PvP System
- **Turn-based Combat**: 15-turn limit with timeouts
- **Resource Stealing**: XP, hashpower, and loot theft
- **Ticket Tiers**: 5 tiers from micro to kraken stakes
- **Damage Mechanics**: HP-based with special effects

## 🔐 Security Features

### Economic Security
- **Burn Tax**: Deflationary tokenomics
- **Controlled Supply**: Fixed 21B token cap
- **Theft Limits**: Anti-exploitation caps on PvP
- **Time Locks**: Cooldowns prevent abuse

### Technical Security
- **Program Authorities**: Multi-signature support
- **Account Validation**: Comprehensive checks
- **Overflow Protection**: SafeMath operations
- **Access Controls**: Role-based permissions

## 🌐 Network Support

### Devnet
- **Purpose**: Development and testing
- **Features**: Automatic airdrops, relaxed constraints
- **RPC**: `https://api.devnet.solana.com`

### Testnet
- **Purpose**: Final testing before mainnet
- **Features**: Mainnet-like conditions
- **RPC**: `https://api.testnet.solana.com`

### Mainnet
- **Purpose**: Production deployment
- **Features**: Full security, real tokens
- **RPC**: `https://api.mainnet-beta.solana.com`

## 🛠️ Troubleshooting

### Common Issues

#### 1. Insufficient Balance
```bash
Error: Insufficient SOL balance for deployment
```
**Solution**: Fund the deployer account or request airdrop on devnet

#### 2. Program Not Found
```bash
Error: MoonBase program ID not found
```
**Solution**: Deploy the Solana programs first using `anchor deploy`

#### 3. Token Account Errors
```bash
Error: Token account creation failed
```
**Solution**: Check token mint exists and retry the operation

#### 4. Network Connection Issues
```bash
Error: Failed to connect after multiple attempts
```
**Solution**: Check RPC URL and network connectivity

### Debug Mode

Enable verbose logging:
```bash
DEBUG=1 node init_moonBase.js
```

### State Reset

To reset deployment state:
```bash
rm deployments/{network}.json
```

## 📈 Monitoring

### Transaction Tracking
- All transactions are logged with signatures
- Explorer URLs provided for verification
- Retry mechanisms for failed transactions

### State Validation
- Prerequisites checked before each operation
- Component status tracked individually
- Completion percentage calculated

### Error Reporting
- Detailed error messages with context
- Recovery suggestions provided
- State preserved for debugging

## 🚀 Production Checklist

### Pre-Deployment
- [ ] Programs deployed and verified
- [ ] Configuration reviewed and approved
- [ ] Deployer account funded sufficiently
- [ ] Network connectivity confirmed
- [ ] Backup procedures in place

### During Deployment
- [ ] Monitor transaction confirmations
- [ ] Verify state file updates
- [ ] Check explorer for transaction success
- [ ] Document any manual interventions

### Post-Deployment
- [ ] Verify all components initialized
- [ ] Test basic functionality
- [ ] Archive deployment artifacts
- [ ] Document lessons learned
- [ ] Plan monitoring strategy

## 📞 Support

### Documentation
- **IDL Files**: `../prod_moonbase/target/idl/`
- **Program Source**: `../prod_moonbase/programs/`
- **Frontend**: `../websiteApp/`

### Deployment Artifacts
- **State Files**: `./deployments/`
- **Keypairs**: `./deployer-keypair.json`
- **Logs**: Console output with timestamps

### Contact
For deployment issues or questions:
1. Check this documentation
2. Review error logs and state files
3. Verify network status and RPC health
4. Contact development team with specific error details

---

**DogeTech Team** | *Building the future of lunar gaming on Solana* 🌙🚀 

anchor upgrade target/deploy/moon_base.so --program-id 3tXgyDmYHrZjipR6SBYsJrvBCxoZ3QKZ4AkJbhAzj3bR --provider.cluster localnet