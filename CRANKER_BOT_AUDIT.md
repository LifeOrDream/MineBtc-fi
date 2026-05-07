# Cranker Bot Claim Security Audit

## Date: 2026-04-30
## Scope: `claim_round_rewards`, `claim_autominer_rewards`, `claim_faction_war_rewards`

---

## Executive Summary

**FINDING: No exploit path exists.** The current cranker bot design is secure — bots can call claim instructions on behalf of users, but **all rewards are cryptographically bound to the rightful owner** via PDA derivation. A malicious bot cannot redirect rewards to itself or any other wallet.

**RECOMMENDATION: Remove the opt-out mechanism.** Users should not be able to disable bot claims. Bots are essential for UX (users don't want to manually claim every round), and the opt-out adds unnecessary complexity with zero security benefit.

---

## How Cranker Claims Work

### `claim_round_rewards`

```rust
#[account(
    mut,
    seeds = [PLAYER_DATA_SEED.as_ref(), user_wallet.key().as_ref()],
    bump = player_data.bump
)]
pub player_data: Box<Account<'info, PlayerData>>,

#[account(mut)]
pub user_wallet: UncheckedAccount<'info>,

#[account(mut)]
pub caller: Signer<'info>,
```

**Key security properties:**
1. `player_data` is a PDA derived from `PLAYER_DATA_SEED + user_wallet`. The bot **cannot** pass a different `user_wallet` because the PDA would not resolve.
2. `user_game_bet` is a PDA derived from `USER_GAME_BET_SEED + user_wallet + round_id` with constraint `user_game_bet.owner == user_wallet.key()`. The bot **cannot** forge this.
3. SOL is transferred **to `user_wallet`**, not to `caller`:
   ```rust
   helper::transfer_from_sol_prize_pot_vault(
       &sol_prize_pot_vault,
       &user_wallet,  // ← user's wallet, NOT caller's
       ...
   )?;
   ```
4. degenBTC is added to `player_data.pending_minebtc_rewards`. Since `player_data` is a PDA of `user_wallet`, the bot **cannot** credit a different user.

**What the bot gets:** Only the rent lamports from closing the `user_game_bet` account (~0.002 SOL). This is the intended incentive.

---

### `claim_autominer_rewards`

```rust
#[account(
    mut,
    seeds = [AUTOMINER_VAULT_SEED.as_ref(), autominer_vault.owner.as_ref()],
    bump = autominer_vault.vault_bump
)]
pub autominer_vault: Box<Account<'info, AutominerVault>>,

#[account(mut, constraint = owner_wallet.key() == autominer_vault.owner)]
pub owner_wallet: UncheckedAccount<'info>,
```

**Key security properties:**
1. `autominer_vault` is a PDA derived from the owner's pubkey. Bot cannot change this.
2. `owner_wallet` has a hard constraint `owner_wallet.key() == autominer_vault.owner`. Bot cannot redirect to itself.
3. Leftover SOL goes to `owner_wallet` (the user).
4. Auto-reload SOL goes to `autominer_custody` (a protocol PDA), not the bot.

---

### `claim_faction_war_rewards`

```rust
#[account(
    mut,
    seeds = [USER_FACTION_WAR_BETS_SEED, user_faction_war_bets.owner.as_ref(), &faction_war_id.to_le_bytes()],
    bump = user_faction_war_bets.bump,
)]
pub user_faction_war_bets: Box<Account<'info, UserFactionWarBets>>,

#[account(
    mut,
    constraint = player.key() == user_faction_war_bets.owner
)]
pub player: AccountInfo<'info>,
```

**Key security properties:**
1. `user_faction_war_bets` is a PDA derived from the owner's pubkey.
2. `player` account has constraint `player.key() == user_faction_war_bets.owner`. Bot cannot redirect SOL to itself.
3. degenBTC goes to `player_data.pending_minebtc_rewards`, which is a PDA of the owner.

---

## Exploit Scenarios Tested

| Scenario | Possible? | Why |
|---|---|---|
| Bot claims for User A, redirects SOL to Bot's wallet | **NO** | `user_wallet` is part of the PDA seed for `player_data` and `user_game_bet` |
| Bot claims for User A, credits degenBTC to Bot's account | **NO** | `player_data` is a PDA of `user_wallet`, not `caller` |
| Bot claims for User A, but passes User B's wallet | **NO** | `user_game_bet` constraint `owner == user_wallet` fails |
| Bot front-runs user's own claim to steal rent | **NO** | Rent is minimal (~0.002 SOL), and only one claim per round per user |
| Bot spams claims to drain prize pot | **NO** | Each claim requires a valid `user_game_bet` PDA, and the prize pot is only touched for that user's specific reward |

---

## What `allow_bots_to_claim` Actually Does

The `validate_reward_claim_caller` check:
```rust
require!(caller == owner || allow_bots_to_claim, ...)
```

This check is **redundant for security**. Even if removed entirely:
- A bot calling `claim_round_rewards` for User A **must** pass User A's `user_wallet` because it's a PDA seed
- The rewards **must** go to User A's wallet because the transfer target is `user_wallet`

The check only serves to **prevent bots from claiming on behalf of users who opted out**. It does not prevent any exploit.

---

## Recommendation: Remove Opt-Out

**Rationale:**
1. **No security benefit** — the PDA architecture already makes exploits impossible
2. **UX degradation** — users who opt out must manually claim every round (60s rounds = painful)
3. **Operational burden** — frontend needs to check `allow_bots_to_claim` before calling cranker endpoints
4. **False sense of control** — users think they're "securing" something, but they're not

**Changes needed:**
1. Remove `allow_bots_to_claim: bool` from `PlayerData`
2. Remove `validate_reward_claim_caller` helper
3. Remove `PermissionlessRewardClaimsDisabled` error
4. Remove `set_player_claim_settings` instruction
5. Remove `internal_set_player_claim_settings` function
6. Remove `SetPlayerClaimSettings` accounts struct
7. Remove `allow_bots_to_claim = true` from `initialize_player`
8. Remove all `validate_reward_claim_caller` calls from claim functions
9. Update `PlayerData::LEN`
10. Update frontend IDL
