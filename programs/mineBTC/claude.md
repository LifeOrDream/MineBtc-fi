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

```rust
// GlobalConfig - singleton
seeds = [b"global_config"]

// FactionState - per faction
seeds = [b"faction", &faction_id.to_le_bytes()]

// PlayerData - per user
seeds = [b"player", user.key().as_ref()]

// GameSession - per round
seeds = [b"game", &round_id.to_le_bytes()]

// MineBtcMining - singleton
seeds = [b"minebtc_mining"]

// AutominerVault - per user
seeds = [b"autominer", user.key().as_ref()]

// FactionStakeInfo - per user per faction
seeds = [b"faction_stake", user.key().as_ref(), &faction_id.to_le_bytes()]
```

## Important Constants

```rust
// Betting
pub const MIN_BET_AMOUNT: u64 = 1_000_000;  // 0.001 SOL
pub const MAX_BETS_PER_ROUND: u8 = 40;
pub const ROUND_DURATION_SECS: i64 = 60;

// Factions
pub const NUM_FACTIONS: u8 = 12;
pub const BLOCKS_PER_ROUND: u8 = 24;  // 2 per faction

// Fees (basis points)
pub const PROTOCOL_FEE_BPS: u64 = 1000;  // 10%
pub const STAKERS_FEE_BPS: u64 = 4000;   // 40%
pub const PRIZE_POT_BPS: u64 = 5000;     // 50%

// Rewards distribution
pub const WINNING_BLOCK_SHARE: u64 = 60;     // 60%
pub const SAME_FACTION_SHARE: u64 = 25;       // 25%
pub const STAKERS_SHARE: u64 = 15;            // 15%

// Motherlode
pub const MOTHERLODE_HIT_CHANCE: u64 = 16;   // 0.16% = 16/10000

// Staking lockup multipliers (scaled by 100)
pub const LOCKUP_30D_MULT: u64 = 100;   // 1.0x
pub const LOCKUP_90D_MULT: u64 = 250;   // 2.5x
pub const LOCKUP_180D_MULT: u64 = 500;  // 5.0x
pub const LOCKUP_1Y_MULT: u64 = 900;    // 9.0x
pub const LOCKUP_3Y_MULT: u64 = 1500;   // 15.0x

// Precision for reward index calculations
pub const PRECISION: u128 = 1_000_000_000_000;  // 10^12
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
