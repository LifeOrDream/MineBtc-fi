# MineBTC Solana Program - AI Assistant Guide

This document helps AI assistants understand the MineBTC Anchor program for making code changes effectively.

## Codebase Overview

Anchor-based Solana program implementing a faction-based raffle game with staking, NFTs, and deflationary tokenomics.

**Key Technologies:**
- Anchor Framework (Solana smart contracts)
- Token-2022 (transfer fees)
- Metaplex Core (NFT doges)
- Raydium CP-Swap (liquidity pool integration)

## Architecture

```
GlobalConfig (singleton)
    |
    +-- FactionState (x12) -- Per-faction staking pools and reward indexes
    |
    +-- GameSession (per round) -- Round bets, winning info
    |
    +-- MineBtcMining -- Emissions, price history, POL state
    |
    +-- TaxConfig -- Transfer tax distribution settings
    |
    +-- DogeConfig -- NFT bonding curve parameters

PlayerData (per user)
    |
    +-- FactionStakeInfo (per faction) -- User's stakes in each faction
    |
    +-- AutominerVault -- User's autominer configuration
    |
    +-- BtcDoge (NFTs) -- Owned/staked doges
```

## Critical Files

| File | Purpose | Lines | Modify When |
|------|---------|-------|-------------|
| `instructions/game.rs` | Round lifecycle | ~1200 | Round start/end logic, reward distribution |
| `instructions/user.rs` | Player actions | ~1800 | Betting, claiming, autominer |
| `instructions/stake.rs` | Staking system | ~1500 | Staking/unstaking, reward indexes |
| `instructions/doges.rs` | NFT system | ~800 | Doge minting, mutations, evolution |
| `instructions/economy.rs` | Price/emissions | ~600 | Snapshots, rate updates, POL |
| `instructions/tax.rs` | Transfer tax | ~400 | Tax distribution logic |
| `state.rs` | All accounts | ~800 | Adding new fields to accounts |
| `events.rs` | Event definitions | ~400 | Adding new events for backend |
| `errors.rs` | Custom errors | ~100 | Adding new error types |

## Directory Structure

```
programs/mineBTC/src/
├── lib.rs                    # Entry point, instruction routing
├── state.rs                  # Account state structs
├── events.rs                 # Event definitions for backend indexing
├── errors.rs                 # Custom error types
├── genescience.rs            # Doge DNA/mutation algorithms
├── mpl_core_helpers.rs       # Metaplex Core integration
└── instructions/
    ├── mod.rs                # Module exports
    ├── game.rs               # Round lifecycle (MOST IMPORTANT)
    ├── user.rs               # Player betting/claiming
    ├── stake.rs              # Staking mechanics
    ├── doges.rs              # NFT doge system
    ├── economy.rs            # Price snapshots, emissions
    ├── tax.rs                # Transfer tax handling
    ├── admin.rs              # Admin/config functions
    └── helper.rs             # Shared utilities
```

## Key Concepts

### Round Lifecycle (game.rs)

1. **`int_start_round`**: Cranker starts new round
   - Generates block assignments (24 blocks -> 12 factions, 2 each)
   - Creates commit hash for VRF
   - Emits `RoundStarted` event

2. **`int_end_round`**: Cranker ends round after 60s + minimum bettors
   - Reveals VRF to determine winning block
   - Identifies winning faction (2 blocks win)
   - Emits `RoundEnded` event

3. **`int_end_round_faction_rewards`**: Distribute rewards per faction
   - Called once per faction (12 calls total)
   - Updates reward indexes for proportional distribution
   - Handles motherlode jackpot (0.16% chance)
   - Emits `RewardsDistributedForFaction` event

### Reward Index Pattern (stake.rs)

Rewards use a "global index" pattern for gas-efficient proportional distribution:

```rust
// When distributing rewards:
faction.sol_reward_index += rewards * PRECISION / total_hashpower;

// When claiming:
pending = (current_index - user.last_index) * user_hashpower / PRECISION;
user.last_index = current_index;
```

This allows O(1) reward distribution regardless of user count.

### Staking Mechanics (stake.rs)

- **MineBTC Staking**: `stake_minebtc` / `unstake_minebtc`
  - Lockup multipliers: 30d=1x, 90d=2.5x, 180d=5x, 1y=9x, 3y=15x
  - Emergency unstake: 15% penalty

- **LP Staking**: `stake_lp` / `unstake_lp`
  - Same lockup multipliers
  - Higher APR than token staking

- **Hashpower Calculation**:
```rust
hashpower = amount * lockup_multiplier * doge_multiplier;
```

### Doge NFT System (doges.rs)

- **Minting**: `batch_mint_doges` - Bonding curve pricing
- **Staking**: `stake_doge` / `unstake_doge` - Boost hashpower
- **Mutations**: `trigger_mutation` - Random stat changes
- **Evolution**: `trigger_evolution` - Stage progression

Doge multipliers (staked count):
- 1: 1.3x, 2: 1.6x, 3: 2.0x, 4: 2.5x, 5: 3.0x

### Economy System (economy.rs)

