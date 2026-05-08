# EXHAUSTIVE i64/u64 CAST AUDIT REPORT
## MineBtc-fi Solana Contract — All .rs files
## Date: 2026-03-26

> Historical note: this report is a point-in-time audit snapshot from before the country-direction round and index-epoch refactor. Treat gameplay descriptions and line references here as archival, and verify against current source before acting on them.

---

## CRITICAL FINDINGS (Bugs / Potential Exploits)

### ⛔ CRITICAL #1: stake.rs:431 — i64-to-u64 cast of potentially negative value (MineBTC unstake)
```
File: instructions/stake.rs:426-431
Code:
    let total_lockup_seconds = user_position.lockup_end_timestamp - user_position.start_timestamp;
    let remaining_seconds = user_position.lockup_end_timestamp - current_ts;
    let mut remaining_seconds_pct = 0u64;
    if total_lockup_seconds > 0 {
        remaining_seconds_pct =
            (M_HUNDRED as i64 * remaining_seconds / total_lockup_seconds) as u64;
    }
```
**What it represents:** Percentage of lockup time remaining for emergency withdrawal penalty calc.
**Can it be negative?** YES — `remaining_seconds` can be negative if `current_ts > lockup_end_timestamp` (lockup expired). The guard only checks `total_lockup_seconds > 0`, NOT `remaining_seconds > 0`.
**Can it overflow?** YES — negative i64 cast to u64 wraps to ~u64::MAX.
**Is it properly guarded?** NO — This is the SAME bug pattern as the emergency_tax bug.
**Impact:** If lockup has expired and someone calls unstake, `remaining_seconds` is negative, the `as u64` cast wraps to a huge number, creating a massive bogus penalty percentage.
**HOWEVER:** Line 315 sets `is_early_withdrawal = current_ts < user_position.lockup_end_timestamp`, and the penalty calc (line 433) gates on `is_early_withdrawal`. BUT lines 425-431 are AFTER the penalty logic and are used for EVENT EMISSION only (not actual penalty). So the wrong value goes into the event, not into the actual penalty. Still a bug but **LOW SEVERITY** (event data corruption, not fund loss).

### ⛔ CRITICAL #2: stake.rs:827 — SAME pattern for LP token unstake
```
File: instructions/stake.rs:818-827
Code:
    let total_lockup_seconds = user_position.lockup_end_timestamp - user_position.start_timestamp;
    let remaining_seconds = if is_early_withdrawal {
        user_position.lockup_end_timestamp - current_ts
    } else {
        0
    };
    let mut remaining_seconds_pct = 0u64;
    if total_lockup_seconds > 0 && is_early_withdrawal {
        remaining_seconds_pct =
            (M_HUNDRED as i64 * remaining_seconds / total_lockup_seconds) as u64;
    }
```
**Can it be negative?** This one is BETTER — it gates `remaining_seconds` behind `is_early_withdrawal` check, setting it to 0 if not early. AND the pct calc also checks `is_early_withdrawal`.
**Is it properly guarded?** YES, this one is guarded. SAFE.

