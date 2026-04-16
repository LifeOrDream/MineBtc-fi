# Passive Staking and Doge Multipliers

This document describes the **passive staking** system in `stake.rs` and the
**Doge staking multiplier** system in `doges.rs`.

It is intentionally separate from gameplay-Doge progression:

- `stake.rs` and `int_stake_doge` are about **passive yield and faction hashpower**
- gameplay locking in `user.rs` is about **round betting, mutations, XP, and rebase/campaign logic**

Those two systems use different multipliers and should stay mentally separate.

---

## 1. What Users Can Stake

Users can open passive positions in their **home faction** for:

- `MineBTC`
- `LP tokens`

Each position is stored in a `StakedPosition` PDA and has:

- `position_type`
  - `0` = MineBTC
  - `1` = LP
- `position_index`
- `staked_amount`
- `weighted_amount`
- `multiplier`
- `lockup_end_timestamp`

The player can also stake up to `MAX_STAKED_DOGES` Doges. Those Doges do **not**
earn staking rewards by themselves. Instead, they multiply the player's passive
MineBTC/LP hashpower.

---

## 2. Core Passive Staking Formula

Passive staking is applied in two layers:

1. Lockup multiplier
2. Doge multiplier

### Layer 1: lockup weighting

For both MineBTC and LP positions:

```text
weighted_amount = staked_amount × lockup_multiplier / 100
```

- `lockup_multiplier` comes from `HashpowerConfig`
- longer lockups produce more `weighted_amount`

### Layer 2: Doge multiplier

Then the player's passive Doge multiplier is applied:

```text
hashpower_contribution = weighted_amount × doge_multiplier / BASE_MULTIPLIER
```

Where:

- `BASE_MULTIPLIER = 1000` means `1.0x`
- `doge_multiplier = 1500` means `1.5x`

This final `hashpower_contribution` is what changes:

- `player_data.dogebtc_hashpower` / `player_data.lp_hashpower`
- `faction_state.total_dogebtc_hashpower` / `faction_state.total_lp_hashpower`

---

## 3. MineBTC Staking

### Stake flow

`int_stake_minebtc`:

1. Validates the player is staking into their home faction
2. Reads the live Token-2022 transfer fee from the MineBTC mint
3. Uses the **post-fee credited amount** as the real staked amount
4. Computes lockup-weighted amount
5. Syncs pending staking rewards before changing balances
6. Creates/initializes the `StakedPosition`
7. Adds the new hashpower into player + faction totals
8. Transfers the requested MineBTC into the custodian vault

### Important note about transfer fees

MineBTC staking does **not** assume a hardcoded 1% transfer fee anymore.

The credited position amount is:

```text
actual_amount = requested_amount - live_token_2022_fee
```

This matters because otherwise staking credits drift from what the token transfer
actually delivered.

### Unstake flow

`int_unstake_minebtc`:

1. Syncs pending rewards before mutating balances
2. Removes that position's hashpower from player + faction totals
3. If lockup is still active, applies emergency penalty
4. Burns the MineBTC penalty from custody
5. Transfers the remaining MineBTC back to the user
6. Closes the position PDA

### Early withdrawal penalty

MineBTC early-withdrawal penalty is **fully burned**.

The penalty scales by how much of the lockup is still remaining:

```text
penalty_pct = emergency_tax_pct × remaining_lockup_pct
penalty_amount = staked_amount × penalty_pct / 100
```

---

## 4. LP Staking

LP staking uses the same structure as MineBTC staking with two differences:

- there is no Token-2022 fee normalization step
- early withdrawal burns LP penalty directly

### Stake flow

`int_stake_lp_tokens`:

1. Validates home-faction staking
2. Uses the full deposited LP amount as `actual_amount`
3. Computes lockup-weighted amount
4. Syncs pending staking rewards before changing balances
5. Creates the LP `StakedPosition`
6. Applies the player's Doge multiplier
7. Transfers LP into custody

### Unstake flow

`int_unstake_lp_tokens`:

1. Syncs pending rewards first
2. Removes position hashpower from player + faction
3. Burns any early-withdrawal LP penalty
4. Returns the remaining LP to the user
5. Closes the position PDA

---

## 5. How Passive Rewards Actually Accrue

Passive staking has **two reward rails**:

- SOL rewards
- MineBTC rewards

### SOL rewards

SOL rewards come from the round staker-fee lane.

They are tracked through faction reward indexes:

- `faction_state.dogebtc_sol_reward_index`
- `faction_state.lp_sol_reward_index`

When a user syncs rewards, new SOL is added to:

- `player_data.pending_sol_rewards`

When the user calls `claim_staking_rewards`, that SOL is transferred directly to
their wallet and `pending_sol_rewards` is reset to zero.

### MineBTC rewards

MineBTC rewards come from faction reward indexes too:

- `faction_state.dogebtc_dogebtc_reward_index`
- `faction_state.lp_dogebtc_reward_index`

When rewards are synced:

1. newly accrued MineBTC is added to `pending_minebtc_rewards`
2. global claimable MineBTC is added to `unrefined_rewards.total_minebtc_claimable`
3. an attribution event is emitted

MineBTC is **not transferred** during staking reward sync.
It remains pending until the user explicitly calls `withdraw_dbtc_rewards`.

