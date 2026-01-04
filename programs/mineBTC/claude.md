# MineBTC Solana Program - Gameplay Documentation

## Overview

This is an Anchor-based Solana program implementing a raffle-style game where users bet on 24 blocks (belonging to 12 factions) each round. The game runs in 60-second rounds with automatic winners based on VRF randomness.

## Core Architecture

### Round Lifecycle

1. **Round Start** (`start_new_round` in `game.rs`)
   - Triggered by crank/autominer system
   - Generates block assignments (24 blocks assigned to 12 factions, 2 blocks per faction)
   - Creates commit hash for VRF randomness
   - Emits `RoundStarted` event with block assignments

2. **Betting Phase** (`join_round` in `user.rs`)
   - Users bet SOL on 1-4 blocks per transaction
   - Each bet earns points (based on SOL amount)
   - Weighted points calculated from multipliers (doge multiplier, ticket boosts)
   - Autominer can bet automatically each round

3. **Round End** (`end_round` in `game.rs`)
   - Triggered after 60 seconds AND minimum 2 unique bettors on different blocks
   - If <2 bettors, round extends until condition met
   - VRF reveals winning block
   - Winning faction identified (2 blocks win: winning block + same faction block)

4. **Rewards Distribution** (`distribute_round_rewards_for_faction` in `game.rs`)
   - SOL pool redistributed to winning block bettors (proportional to points)
   - dogeBTC emissions distributed to winning block (proportional to weighted points)
   - dogeBTC emissions distributed to same-faction block bettors
   - Faction stakers receive portion of emissions
   - Motherlode jackpot check (random 0.66% chance)

5. **Claiming** (`claim_round_rewards` in `user.rs`)
   - Users claim SOL + dogeBTC rewards from winning rounds
   - Multiple rounds can be claimed in one transaction

## Key Files

### `instructions/game.rs`
- `start_new_round`: Initialize new round with block assignments
- `end_round`: End round and determine winners
- `distribute_round_rewards_for_faction`: Distribute rewards per faction
- `crank_autominer_bets`: Process autominer bets at round start

### `instructions/user.rs`
- `initialize_player`: Create player account with faction
- `join_round`: Place bets on blocks
- `claim_round_rewards`: Claim winnings
- `init_autominer_vault`: Setup autominer for auto-betting
- `update_autominer_vault`: Modify autominer settings
- `stop_autominer`: Stop and refund autominer
- `reload_autominer`: Extend autominer duration

### `instructions/doges.rs`
- `deploy_doge`: Deploy NFT doge for gameplay
- `lock_doge`: Lock doge to earn XP and mutations
- `unlock_doge`: Return doge to user
- `trigger_mutation`: Trigger doge stat changes
- `trigger_evolution`: Evolve doge to next stage

### `state.rs`
- `GlobalState`: Global game configuration
- `GameSession`: Per-round data (bets, blocks, totals)
- `PlayerData`: User account (faction, stats, pending rewards)
- `BtcDoge`: Doge NFT state (DNA, XP, multiplier)
- `AutominerVault`: Autominer configuration per user
- `FactionState`: Per-faction staking and rewards data

## Events

Key events emitted for backend indexing:

```rust
// Round lifecycle
RoundStarted { round_id, block_assignments, ... }
RoundEnded { round_id, winning_block, winning_faction_id, ... }
RewardsDistributedForRound { round_id }

// User actions
BetsPlaced { round_id, user, target_blocks, net_amounts, points_amounts, ... }
RoundRewardsClaimed { round_id, user, sol_reward, minebtc_reward, ... }

// Autominer
AutominerInitialized { user, sol_amount, num_rounds, ... }
AutominerBetsPlaced { round_id, user, target_blocks, rounds_remaining, ... }
AutominerStopped { user, refund_amount, ... }

// Doge mutations
DogeEvolved { doge_mint, new_stage, ... }
DogePowerMutation { doge_mint, trait_index, old_val, new_val, ... }
```

## Game Mechanics

### Block/Faction System
- 24 blocks total, 12 factions
- Each faction owns 2 blocks per round (randomly assigned)
- Winning block determined by VRF
- Same-faction block (other block of winning faction) also wins dogeBTC

### Betting
- Minimum bet: 0.001 SOL
- Maximum bets per round: 40 per user
- Points = SOL amount (1:1)
- Weighted points = points * multiplier (doge + ticket boosts)

### Rewards
- SOL pool: All bets minus fees, redistributed to winning block bettors
- dogeBTC emissions: Fixed per round from global state
- Distribution: 60% winning block, 25% same-faction, 15% faction stakers
- Motherlode: 0.66% chance to win accumulated jackpot

### Autominer
- Users deposit SOL for N rounds of auto-play
- Configurable: specific blocks OR specific factions
- Bets placed automatically at round start by crank
- Can be stopped/reloaded anytime

### Doges
- NFT companions that boost betting multiplier
- Gain XP from gameplay
- Random mutations during rounds (power traits, visual traits)
- Evolution stages: Pup -> Shiba -> Alpha -> Omega
