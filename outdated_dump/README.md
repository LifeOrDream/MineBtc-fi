dragonhive_nfts contracts
cargo update -p base64ct --precise 1.7.1







# MoonDoge Facility - Solana Game

A Solana-based mining game where players build moon bases, install modules, deploy mining gear, and earn MoonDoge tokens through gameplay.

## Overview

MoonDoge Facility is a play-to-earn blockchain game built on the Solana network. Players can:

- Create personal moon bases
- Install and upgrade modules (rooms)
- Deploy mining gear to generate hashpower
- Lock NFTs to boost mining capabilities
- Earn MoonDoge tokens through mining operations
- Participate in a referral system for additional rewards

The game implements a dynamic mining economy with price-responsive distribution adjustments and Protocol Owned Liquidity (POL) mechanisms, creating a self-balancing tokenomics model.

## Program Structure

The Solana program is organized into several components:

- **MoonBase**: The player's central facility with faction alignment and XP tracking
- **Modules**: Rooms that provide space for mining equipment (award XP when installed/upgraded)
- **Gears**: Mining equipment that generates hashpower
- **Mining System**: Rewards distribution based on contribution (awards XP for mining)
- **NFT Integration**: Lock Doge NFTs for mining boosts (awards XP)
- **Referral System**: Earn rewards for bringing new players (awards XP)
- **Faction System**: Player alignment system for future competitive features
- **XP & Level System**: Comprehensive progression system with automatic level ups

## Game Mechanics

### Resources

- **Hashpower**: Determines mining rewards
- **Electricity**: Limits how much gear can be deployed
- **MoonDoge Tokens**: Earned through mining operations

### Player Factions

The game supports a faction system that allows players to align with different groups:

- **Faction Selection**: Players must choose a faction when creating their moonbase
- **Supported Factions**: Configurable list of factions (e.g., "USA", "China", "Russia", "Japan", "Germany")
- **Faction Limits**: Maximum of 10 factions supported, each with names up to 16 characters
- **Permanent Assignment**: Once a player selects a faction, it cannot be changed
- **Admin Management**: Only program authorities can add new factions to the system

#### Faction Administration

- **Adding Factions**: Use `add_faction` instruction (admin only)
- **Validation**: Faction names are case-insensitive and must be unique
- **No Removal**: Factions cannot be removed once added to maintain data integrity
- **Future Features**: Faction system provides foundation for faction-based competitions, rewards, and gameplay mechanics

### XP and Level System

The game features a comprehensive experience point (XP) and leveling system that rewards player engagement and progression:

#### **XP Sources and Rewards**

| Action                  | XP Gained | Frequency   |
| ----------------------- | --------- | ----------- |
| Daily login             | +10 XP    | Daily       |
| Installing a new module | +50 XP    | Per install |
| Upgrading a module      | +30 XP    | Per upgrade |
| Locking a mDOGE NFT     | +20 XP    | Per lock    |
| Mining 1000 mDOGE       | +15 XP    | Auto-scaled |
| Referring 1 user        | +100 XP   | One-time    |

#### **Level Progression**

Players advance through levels using a progressive XP formula:

- **Formula**: `required_xp = 100 + (level² × 20)`
- **Automatic Level Up**: Players level up automatically when XP threshold is reached

| Level | XP Required | Total XP |
| ----- | ----------- | -------- |
| 1     | 100         | 100      |
| 2     | 180         | 280      |
| 3     | 360         | 640      |
| 4     | 580         | 1,220    |
| 5     | 840         | 2,060    |

#### **Player Progression Tracking**

Each player's moonbase tracks:

- **Current Level**: Player's progression level (starts at 0)
- **Total XP**: Accumulated experience points
- **Daily Login Streak**: Consecutive daily logins (resets after 48h gap)
- **Last Login**: Timestamp for daily reward eligibility

Each player's referral rewards account tracks:

- **Referral Count**: Number of users successfully referred
- **Total SOL Earned**: Accumulated referral rewards

#### **Daily Login System**

- **Streak Tracking**: Maintains consecutive login count
- **Reward Timing**: Must wait 24 hours between login rewards
- **Streak Reset**: Breaks if gap exceeds 48 hours
- **XP Reward**: 10 XP per daily login