- **Price Snapshots**: `snapshot_price` - Every 30 minutes via Raydium swap
- **Rate Updates**: `update_rate` - After 8 snapshots (4 hours)
  - Price down >3%: Reduce emissions 10%
  - Price up >3%: Increase emissions 10%
- **POL Addition**: `add_lp_and_burn` - Add liquidity, burn LP tokens

### Transfer Tax (tax.rs)

Token-2022 transfer hook distributes 1% tax:
- 50% burned
- 25% NFT floor sweep vault
- 25% Faction treasury (weekly distribution)

## Common Tasks

### Adding a New Event

1. Define in `events.rs`:
```rust
#[event]
pub struct NewEvent {
    pub field1: u64,
    pub field2: Pubkey,
}
```

2. Emit in instruction:
```rust
emit!(NewEvent {
    field1: value,
    field2: pubkey,
});
```

3. Add handler in backend `processEvents.ts`

### Adding a New Account Field

1. Add to struct in `state.rs`:
```rust
pub struct GlobalConfig {
    // ... existing fields
    pub new_field: u64,  // Add at END to avoid migration
}
```

2. Update space calculation if needed
3. Initialize in relevant instruction

### Adding a New Instruction

1. Create function in appropriate `instructions/*.rs`:
```rust
pub fn int_new_instruction(ctx: Context<NewInstruction>, args: Args) -> Result<()> {
    // Implementation
    Ok(())
}
```

2. Create context struct:
```rust
#[derive(Accounts)]
pub struct NewInstruction<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    // ... accounts
}
```

3. Add to `lib.rs` instruction enum:
```rust
pub fn new_instruction(ctx: Context<NewInstruction>, args: Args) -> Result<()> {
    instructions::int_new_instruction(ctx, args)
}
```

4. Export from `instructions/mod.rs`

## Key Account PDAs

**IMPORTANT**: These are the EXACT seeds from `state.rs`. Use these when deriving PDAs.

### Global State

```rust
// GlobalConfig - singleton
seeds = [b"global-config"]

// HashpowerConfig - singleton
seeds = [b"hashpower-config"]

// MineBtcMining - singleton
seeds = [b"mine-btc-mining"]

// GlobalGameState - singleton
seeds = [b"global-game-state"]

// DogeConfig - singleton
seeds = [b"doge-config"]

// TaxConfig - singleton
seeds = [b"tax-config"]

// UnrefinedRewards - singleton
seeds = [b"unrefined-rewards"]

// BuybacksAccount - singleton
seeds = [b"buybacks"]
```

### Vault PDAs

```rust
// SOL Treasury
seeds = [b"sol-treasury"]

// Doges Treasury (NFT sale SOL)
seeds = [b"doges-treasury"]

// MineBTC Vault Authority
seeds = [b"minebtc-vault-authority"]

// MineBTC Vault (token account)
seeds = [b"minebtc_vault"]

// Buybacks SOL Vault
seeds = [b"buybacks-sol-vault"]

// SOL Prize Pot
seeds = [b"sol-prize-pot"]

// Motherlode Pot
seeds = [b"motherlode-pot"]

// Staker SOL Reward Vault
seeds = [b"staker-sol-reward-vault"]

// MineBTC Custodian (holds all staked dogeBTC)
seeds = [b"minebtc-custodian"]

// MineBTC Custodian Authority
seeds = [b"minebtc-custodian-authority"]

// LP Custodian (holds all staked LP tokens)
seeds = [b"lp-custodian"]

// LP Custodian Authority
seeds = [b"lp-custodian-authority"]
```

### Tax System PDAs

```rust
// Withdraw Withheld Authority (Token-2022)
seeds = [b"withdraw-withheld-authority"]

// Faction Treasury Vault
seeds = [b"faction-treasury-vault"]

// NFT Floor Sweep Vault
seeds = [b"nft-floor-sweep-vault"]

// NFT Sale SOL Vault
seeds = [b"nft-sale-sol-vault"]
```

### Per-Entity PDAs

```rust
// FactionState - one per faction (faction_id is u8)
seeds = [b"faction", &[faction_id]]

// PlayerData - one per user
seeds = [b"player", user.key().as_ref()]

// GameSession - one per round (round_id is u64)
seeds = [b"game-session", &round_id.to_le_bytes()]

// UserGameBet - one per user per round
seeds = [b"user-bet", user.key().as_ref(), &round_id.to_le_bytes()]

// AutominerVault - one per user
seeds = [b"autominer", user.key().as_ref()]

// AutominerCustody - global
seeds = [b"autominer-custody"]

// StakedPosition - per user per position (position_index is u8)
seeds = [b"staked-position", user.key().as_ref(), &[position_index]]

// LP StakedPosition - per user per position
seeds = [b"lp-staked-position", user.key().as_ref(), &[position_index]]

// ReferralRewards - one per user
seeds = [b"referral-rewards", user.key().as_ref()]

// DogeMetadata - per NFT mint
seeds = [b"doge-metadata", doge_mint.key().as_ref()]

// DogeCustody - global (holds staked NFTs)
seeds = [b"doge-custody"]

// Collection Authority
seeds = [b"collection_authority"]
```