### ⛔ CRITICAL #3: user.rs:718,761 — i64 subtraction and cast for autominer SOL accounting
```
File: instructions/user.rs:718
Code:
    let diff = new_sol_needed as i64 - old_sol_balance as i64;

File: instructions/user.rs:761
Code:
    autominer_vault.sol_balance = (old_sol_balance as i64 + sol_diff) as u64;
```
**What it represents:** SOL difference calculation for autominer vault updates.
**Can it overflow?** YES — if `new_sol_needed` or `old_sol_balance` exceeds i64::MAX (~9.2e18 lamports = ~9.2 billion SOL), the `as i64` cast wraps. In practice SOL supply is ~500M so values won't exceed i64::MAX.
**Can result be negative?** YES — `diff` CAN be negative (that's intentional — it means refund). Line 723 casts positive diff back to u64, line 736 casts `-diff` to u64. Both guarded by `if diff > 0` / `else if diff < 0`.
**Line 761:** `(old_sol_balance as i64 + sol_diff) as u64` — if `old_sol_balance as i64 + sol_diff` goes negative, this wraps to huge u64. This COULD happen if sol_diff is very negative.
**Is it properly guarded?** PARTIALLY. The diff is computed correctly, transfers are guarded. But line 761's `as u64` has no guard against negative result. If the vault somehow has less SOL than expected (rent, etc.), this could produce a bogus balance.
**SEVERITY: MEDIUM** — Theoretically, if sol_diff is more negative than old_sol_balance, the result wraps.

### ⛔ CRITICAL #4: user.rs:573 — u64 multiplication overflow
```
File: instructions/user.rs:573
Code:
    sol_per_round * num_rounds as u64
```
**What it represents:** Total SOL needed for autominer.
**Can it overflow?** YES — if sol_per_round and num_rounds are both large, this wraps silently. No checked_mul.
**Is it properly guarded?** NO — No overflow check. If a user passes large values, this can wrap to a small number, depositing less SOL than needed.
**SEVERITY: MEDIUM** — User could set up autominer with less SOL than expected.

### ⛔ CRITICAL #5: user.rs:702,710 — More u64 multiplication overflow
```
File: instructions/user.rs:702
Code:
    let current_sol_needed = rounds_remaining as u64 * old_sol_per_round;
File: instructions/user.rs:710
Code:
    let new_sol_needed = (rounds_remaining + rounds_added) as u64 * new_sol_per_round;
```
**Same pattern as #4.** No checked_mul.

---

## HIGH SEVERITY FINDINGS

### 🔴 HIGH #1: epoch.rs:351 — Timestamp i64-to-u64 comparison
```
File: instructions/epoch.rs:351
Code:
    clock.unix_timestamp as u64 >= epoch_state.end_timestamp,
```
**What it represents:** Checking if epoch has ended.
**Can it be negative?** `unix_timestamp` is i64; if somehow negative (impossible in practice on Solana), `as u64` wraps to huge number, always passing the check.
**Is it properly guarded?** Practically safe since Solana timestamps are always positive. BUT bad pattern — should compare as i64 instead.

### 🔴 HIGH #2: epoch.rs:372, game.rs:909 — Storing i64 timestamp as u64
```
File: instructions/epoch.rs:372
Code:
    epoch_config.last_epoch_start = clock.unix_timestamp as u64;
File: instructions/game.rs:909
Code:
    epoch_config.last_epoch_start = clock.unix_timestamp as u64;
```
**What it represents:** Storing timestamp.
**Problem:** Mixing i64 and u64 timestamp representations creates confusion. The `end_timestamp` in epoch_state is u64 but Solana's clock is i64. All comparisons must be consistent.
**SEVERITY: LOW** — Timestamps are positive in practice.

### 🔴 HIGH #3: user.rs:1823 — Another timestamp i64-to-u64
```
File: instructions/user.rs:1823
Code:
    epoch_state.start_timestamp = clock_now.unix_timestamp as u64;
```
Same pattern — storing i64 as u64.

### 🔴 HIGH #4: game.rs:881 — Timestamp comparison with cast
```
File: instructions/game.rs:881
Code:
    if clock.unix_timestamp as u64 >= epoch_state.end_timestamp && epoch_state.stage == 1 {
```
Same pattern as epoch.rs:351.

### 🔴 HIGH #5: tax.rs:683 — Timestamp as random seed
```
File: instructions/tax.rs:683
Code:
    let random_seed = clock.unix_timestamp as u64;
```
**What it represents:** Using timestamp as random seed (for leaderboard tiebreaker).
**Problem:** i64-to-u64 cast is fine here (timestamp positive). BUT using timestamp as randomness is weak/predictable.
**SEVERITY: LOW** for the cast, separate issue for randomness.

---

## MEDIUM SEVERITY FINDINGS

### 🟡 MEDIUM #1: economy.rs:1420 — i128-to-i64 narrowing cast
```
File: instructions/economy.rs:1405-1420
Code:
    let old = old_price as i128;
    let new = new_price as i128;
    let diff = new - old;
    let change_pct = (diff * 100) / old;
    ...
    (change_pct as i64, direction)
```
**What it represents:** Price change percentage.
**Can it overflow?** If `change_pct` exceeds i64::MAX (only if prices differ by more than ~9.2e16x), it truncates. In practice prices won't differ by this much.
**Is it properly guarded?** No explicit guard, but practically safe.
**SEVERITY: LOW**

### 🟡 MEDIUM #2: economy.rs:226 — u64-to-i64 cast
```
File: instructions/economy.rs:226
Code:
    let snapshot_interval = ctx.accounts.global_config.snapshot_interval as i64;
```
**What it represents:** Config value cast for timestamp arithmetic.
**Can it overflow?** If snapshot_interval > i64::MAX, wraps to negative. Admin-controlled value.
**Is it properly guarded?** Admin must set sane values. No explicit guard.

### 🟡 MEDIUM #3: helper.rs:391 — u64-to-i64 for lockup duration
```
File: instructions/helper.rs:391
Code:
    position.lockup_end_timestamp = current_ts.saturating_add(seconds_to_add as i64);
```
**What it represents:** Converting lockup duration (u64) to i64 for timestamp addition.
**Can it overflow?** If `seconds_to_add` > i64::MAX, wraps to negative, making lockup_end < current_ts.
**Is it properly guarded?** `saturating_add` prevents the addition overflow, but the `as i64` cast itself is unguarded.
**In practice:** `seconds_to_add = lockup_duration * DAY_IN_SECONDS`. Even 1000 days = 86.4M seconds, well within i64 range.
**SEVERITY: LOW**

### 🟡 MEDIUM #4: economy.rs:995-1003 — u128-to-u64 narrowing in LP price calc
```
File: instructions/economy.rs:995-1003
Code:
    (minebtc_consumed as u128) * (minebtc_price as u128) / (1_000_000) as u128
    ...
    } as u64;
    (total_value_sol as u128) * (1_000_000_000) as u128 / (lp_tokens_minted as u128)
    ...
    mine_btc_mining.lp_token_price_in_sol = ... as u64;
```
**What it represents:** LP token price calculation.
**Can it overflow?** u128 intermediate is fine, but final `as u64` truncates if result > u64::MAX.
**Is it properly guarded?** No `.min(u64::MAX as u128)` guard before cast.
**SEVERITY: LOW** — values unlikely to exceed u64::MAX in practice.

### 🟡 MEDIUM #5: economy.rs:815,820,837,839,878 — u128-to-u64 narrowing in LP calculations
```
File: instructions/economy.rs:815
    (lp_token_amount as u128 * sol_vault_balance as u128 / lp_supply as u128) as u64
File: instructions/economy.rs:820
    (lp_token_amount as u128 * minebtc_vault_balance as u128 / lp_supply as u128) as u64
File: instructions/economy.rs:837
    (available_sol as u128 * lp_supply as u128 / sol_vault_balance as u128) as u64
(etc.)
```
**Pattern:** All do `(u64 * u64 / u64) as u64` via u128 intermediate.
**Can it overflow?** The u128 intermediate prevents multiplication overflow. The final `as u64` could truncate if result > u64::MAX, but since numerator/denominator are u64, the result fits in u64 in most cases.
**Is it properly guarded?** No explicit `.min()` guard, but mathematically safe when dividing by a reasonable value.
**SEVERITY: LOW**

### 🟡 MEDIUM #6: epoch.rs:84,87,90 — u128-to-u64 narrowing for reward pools
```
File: instructions/epoch.rs:84
    reward_pools[ranked[0].1] += top1_bonus as u64;
File: instructions/epoch.rs:87
    reward_pools[ranked[1].1] += top2_bonus as u64;
```
**What it represents:** Adding bonus rewards computed as u128 back to u64 pools.
**Can it overflow?** `top1_bonus = pool as u128 * pct as u128 / 100`. Since pool is u64 and pct < 100, result fits in u64.
**SEVERITY: LOW** — mathematically safe.

---

## LOW SEVERITY / SAFE FINDINGS

### ✅ SAFE: helper.rs:427-445 — calculate_emergency_tax (ALREADY FIXED)
```
File: instructions/helper.rs:432-445
Code:
    let remaining_seconds = user_position.lockup_end_timestamp - current_ts;
    if remaining_seconds <= 0 || total_lockup_seconds <= 0 {
        return 0;
    }
    let remaining_seconds_pct = (M_HUNDRED as i64 * remaining_seconds) / total_lockup_seconds;
    let calc_penalty_pct = (emergency_tax * (remaining_seconds_pct as u64)) / M_HUNDRED;
```
**Status:** FIXED — has guard `if remaining_seconds <= 0 { return 0 }` before any cast.
**The `remaining_seconds_pct as u64` on line 445 is safe because it's guaranteed positive by the guard.**

### ✅ SAFE: genescience.rs:60-86 — u128 arithmetic for pricing
Uses checked_mul throughout. `result.min(u64::MAX as u128) as u64` properly clamped.

### ✅ SAFE: genescience.rs:246 — u128-to-u32 narrowing
```
    let xp_gained = ((user_total_bet as u128 * 1) / 1_000_000) as u32;
```
Division by 1M means result fits in u32 for any reasonable bet amount.

### ✅ SAFE: genescience.rs:278 — u64 multiplication
```
    let final_chance_bps = (MAX_BASE_CHANCE * bet_strength * (mult_factor as u64)) / 100_000_000;
```
MAX_BASE_CHANCE=3000, bet_strength<=10000, mult_factor<=10000. Max product = 3e11, fits in u64.

### ✅ SAFE: genescience.rs:296 — u16-to-u64 widening
```
    let roll_val = u16::from_le_bytes([seed[0], seed[1]]) as u64;
```
Always safe — widening cast.

### ✅ SAFE: All `i as u8` casts in genescience.rs for trait indexing
Loop indices are always small (0-15 range), safe to cast to u8.

### ✅ SAFE: genescience.rs:561 — u8-to-u16 widening then narrowing
```
    1 => ((t1 as u16 + t2 as u16) / 2) as u8,
```
Two u8 values averaged. Max = (255+255)/2 = 255. Fits in u8.

### ✅ SAFE: epoch.rs:50-57 — u128 intermediate arithmetic
Proper widening to u128 for multiplication, then narrowing back to u64.

### ✅ SAFE: epoch.rs:118, 209-212 — u8-to-u16 widening for percentage validation
```
    model5_pct as u16 + top1_pct as u16 + top2_pct as u16 + top3_pct as u16 <= 100,
```
u8 values (max 100 each) summed as u16 (max 400). Safe.

### ✅ SAFE: epoch.rs:359-363 — u128 with checked arithmetic
```
    (epoch_state.total_degenbtc_mined_in_epoch as u128)
        .checked_mul(epoch_state.risk_factor_snapshot as u128)
        ...
        .unwrap_or(0) as u64;
```
Uses checked_mul and unwrap_or(0). The final `as u64` could truncate but `unwrap_or(0)` catches overflow.

### ✅ SAFE: epoch.rs:442-446 — Reward calculation with checked arithmetic
Uses checked_mul, checked_div, unwrap_or(0).

### ✅ SAFE: user.rs:133 — constant cast
```
    player_data.hashbeast_multiplier = BASE_MULTIPLIER as u16;
```
BASE_MULTIPLIER is a known constant (1000). Fits in u16.

### ✅ SAFE: user.rs:370,961,2003,2065 — Modulo narrowing
```
    let random_block = (slot_bytes[0] % 24) as u8;
```
Result of `% 24` is always 0-23. Fits in u8.

### ✅ SAFE: user.rs:471,1951 — Comparison narrowing
```
    require!(block_id < NUM_BLOCKS as u8, ...);
```
NUM_BLOCKS is a small constant. Safe.

### ✅ SAFE: user.rs:473,482,503,513 — Small length casts
```
    bets_per_round = blocks.len() as u64;
```
Vector lengths fit in u64.

### ✅ SAFE: user.rs:1565,1797 — Length to u8/u64
```
    let num_bets = bet_types.len() as u64;
    num_bets: num_bets as u8,
```
Bet count is bounded by game rules (max 24 blocks). Fits in u8.

### ✅ SAFE: user.rs:1572,1574,1629 — u16-to-u64 widening
```
    BASE_MULTIPLIER as u64
    player_data.active_multiplier as u64
```
Widening casts, always safe.

### ✅ SAFE: user.rs:1616,1620 — u8-to-u64 widening for fee percentages
```
    global_config.sol_fee_config.protocol_fee_pct as u64,
    let stakers_fee = fee * global_config.sol_fee_config.stakers_pct as u64 / M_HUNDRED;
```
Widening from u8.

### ✅ SAFE: user.rs:1367-1396 — mul_div returns u128, cast to u64
```
    helper::mul_div(points, index as u64, INDEX_PRECISION)? as u64;
```
mul_div uses checked arithmetic internally. The `as u64` could truncate if result > u64::MAX, but in practice reward amounts won't exceed u64::MAX.

### ✅ SAFE: stake.rs:137,223,546,632 — u16-to-u64 widening for multiplier
```
    let weighted_amount = (actual_amount * multiplier as u64) / M_HUNDRED;
```
multiplier is u16 (max 65535). `actual_amount * 65535` could overflow u64 for very large stakes. BUT no checked_mul. **Note: if actual_amount > u64::MAX/65535 (~2.8e14 = 280T tokens), overflow occurs.** With 6 decimal tokens, this is 280B tokens. Unlikely but possible if token supply is huge. **Potential issue but very low probability.**

### ✅ SAFE: stake.rs:187-188,596-597 — Multiplier application
```
    let hashbeasts_multiplier = player_data.hashbeast_multiplier as u64;
    let weighted_amount_with_hashbeasts = (weighted_amount * hashbeasts_multiplier) / BASE_MULTIPLIER as u64;
```
Similar to above — could overflow for extreme values.

### ✅ SAFE: tax.rs:61,132-133 — u8-to-u64 widening for percentage sum
```
    (nft_floor_sweep_pct as u64) + (faction_treasury_pct as u64) + (burn_pct as u64)
```
u8 values summed as u64. Safe.

### ✅ SAFE: tax.rs:99,149 — u8 subtraction (potential underflow!)
```
    let vault_pct = M_HUNDRED as u8 - nft_floor_sweep_pct - faction_treasury_pct - burn_pct;
```
**Note:** If the sum of the three pcts exceeds M_HUNDRED (100), this underflows! But there's a require! check above ensuring sum <= 100. So guarded.

### ✅ SAFE: tax.rs:354,356,357 — helper::mul_div with as u64
Uses checked mul_div. Result truncation unlikely.

### ✅ SAFE: tax.rs:576,656 — MAX_FACTIONS as u8
Constant, fits in u8.

### ✅ SAFE: game.rs:325,397 — Random block selection
```
    ]) % NUM_BLOCKS as u64) as u8;
```
Result of `% NUM_BLOCKS` (24) fits in u8.

### ✅ SAFE: game.rs:352,853 — u64-to-u128 widening
```
    global_state.total_sol_bets + (game_session.total_sol_bets as u128);
```
Widening cast, safe.

### ✅ SAFE: game.rs:655,663,670,678 — u128 intermediate for percentage
```
    (minebtc_rewards as u128 * minebtc_winners_pct as u128 / 100) as u64;
```
Since pct < 100, result <= minebtc_rewards. Fits in u64.

### ✅ SAFE: economy.rs:439-444 — Price calculation with guards
```
    (sol_for_swap as u128)
        ...
        .checked_div(minebtc_received as u128)
        ...
        .min(u64::MAX as u128) as u64
```
Properly uses `.min(u64::MAX as u128)` before cast. Well guarded.

### ✅ SAFE: economy.rs:499-523 — TWAP calculation
Uses u128 intermediate. Final `.min(u64::MAX as u128) as u64` is properly guarded.

### ✅ SAFE: economy.rs:1321-1348 — LP amount calculations
Uses u128 intermediate. Results fit in u64 for reasonable values.

### ✅ SAFE: admin.rs:429-432 — u8-to-u16 widening for percentage sum
```
    let total = minebtc_stakers_pct as u16
        + minebtc_winners_pct as u16
        + minebtc_same_faction_pct as u16
        + minebtc_motherlode_pct as u16;
```
u8 values summed as u16. Max 1020. Safe.

### ✅ SAFE: admin.rs:757 — u8-to-u16 widening
```
    let total_pct: u16 = creators.iter().map(|c| c.percentage as u16).sum();
```
Sum of percentages, max ~100. Safe.

### ✅ SAFE: hashbeasts.rs:120,125 — u8-to-u64 widening
```
    hashbeast_config.hashbeasts_minted + mint_count as u64 <= hashbeast_config.max_supply,
```
Safe widening.

### ✅ SAFE: state.rs:511 — DAY_IN_SECONDS as i64
```
    pub const DISTRIBUTION_COOLDOWN_SECONDS: i64 = DAY_IN_SECONDS as i64;
```
DAY_IN_SECONDS = 86400. Fits in i64.

---

## SUMMARY OF ACTIONABLE ITEMS

### Must Fix (Potential Fund Impact):
1. **stake.rs:426-431** — Add `remaining_seconds > 0` guard before the `as u64` cast in the MineBTC unstake event emission section. Low real-world impact (event data only) but still a correctness bug.

2. **user.rs:761** — `(old_sol_balance as i64 + sol_diff) as u64` needs a guard: `max(0, ...)` or a require! to prevent negative result wrapping.

3. **user.rs:573,702,710** — Use `checked_mul` instead of `*` for SOL calculations to prevent silent overflow.

### Should Fix (Code Quality / Defense in Depth):
4. **epoch.rs:351, game.rs:881** — Compare timestamps consistently as i64 instead of casting clock to u64.

5. **epoch.rs:372, game.rs:909, user.rs:1823** — Store timestamps as i64 consistently or add validation that value is positive before `as u64`.

6. **economy.rs:995-1003** — Add `.min(u64::MAX as u128)` before `as u64` in LP price calculation.

### Already Fixed:
7. **helper.rs:427-445** — calculate_emergency_tax properly guards `remaining_seconds <= 0` before any cast. ✅

### Pattern Summary:
- Total casts found: ~249 across all files
- Critical (potential fund impact): 3 (items 1-3)
- High (correctness issues): 5 (timestamp consistency)
- Medium (edge case truncation): 6
- Low/Safe: ~235+

### Files Audited:
- ✅ instructions/helper.rs (550 lines)
- ✅ instructions/user.rs (3131 lines)
- ✅ instructions/stake.rs (1799 lines)
- ✅ instructions/game.rs (~900 lines)
- ✅ instructions/economy.rs (1692 lines)
- ✅ instructions/epoch.rs (518 lines)
- ✅ instructions/hashbeasts.rs (1694 lines)
- ✅ instructions/admin.rs (~950 lines)
- ✅ instructions/tax.rs (1205 lines)
- ✅ genescience.rs (1652 lines)
- ✅ state.rs (constants only)
- ✅ lib.rs (no casts)
- ✅ events.rs (no casts)