#### **Referral System Integration**

- **XP Calculation**: Advanced formula that rewards both quantity and quality of referrals
  - **Base XP**: 100 XP per successful referral
  - **Bonus XP**: Additional XP based on total SOL earned from referrals using `√(total_sol_earned_lamports) ÷ 1000`
  - **Formula**: `(referrals_count × 100) + (√(sol_earned_lamports) ÷ 1000)`
- **Quality Incentive**: Encourages referring active users who contribute more to the ecosystem
- **Count Tracking**: Both referrer and referee accounts track referral statistics
- **Automatic Processing**: XP and counts updated automatically during moonbase creation

**Example XP Calculations:**

- 1 referral, 0 SOL earned: 100 XP
- 1 referral, 1 SOL earned: 131 XP (100 base + 31 bonus)
- 10 referrals, 10 SOL earned: 1,100 XP (1000 base + 100 bonus)
- 50 referrals, 200 SOL earned: 5,447 XP (5000 base + 447 bonus)

### Mining & Rewards

- Mining rewards are proportional to a player's hashpower contribution
- Dynamic distribution system adjusts rewards based on market conditions
- Rewards are calculated per slot with real-time distribution

### Dynamic Distribution System

The game implements a sophisticated price-responsive token distribution mechanism that replaces traditional halvening:

#### **8-Hour Price Oracle Cycles**

- Every hour, 50% of the hourly mDOGE distribution is swapped for SOL via Raydium
- Price data is collected and stored for 8-hour rolling averages
- After 8 hours of data collection, the system processes accumulated liquidity

#### **Rate Adjustments**

- **Price Increase**: Distribution rate increases by 1%
- **Price Decrease**: Distribution rate decreases by 3%
- **No Change**: Rate remains constant

#### **Protocol Owned Liquidity (POL)**

- SOL from swaps accumulates over the 8-hour period
- At cycle completion, accumulated SOL + calculated mDOGE are added to the Raydium LP
- LP tokens are immediately burned, permanently removing liquidity
- Creates deflationary pressure while supporting price stability

#### **Configurable Parameters**

- `slots_for_swap`: Configurable timing parameter (default: 9000 slots)
- Swap percentage: 50% of hourly distribution
- Price history: 8-hour rolling window
- LP calculation: `slots_for_swap × 8 ÷ 2 × current_dist_rate` mDOGE

This system ensures organic growth adjustments based on market conditions while building permanent liquidity support.

### Upgrades

- Modules can be upgraded to provide more space
- Gears can be upgraded to provide more hashpower

## Building & Deployment

### Prerequisites