## Important Constants

### From `state.rs`

```rust
// Game Grid
pub const NUM_BLOCKS: usize = 24;           // Total blocks in game
pub const NUM_FACTIONS: usize = 12;         // Total factions
pub const BLOCKS_PER_FACTION: usize = 2;    // Each faction owns 2 blocks
pub const MAX_CRANKER_BOTS: usize = 3;      // Whitelisted cranker bots

// Staking
pub const MAX_STAKED_DOGES: usize = 5;      // Max doges user can stake
pub const MAX_ALLOWED_POSITIONS: u8 = 7;    // Max staking positions per type
pub const EMERGENCY_WITHDRAWAL_PENALTY_PCT: u8 = 15;  // Early unstake penalty

// Token
pub const MINEBTC_DECIMALS: u8 = 6;
pub const BURN_TAX_PERCENTAGE: u64 = 1;     // 1% transfer tax

// Precision
pub const INDEX_PRECISION: u64 = 1_000_000; // Reward index scaling

// Motherlode
pub const MOTHERLODE_CHANCE: u64 = 625;     // 1 in 625 (0.16%)

// Time
pub const DAY_IN_SECONDS: u64 = 86400;
```

### From `config.json`

```rust
// Game
round_duration_seconds = 60;                // 60 seconds per round

// Hashpower Lockup (linear interpolation)
min_lockup_days = 7;                        // Minimum lockup
max_lockup_days = 365;                      // Maximum lockup
base_multiplier = 100;                      // 1x at min lockup
max_multiplier = 420;                       // 4.2x at max lockup

// Doge NFT Bonding Curve
doge_base_price = 1_000_000_000;           // 1 SOL
doge_curve_a = 1_111_111;                  // Curve steepness
doge_max_supply = 42_690;                  // Max mintable

// Tax Distribution (of 1% transfer fee)
tax_burn_pct = 25;                         // 25% burned
tax_nft_floor_sweep_pct = 25;              // 25% to NFT sweep
tax_faction_treasury_pct = 25;             // 25% to factions

// Ticket Tiers (additional SOL on Doge mint)
tier_0 = 1_000_000;                        // 0.001 SOL
tier_1 = 10_000_000;                       // 0.01 SOL
tier_2 = 100_000_000;                      // 0.1 SOL
```

### Doge Multipliers (staked count -> multiplier)

```rust
// Multiplier = BASE_MULTIPLIER (1000) + bonus
// 0 doges: 1000 (1.0x)
// 1 doge:  1300 (1.3x)
// 2 doges: 1600 (1.6x)
// 3 doges: 2000 (2.0x)
// 4 doges: 2500 (2.5x)
// 5 doges: 3000 (3.0x)
```

## Common Gotchas

1. **BN vs u64**: Anchor returns BN objects in JS. Convert with `.toNumber()` or `.toString()` for large values.

2. **Account Size**: Space must be declared upfront. Add new fields at END of structs.

3. **PDA Derivation**: Always verify seeds match between client and program.

4. **Reward Index Precision**: Use `u128` for intermediate calculations to prevent overflow.

5. **Token-2022 Transfers**: Must use `transfer_checked` with correct decimals.

6. **Metaplex Core**: Doge NFTs use Metaplex Core (not Token Metadata).

7. **Lockup Timestamps**: Use `Clock::get()?.unix_timestamp` for time comparisons.

8. **Emergency Unstake**: 15% penalty goes to other stakers via reward index.

## Testing Commands

```bash
# Build program
anchor build -p minebtc

# Run tests
anchor test

# Deploy to devnet
anchor deploy --provider.cluster devnet

# Generate IDL
anchor idl build -p minebtc
```

## Event Reference

Key events for backend indexing:

| Event | When Emitted | Key Fields |
|-------|--------------|------------|
| `RoundStarted` | Round begins | round_id, block_assignments |
| `RoundEnded` | Round ends | round_id, winning_block, winning_faction |
| `BetsPlaced` | User bets | round_id, user, amounts, blocks |
| `RewardsDistributedForFaction` | Rewards calc'd | round_id, faction_id |
| `RoundRewardsClaimed` | User claims | round_id, user, sol_reward, minebtc_reward |
| `DogeBtcStaked` | User stakes | user, amount, lockup_days |
| `DogeBtcUnstaked` | User unstakes | user, amount |
| `SolRewardsClaimed` | User claims SOL | user, amount |
| `MinebtcRewardsClaimed` | User claims token | user, amount |
| `AutominerInitialized` | Autominer created | user, sol_amount, num_rounds |
| `AutominerBetsPlaced` | Autominer bets | user, round_id, amounts |
| `DogeEvolved` | Doge evolves | doge_mint, new_stage |
| `DogePowerMutation` | Doge mutates | doge_mint, trait_index, new_value |
| `PriceSnapshotTaken` | Price recorded | price, snapshot_index |
| `RateUpdated` | Emissions change | old_rate, new_rate |