---

## 6. Refining Fee / Unrefining Index

MineBTC withdrawal uses a refining redistribution mechanic.

When a user withdraws pending MineBTC:

1. any deferred refining yield is synced first
2. a refining fee may be taken from the withdrawing balance
3. that fee is redistributed through `unrefined_rewards.unrefining_index`
4. remaining unclaimed users receive that yield later when they sync/withdraw

### Why this exists

This creates a recycling loop:

- impatient claimers pay a fee
- long-tail claimers earn that fee proportionally

### Important accounting rule

Only the user's own pending MineBTC is deducted from
`unrefined_rewards.total_minebtc_claimable`.

Referral bonus and referral reward are paid from the emissions vault directly and
must **not** be subtracted from total claimable, otherwise the unrefining index drifts.

---

## 7. Doge Staking vs Gameplay Doge

There are **two different Doge multipliers** in the system.

### Passive staking Doges

Handled in `doges.rs`:

- `int_stake_doge`
- `int_unstake_doge`

These affect:

- `player_data.doge_multiplier`
- passive MineBTC hashpower
- passive LP hashpower

Important properties:

- a user can stake **any owned Doge**, regardless of Doge faction
- the multiplier only boosts the player's **home-faction passive staking**
- the contract reconstructs the raw multiplier from remaining staked Doge metadata
- the **effective** multiplier is capped by `MAX_MULTIPLIER`

### Gameplay Doge

Handled in `user.rs`:

- `use_doge_for_gameplay`
- `request_doge_gameplay_unlock`
- `withdraw_doge_from_gameplay`

This uses:

- `player_data.active_multiplier`

That multiplier is for:

- weighted round points
- mutation score logic
- gameplay progression

It is a separate system from passive staking.

---

## 8. How Passive Doge Multiplier Works Internally

The program does **not** store a separate uncapped raw multiplier in `PlayerData`.

Instead:

1. the client passes metadata accounts for already-staked Doges in `remaining_accounts`
2. the program reconstructs the raw sum
3. it derives the capped effective multiplier
4. it recalculates player hashpower using the old and new effective multipliers

That is why `stake_doge` / `unstake_doge` must include the remaining Doge metadata accounts.

### Recalculation model

When the passive multiplier changes:

```text
new_hashpower = old_hashpower × new_effective_multiplier / old_effective_multiplier
```

This keeps MineBTC and LP passive hashpower aligned with the updated Doge boost.

---

## 9. Events Useful For Backend / Frontend

### Position lifecycle

- `MineBtcStaked`
- `MineBtcUnstaked`
- `LiquidityStaked`
- `LiquidityUnstaked`
- `EmergencyWithdrawal`

### Reward lifecycle

- `SolRewardsClaimed`
- `DbtcRewardsClaimed`
- `MinebtcClaimableAccrued`
- `RefiningFeeRedistributed`

### Doge passive multiplier lifecycle

- `DogeStaked`
- `DogeUnstaked`

If you want a wallet-level reward breakdown on frontend, the canonical source is:

- `MinebtcClaimableAccrued.source`
- plus `DbtcRewardsClaimed` for actual withdrawal

---

## 10. Debug Log Tags

Useful log prefixes when debugging staking:

- `[stake_minebtc]`
- `[unstake_minebtc]`
- `[stake_lp_tokens]`
- `[unstake_lp_tokens]`
- `[claim_staking_rewards]`
- `[withdraw_dbtc_rewards]`
- `[update_minebtc_rewards]`
- `[update_lp_rewards]`
- `[stake_doge]`
- `[unstake_doge]`
- `[load_staked_doge_raw_multiplier]`

---

## 11. Product / Game Recommendations

### Keep

- Home-faction-only passive staking
- Any-faction passive Doge staking
- Separate passive and gameplay multipliers
- Refining redistribution as a sticky reward loop

### Improve in UX

1. Show the two multiplier systems separately
   Passive multiplier and gameplay multiplier should never be shown as if they are the same stat.

2. Make refining fee explicit before withdrawal
   The user should see:
   - current pending MineBTC
   - refining fee
   - referral bonus
   - net amount received

3. Show passive reward sources separately
   Users should be able to see how much pending MineBTC came from:
   - rounds
   - rebase/campaign
   - MineBTC staking
   - LP staking
   - refining yield

4. Make lockup choice legible
   Users should understand that the order is:
   `deposit -> lockup multiplier -> passive Doge multiplier -> final hashpower`

### Business / balance thoughts

1. Passive Doge staking is strong because it boosts both MineBTC and LP rails.
   That is good for collectible demand, but frontend should clearly explain the cap.

2. Refining yield is interesting but abstract.
   It likely needs very strong UI copy, otherwise it will feel like hidden tax instead of
   a strategic claim-timing mechanic.

3. LP staking should probably be messaged as the "higher commitment / dual-exposure" lane.
   MineBTC staking is simpler, LP staking is more advanced.

4. The passive layer feels production-viable if the UI keeps it understandable.
   The bigger risk is not contract math now; it is user confusion between:
   - staking Doges
   - gameplay Doge
   - pending MineBTC
   - withdrawing MineBTC
   - refining redistribution