- [Solana CLI Tools](https://docs.solana.com/cli/install-solana-cli-tools)
- [Anchor Framework](https://project-serum.github.io/anchor/getting-started/installation.html) v0.29.0 or higher
- [Node.js &amp; npm](https://nodejs.org/)
- [Rust](https://www.rust-lang.org/tools/install) with the Solana BPF target

### Building the Program

This will:

1. Navigate to the program directory
2. Build the program using Anchor
3. Copy the compiled binary to the deployment directory

You can also build manually:

```bash
# Navigate to the program directory
cd prod_moonbase

# Build the program
anchor build

# Get the program ID
solana address -k target/deploy/moon_base-keypair.json
```

### Deployment

Deploy the program using the provided script:

```bash
# anchor deploy --provider.cluster devnet
```

or

```bash
solana program deploy ./target/deploy/moon_base.so  --program-id ./target/deploy/moon_base-keypair.json --keypair ../wallet-keypair.json  

solana program deploy ./target/deploy/moon_economy.so  --program-id ./target/deploy/moon_economy-keypair.json --keypair ../wallet-keypair.json  

solana program deploy ./target/deploy/raydium_cp_swap.so  --program-id ./target/deploy/raydium_cp_swap-keypair.json --keypair ../wallet-keypair.json  
```

Yes, when you make changes to your Solana program and rebuild it, you'll need to handle the program ID properly. Here's the process:

1. Generate New Program ID (if you haven't already):

```bash
   solana-keygen new -o target/deploy/moon_base-keypair.json  --force

   solana-keygen new -o target/deploy/moon_economy-keypair.json  --force

   # localnet / devnet only
   solana-keygen new -o target/deploy/raydium_cp_swap-keypair.json  --force
   
```

2. Get the Program ID:

```bash
   solana-keygen pubkey target/deploy/moon_base-keypair.json
```

3. Update Program ID in following places:
   - In your Anchor.toml
   - In your program's lib.rs:

### Configuration

The program uses several configuration files:

- `Anchor.toml`: Program and deployment configuration

### Initializing the Moon-base program

To initialize the program, urn the following scropts

```
cd prod_moonbase/setup_scripts
node init_mdoge_token.js
node init_testLP_token.js
node init_moonbase.js
```

Start a solana local validator -

```
solana-test-validator
```

## Program Interface

### Admin Functions

- Initialize program
- Manage global configurations
- Create/update module and gear configurations
- Manage mining parameters
- Withdraw collected fees
- Configure Raydium pool for dynamic distribution
- Update slots per hour timing parameters
- **Faction Management**: Add new factions to the supported factions list

### User Functions

- Create moon bases (with faction selection)
- Install and upgrade modules (awards XP)
- Create and deploy mining gear
- Manage NFT locking (awards XP)
- Claim mining rewards (awards XP based on amount)
- Claim referral rewards
- **Daily login** (awards XP and maintains streaks)
- **Level progression** (automatic level up when XP thresholds are met)

## Development

### Adding New Module Types

1. Use `add_module_to_base` instruction
2. Provide configuration parameters
3. Set price, tiles, and upgrade parameters

### Adding New Gear Types

1. Use `initialize_gear` instruction
2. Configure hashpower, electricity usage
3. Set compatible module types

### **Loot Rewards System**

The game features an automated loot rewards system that accumulates tokens and SOL for future distribution:

#### **Automatic Accumulation**

- **mDOGE Loot**: 10% of all mining rewards are automatically transferred to the loot vault
- **SOL Loot**: 10% of all SOL collections (moonbase creation, module costs, etc.) go to the loot vault
- **Transparent Tracking**: All accumulations are tracked and logged via events

#### **Loot Vault Infrastructure**

- **Dedicated Vaults**: Separate secure vaults for mDOGE tokens and SOL
- **PDA Security**: All vaults use Program Derived Addresses for maximum security
- **Admin Management**: Only program authorities can manage loot distribution
- **Event Logging**: Comprehensive event system tracks all loot activities

#### **Future Loot Mechanics**

- **Loot Drops**: Foundation for implementing random loot drop events
- **Special Rewards**: Accumulated tokens can be distributed for achievements, competitions, or special events
- **Community Events**: Large pool of rewards available for community-driven activities

## Events & Monitoring

The program emits comprehensive events for tracking player progression and system state:

### XP & Level Events

- **`XpGained`**: Emitted when players earn XP from any action
- **`LevelUp`**: Emitted when players automatically level up
- **`DailyLoginReward`**: Emitted for daily login streaks and XP rewards

### Faction Events

- **`FactionAdded`**: Emitted when admins add new factions

### Loot Rewards Events

- **`LootRewardsAccumulated`**: Emitted when tokens/SOL are added to loot vaults
- **`LootRewardsDistributed`**: Emitted when loot rewards are distributed to players
- **`LootRewardsInitialized`**: Emitted when the loot system is first set up

### Mining & Gameplay Events

- Standard mining, module, gear, and NFT events
- Referral system events with XP tracking

## Security Considerations

- Proper account validation using PDAs
- Authority checks for administrative functions
- Overflow protection in all calculations
- Resource limit enforcement
- **XP System Integrity**: Automatic XP calculations prevent manipulation
- **Level Progression**: Server-side validation ensures fair progression

## License

[MIT License](LICENSE)

## Support

For support and inquiries, please open an issue on this repository or contact the development team.

anchor upgrade target/deploy/moon_base.so --program-id 3VWMZMjJZm5jjwWUZM1i8JPGYRMVtFuJTc9SUasyDVSB --provider.cluster localnet
