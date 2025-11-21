# Moonbase Program Analysis: Game & User Logic

## Overview

This document outlines the analysis of the `game.rs`, `user.rs`, and `stake.rs` files in the Moonbase program. It highlights the game flow, betting mechanics, and critical bugs identified in the fee collection and reward distribution logic.

## Game Flow

1.  **Start Round (`game.rs`)**:

    - A cranker bot calls `start_round`.
    - Validates game is active and can begin.
    - Randomly assigns 12 factions to 24 blocks (2 blocks per faction) using a commit-reveal scheme + slot hash.
    - Initializes `GameSession`.

2.  **Betting (`user.rs`)**:

    - Users call `join_round` or `join_round_batch`.
    - Bets can be on specific blocks, factions (high/low/both), or random.
    - **Fees**:
      - `protocol_fee`: Sent to `sol_treasury`.
      - `stakers_fee`: Calculated but **NOT collected from user** (Bug #1).
      - `net_amount`: Sent to `sol_prize_pot_vault`.
    - Bets are tracked in `UserGameBet` and aggregated in `GameSession`.

3.  **End Round (`game.rs`)**:

    - Cranker bot calls `end_round`.
    - Verifies commit-reveal.
    - Generates final randomness.
    - Selects winning block and faction.
    - Calculates DogeBtc rewards pools (winner, same-faction, stakers, motherlode).
    - Updates reward indexes for SOL and DogeBtc.

4.  **Distribute Faction Rewards (`game.rs`)**:

    - Cranker bot calls `end_round_faction_rewards`.
    - Updates winning faction's reward indexes.
    - Transfers `sol_staker_fees` to `sol_rewards_vault`.
    - **Bug #1 Confirmed**: The transfer source is the `authority` (cranker bot), not the user fees.

5.  **Claim Rewards (`user.rs` & `stake.rs`)**:
    - **Round Rewards (`user.rs`)**:
      - User calls `claim_round_rewards`.
      - Calculates winnings based on bets and winning block.
      - Updates `player_data.pending_sol_rewards` and `player_data.pending_dbtc_rewards`.
    - **SOL Payout (`stake.rs`)**:
      - User calls `claim_sol_rewards`.
      - Transfers `pending_sol_rewards` from `sol_rewards_vault` to user.
      - **Bug #2 Confirmed**: `pending_sol_rewards` includes prize pot winnings, but funds are taken from `sol_rewards_vault` (staker fees) instead of `sol_prize_pot_vault`.

## Critical Bugs Identified

### 1. Stakers Fee Collection Bug

- **Location**: `user.rs` (`internal_join_round`) and `game.rs` (`end_round_faction_rewards`).
- **Issue**: In `internal_join_round`, the `stakers_fee` is calculated and deducted from the `net_amount` (so it doesn't go to the pot), but it is **never transferred** from the user's wallet to any vault.
- **Consequence**: The user underpays by the amount of the stakers fee.
- **Compounding Issue**: In `end_round_faction_rewards`, the `sol_staker_fees` are transferred to `sol_rewards_vault` using the **cranker bot's wallet** (`authority`) as the source.
- **Impact**: Cranker bots will be drained of SOL paying for users' staker fees.

### 2. Prize Pot Payout Bug

- **Location**: `user.rs` (`claim_round_rewards`) and `stake.rs` (`claim_sol_rewards`).
- **Issue**:
  - `claim_round_rewards` adds the user's round winnings (from the prize pot) to `player_data.pending_sol_rewards`.
  - `claim_sol_rewards` pays out the _entire_ `pending_sol_rewards` balance from the `sol_rewards_vault`.
- **Consequence**: Round winnings (which are held in `sol_prize_pot_vault`) are paid out from `sol_rewards_vault` (which only holds staker fees).
- **Impact**:
  - `sol_prize_pot_vault` will accumulate SOL indefinitely (black hole).
  - `sol_rewards_vault` will be drained immediately because it only holds small fees but is asked to pay out large prize winnings.
  - Users will be unable to claim rewards once `sol_rewards_vault` is empty.

## Recommendations

1.  **Fix Stakers Fee Collection**:

    - In `user.rs` -> `internal_join_round`: Transfer the `stakers_fee` from the user (`payer`) to the `sol_rewards_vault` immediately when the bet is placed.
    - Remove the transfer in `game.rs` -> `end_round_faction_rewards` since the funds would already be in the vault.

2.  **Fix Prize Pot Payout**:
    - Split `pending_sol_rewards` in `PlayerData` into `pending_round_sol_rewards` and `pending_staking_sol_rewards`.
    - OR keep `pending_sol_rewards` but track the source.
    - **Better Approach**:
      - In `claim_round_rewards` (`user.rs`), transfer the SOL winnings **directly** from `sol_prize_pot_vault` to the user. Do not add it to `pending_sol_rewards`.
      - Keep `pending_sol_rewards` strictly for staking rewards (which are paid from `sol_rewards_vault` in `claim_sol_rewards`).
      - This separates the two cash flows cleanly:
        - Betting Winnings -> `sol_prize_pot_vault` -> User (via `claim_round_rewards`)
        - Staking Rewards -> `sol_rewards_vault` -> User (via `claim_sol_rewards`)

## Other Notes

- **Autominer**: The autominer logic seems sound, but it will be affected by the same bugs (underpaying fees, unable to claim winnings).
- **Motherlode**: The motherlode logic correctly moves funds between pools, but since the payout mechanism is broken, it doesn't matter yet.
