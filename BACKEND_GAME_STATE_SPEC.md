# Backend Game-State JSON Specification

> **Purpose:** Define the exact 1-second game-state payload pushed from backend to frontend, including how every field is derived from on-chain events and account state. This doc is the single source of truth for the round lifecycle, fee math, reward distribution, and jackpot mechanics.

---

## 1. Round Lifecycle Overview

```
start_round(round_id=N)      → GameSession created, stage=0, betting OPEN
       │
       ▼ (players bet via join_bets / execute_autominer_bet)
   [BETTING PHASE]           → GameSession accumulates sol_bets, points_bets, wgtd_points_bets
       │
       ▼ (when clock.slot > scheduled_entropy_slot AND clock.timestamp >= round_end_timestamp)
end_round()                  → stage 0→1, entropy resolved, winner+direction picked,
                               minebtc split computed, jackpot_hit boolean set
       │
       ▼ (if jackpot_hit == true)
distribute_jackpot_rewards(round_id=N)
                              → drains global jackpot_pot into GameSession.jackpot_rewards_index
                               (idempotent via jackpot_distributed flag)
       │
       ▼
end_round_faction_rewards(war_id=X)
                              → stage 1→2, staker rewards distributed (or redirected to winners),
                               faction-war tracking updated, can_begin_round = true
       │
       ▼
start_round(round_id=N+1)    → next round begins
```

**Key invariants:**
- `stage == 0` → betting is open (frontend should show countdown + bet UI)
- `stage == 1` → round ended, winner known, rewards being computed (frontend shows result reveal)
- `stage == 2` → rewards finalized, claims available, new round can start (frontend shows claim UI + leaderboard)
- `can_begin_round == true` → cranker may call `start_round` for next round_id

---

## 2. Fee Flow (per SOL bet)

Every **gross** bet goes through this flow **inside `join_bets` / `execute_autominer_bet`**:

```
gross_bet_sol
    ├─► 5%  → war_sol_vault   (cycle jackpot reserve)
    ├─► 15% → protocol_fee
    │         ├─► referral_cut (if referrer exists)
    │         │     same-faction recruit  → 1.0% of gross
    │         │     cross-faction recruit → 0.5% of gross
    │         │
    │         └─► effective_fee = protocol_fee − referral_cut
    │               ├─► stakers_pct (default 20% of effective_fee)
    │               │     → sol_rewards_vault (stakers)
    │               │
    │               └─► treasury = effective_fee − stakers_fee
    │                     ├─► 70% buybacks     → sol_treasury (POL / buyback budget)
    │                     ├─► 3%  NFT MM       → inventory_sweep_vault
    │                     └─► 27% dev team     → sol_treasury (dev allocation)
    │
    └─► 80% → net_to_prize_pot
              → sol_prize_pot_vault (paid to exact winners on claim)
```

**Ticket bets** (free tickets) do NOT pay SOL fees. They contribute `points` and `wgtd_points` (points = ticket value, no multiplier) to the round but add `0` to `total_sol_bets`. The 25% ticket-points cap is enforced per-bet.

### How backend derives fee fields for the JSON

| JSON Field | Source | Derivation |
|---|---|---|
|`ongoing_session.total_sol_bets`| `GameSession.total_sol_bets` | Sum of all net SOL bets (after 5% cycle split, before protocol fee) |
|`ongoing_session.stakers_fee`| `GameSession.stakers_fee` | Accumulated from each bet: `effective_fee * stakers_pct / 100` |
|`latest_result.sol_protocol_fee.buybacks`| Treasury × 70% | Read `TaxConfig` / treasury events, or track cumulative |
|`latest_result.sol_protocol_fee.dev_fee`| Treasury × 27% | Same as above |
|`latest_result.sol_protocol_fee.compute_fee`| Treasury × 3% | NFT market-making sweep budget |
|`latest_result.sol_staking_yield`| `sol_rewards_index` | Index-based: `sol_rewards_index * winning_points / INDEX_PRECISION` per winner |

---

## 3. dBTC Emission & Distribution (per round)

At `end_round`, the contract reads `DegenBtcMining.dbtc_per_round` (the current emission rate) and splits it:

```
dbtc_per_round
    ├─► winners_pct  (default 50%)  → dbtc_winner_pool
    │                                 (exact faction+direction winners)
    ├─► same_faction_pct (default 20%) → dbtc_same_faction_direction_pools
    │                                 (each losing direction on winning faction)
    ├─► stakers_pct  (default 5%)   → faction_stakers
    │                                 (degenBTC + LP stakers on winning faction)
    └─► jackpot_pct  (default 5%)   → jackpot_rewards
                                      (added to global_state.jackpot_pot)
```

**Same-faction redirect:** If a losing direction has zero bettors, its allocation is **redirected** to the exact-winners pool. This prevents stranded tokens.

**Staker redirect:** If the winning faction has zero active stakers, the staker share is redirected to exact winners by increasing `sol_rewards_index` and `dbtc_rewards_index`.

### Reward indexes (set at end_round)

- `sol_rewards_index = total_sol_bets * INDEX_PRECISION / winning_points`
- `dbtc_rewards_index = dbtc_winner_pool * INDEX_PRECISION / winning_wgtd_points`

At claim time, a user's reward = `their_points * sol_rewards_index / INDEX_PRECISION` (SOL) and `their_wgtd_points * dbtc_rewards_index / INDEX_PRECISION` (dBTC).

---

## 4. Jackpot Mechanics

> **Important:** The jackpot is paid in **dBTC only**, not SOL. Every round accumulates dBTC into the global pot.

### Accumulation
Every round, `jackpot_rewards` (5% of `dbtc_per_round` emission) is added to `GlobalGameState.jackpot_pot`. The pot grows in dBTC across rounds until hit.

### Hit chance
`jackpot_random = hash_bytes[8..12] % 625` → hit if `== 0` (~0.16% per round).

### Faction selection (inverse-volume weighting)
When hit, the jackpot faction is selected **independently** of the round winner:
- Each faction's weight = `5000 + (10000 − bet_share_bps)`
- Lower bet volume → higher weight (underdogs favored)
- Roll = `hash_bytes[12..16] % total_weight`
- Cumulative walk selects the faction

### Distribution (`distribute_jackpot_rewards`)
- **Eligibility:** ANY bettor on the jackpot faction, **any direction** (not just exact winner)
- `jackpot_rewards_index = jackpot_pot * INDEX_PRECISION / total_wgtd_points_on_jackpot_faction`
- The **entire** accumulated pot is drained into the round's `jackpot_rewards_index`
- If no bettors on jackpot faction → pot **rolls over** (`JackpotRolledOver` event), `jackpot_distributed = true`, pot stays in global state for next hit

### Claim
Jackpot is claimed automatically in `calculate_round_rewards` when user calls `claim_round_rewards`:
```rust
if jackpot_hit && jackpot_rewards_index > 0 && faction_id == jackpot_faction_id {
    jackpot_reward = wgtd_points_bet_on_faction * jackpot_rewards_index / INDEX_PRECISION
}
```

---

## 5. Event-to-JSON Mapping

### Events the backend MUST listen for

| Event | Emitted By | When | JSON Section Updated |
|---|---|---|---|
|`RoundStarted`| `start_round` | New round begins | `macro`, `ongoing_session` |
|`BetsPlaced`| `join_bets` / `execute_autominer_bet` | Each bet batch | `ongoing_session` (live bet totals) |
|`RoundEnded`| `end_round` | Winner resolved | `latest_result`, `latest_jackpot_result` |
|`JackpotHit`| `distribute_jackpot_rewards` | Jackpot paid out | `latest_jackpot_result` |
|`JackpotRolledOver`| `distribute_jackpot_rewards` | No eligible bettors | `latest_jackpot_result` |
|`JackpotNearMiss`| `end_round` | Random was ≤10 but not 0 | Frontend notification only |
|`RewardsDistributedForRound`| `end_round_faction_rewards` | Stage 2 reached | Confirms `latest_result` finality |
|`DegenBtcStakingRewardsDistributed`| `distribute_rewards_amg_stakers` | Staker indexes updated | `latest_result.sol_staking_yield` (indirect) |
|`LpStakingRewardsDistributed`| `distribute_rewards_amg_stakers` | LP indexes updated | Same |
|`GameplayScoreAccumulated`| `track_war_round_completion` / claim mutation | Faction score changes | `ongoing_session_mutations` |
|`FactionWarAutoSettled`| `track_war_round_completion` | Cycle ended | `macro.faction_war` state |

### State accounts the backend MUST poll / subscribe to

| Account | PDA Seed | Fields Used |
|---|---|---|
|`GlobalGameState`| `b"global-game-state"` | `current_round_id`, `can_begin_round`, `jackpot_pot`, `last_round_id`, `winning_faction_id` |
|`GameSession`| `b"game-session" + round_id_le` | All round-specific fields (bets, winner, indexes, stage, mutations) |
|`DegenBtcMining`| `b"mine-btc-mining"` | `dbtc_per_round`, `total_tokens_mined`, `pol_stats.lp_operations_count` |
|`FactionWarConfig`| `b"faction-war-config"` | `current_war_id`, `is_active`, `settle_at_lp_op_count` |

---

## 6. Game-State JSON Schema

```jsonc
{
  "_meta": {
    "timestamp_ms": 1715078400000,
    "slot": 285643210,
    "data_source": "events+account_poll"
  },

  "macro": {
    "dbtc_emissions_per_round": 1250000000,
    "dbtc_emissions_change_pct": 0,
    "duration_secs": 60,
    "current_round_id": 482,
    "prev_winning_faction_id": 3,
    "jackpot_pot": 18475000000,
    "can_begin_round": true,
    "is_paused": false,
  },
  "next_tx": {
    "expected_instruction": "start_round",
    "reason": "stage=2 && can_begin_round=true",
  },
  "ongoing_session": {
    "round_id": 482,
    "round_start_timestamp": 1715078340,
    "round_end_timestamp": 1715078400,
    "scheduled_entropy_slot": 285643180,
    "stage": 2,
    "total_sol_bets": 12500000000,
    "total_tickets_used": 47,
    "tickets_dist": {
      "0.001": 12,
      "0.01": 25,
      "0.1": 10
    },
    "total_wgtd_points_bets": 15800000000,
    "stakers_fee": 187500000,
    "sol_bets_by_faction_direction": [
      [0, 1500000000, 800000000],
      [500000000, 0, 1200000000],
      ...
    ],
    "wgtd_points_bets_by_faction_direction": [
      [0, 2100000000, 1120000000],
      [700000000, 0, 1680000000],
      ...
    ],
    "war_id_when_played": 17
  },

  "ongoing_session_mutations": {
    "mutations_per_faction": [2, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "total_mutations_this_round": 3,
    "winning_faction_volume_at_round": 4500000000
  },

  "latest_result": {
    "round_id": 482,
    "round_end_timestamp": 1715078400,
    "entropy_hash": "a1b2c3d4...",
    "used_entropy_fallback": false,
    "winning_faction_id": 3,
    "winning_direction": 2,
    "total_sol_bets": 12500000000,
    "total_wgtd_points_bets": 15800000000,
    "points_by_faction_direction": [
      [0, 1500000000, 800000000],
      [500000000, 0, 1200000000],
      ...
    ],    
    "wgtd_points_bets_by_faction_direction": [
      [0, 2100000000, 1120000000],
      [700000000, 0, 1680000000],
      ...
    ],
    "dbtc_winner_pool": 625000000,
    "dbtc_same_faction_direction_pools": [0, 250000000, 250000000],
    "faction_stakers": 62500000,
    "jackpot_rewards": 62500000,
    "sol_rewards_index": 5952380,
    "dbtc_rewards_index": 3720238,
    "sol_protocol_fee": {
      "buybacks": 1312500000,
      "dev_fee": 506250000,
      "compute_fee": 56250000
    },
    "sol_staking_yield": {
      "degenbtc_lane_index": 0,
      "lp_lane_index": 0
    }
  },

  "latest_jackpot_result": {
    "round_id": 482,
    "round_end_timestamp": 1715078400,
    "jackpot_hit": true,
    "jackpot_faction_id": 1,
    "winning_faction_id": 3,
    "jackpot_pot_size_on_hit": 18475000000,
    "jackpot_rewards_index": 2345678,
    "jackpot_distributed": true,
    "rolled_over": false
  }
}
```

### Field descriptions

#### `macro`
| Field | Type | Source | Description |
|---|---|---|---|
|`dbtc_emissions_per_round`| `u64` | `DegenBtcMining.dbtc_per_round` | Current round emission in base units (6 decimals) |
|`dbtc_emissions_change_pct`| `i8` | Computed from `DegenBtcMining` history | +1% on price rise above 3% threshold, −3% on fall |
|`duration_secs`| `u16` | `GlobalGameState.round_duration_seconds` | Round length (typically 60) |
|`current_round_id`| `u64` | `GlobalGameState.current_round_id` | Active / most recently started round |
|`prev_winning_faction_id`| `u8` | `GlobalGameState.winning_faction_id` | Winner of the last completed round |
|`jackpot_pot`| `u64` | `GlobalGameState.jackpot_pot` | Global jackpot pot in dBTC base units (accumulated across rounds, paid in dBTC) |
|`can_begin_round`| `bool` | `GlobalGameState.can_begin_round` | Whether `start_round` can be called |
|`is_paused`| `bool` | `GlobalConfig.is_paused` | Kill-switch status |
|`faction_war`| `object` | `FactionWarConfig` | Current cycle ID, active flag, settle cycle |

#### `next_tx`
| Field | Type | Derivation | Description |
|---|---|---|---|
|`expected_instruction`| `string` | Logic based on stage | One of: `start_round`, `end_round`, `distribute_jackpot_rewards`, `end_round_faction_rewards` |
|`reason`| `string` | Human-readable | Why this instruction is next |
|`round_id`| `u64` | Target round | Round the next tx should act on |
|`ETA_seconds`| `i64` | `round_end_timestamp − now` | Seconds until next action (negative = overdue) |

**Next-tx logic:**
```
if stage == 0 && now >= round_end_timestamp && slot > scheduled_entropy_slot:
    expected = "end_round"
elif stage == 1 && !jackpot_distributed && jackpot_hit:
    expected = "distribute_jackpot_rewards"
elif stage == 1:
    expected = "end_round_faction_rewards"
elif stage == 2 && can_begin_round:
    expected = "start_round"
```

#### `ongoing_session`
| Field | Type | Source | Description |
|---|---|---|---|
|`round_id`| `u64` | `GameSession.round_id` | Current round |
|`round_start_timestamp`| `i64` | `GameSession.round_start_timestamp` | Unix timestamp when round started |
|`round_end_timestamp`| `i64` | `GameSession.round_end_timestamp` | Unix timestamp when betting closes |
|`scheduled_entropy_slot`| `u64` | `GameSession.scheduled_entropy_slot` | Slot whose hash resolves the round |
|`stage`| `u8` | `GameSession.stage` | 0=betting, 1=ended, 2=finalized |
|`total_sol_bets`| `u64` | `GameSession.total_sol_bets` | Total net SOL in round (after cycle split) |
|`total_tickets_used`| `u32` | Count from `BetsPlaced` events | Number of ticket-backed bets this round |
|`tickets_dist`| `object` | Aggregated from `BetsPlaced` | Count per ticket tier used |
|`total_wgtd_points_bets`| `u64` | `GameSession.total_wgtd_points_bets` | Total weighted points (multiplier-applied) |
|`stakers_fee`| `u64` | `GameSession.stakers_fee` | Accumulated SOL staker fees |
|`sol_bets_by_faction_direction`| `[u64][3]` | `GameSession.sol_bets_by_faction` × direction | Per-faction, per-direction SOL volume |
|`wgtd_points_bets_by_faction_direction`| `[u64][3]` | `GameSession.wgtd_points_bets_by_faction_direction` | Per-faction, per-direction weighted points |
|`war_id_when_played`| `u64` | `GameSession.war_id_when_played` | Cycle ID at round start (for late-claim detection) |

#### `ongoing_session_mutations`
| Field | Type | Source | Description |
|---|---|---|---|
|`mutations_per_faction`| `[u8; 15]` | `GameSession.mutations_per_faction` | Count of story events per faction this round |
|`total_mutations_this_round`| `u8` | `GameSession.total_mutations_this_round` | Total mutations across all factions |
|`winning_faction_volume_at_round`| `u64` | `GameSession.winning_faction_volume_at_round` | Snapshotted SOL volume on winner (for mutation-roll volume_factor) |

#### `latest_result`
| Field | Type | Source | Description |
|---|---|---|---|
|`round_id`| `u64` | `GameSession.round_id` | Completed round |
|`round_end_timestamp`| `i64` | `GameSession.round_end_timestamp` | When betting closed |
|`entropy_hash`| `string` | `GameSession.entropy_hash` | Hex of slot hash used for randomness |
|`used_entropy_fallback`| `bool` | `GameSession.used_entropy_fallback` | True if scheduled slot aged out |
|`winning_faction_id`| `u8` | `GameSession.winning_faction_id` | Winning faction |
|`winning_direction`| `u8` | `GameSession.winning_direction` | 0=Down, 1=Neutral, 2=Up |
|`total_sol_bets`| `u64` | `GameSession.total_sol_bets` | Total SOL in this round |
|`total_wgtd_points_bets`| `u64` | `GameSession.total_wgtd_points_bets` | Total weighted points |
|`wgtd_points_bets_by_faction_direction`| `[u64][3]` | `GameSession.wgtd_points_bets_by_faction_direction` | Full breakdown for frontend charts |
|`dbtc_winner_pool`| `u64` | `GameSession.dbtc_winner_pool` | dBTC for exact winners |
|`dbtc_same_faction_direction_pools`| `[u64; 3]` | `GameSession.dbtc_same_faction_direction_pools` | dBTC per losing direction on winner |
|`faction_stakers`| `u64` | `GameSession.faction_stakers` | dBTC for stakers (or redirected) |
|`jackpot_rewards`| `u64` | `GameSession.jackpot_rewards` | dBTC added to global jackpot this round |
|`sol_rewards_index`| `u128` | `GameSession.sol_rewards_index` | SOL index for exact winners |
|`dbtc_rewards_index`| `u128` | `GameSession.dbtc_rewards_index` | dBTC index for exact winners |
|`sol_protocol_fee`| `object` | Computed from `BetsPlaced` events + config | Breakdown of where protocol SOL went |
|`sol_staking_yield`| `object` | Computed from staker index updates | Current yield indexes for degenBTC + LP lanes |

#### `latest_jackpot_result`
| Field | Type | Source | Description |
|---|---|---|---|
|`round_id`| `u64` | `GameSession.round_id` | Round where jackpot occurred |
|`round_end_timestamp`| `i64` | `GameSession.round_end_timestamp` | Same as result |
|`jackpot_hit`| `bool` | `GameSession.jackpot_hit` | Whether jackpot fired this round |
|`jackpot_faction_id`| `u8` | `GameSession.jackpot_faction_id` | Faction that won the jackpot (may differ from round winner) |
|`winning_faction_id`| `u8` | `GameSession.winning_faction_id` | Actual round winner (for comparison) |
|`jackpot_pot_size_on_hit`| `u64` | `GameSession.jackpot_pot_size_on_hit` | Size of pot when hit (0 if rolled over) |
|`jackpot_rewards_index`| `u128` | `GameSession.jackpot_rewards_index` | Index for jackpot claimers (0 if no bettors) |
|`jackpot_distributed`| `bool` | `GameSession.jackpot_distributed` | Whether `distribute_jackpot_rewards` ran |
|`rolled_over`| `bool` | `JackpotRolledOver` event | True if hit but no eligible bettors |

---

## 7. How Backend Builds This (Implementation Notes)

### Polling strategy
1. **Every 1s:** Fetch `GlobalGameState`, `GameSession(current_round_id)`, `DegenBtcMining`, `FactionWarConfig`
2. **Event listener:** Stream all program events via WebSocket / gRPC. Buffer `BetsPlaced` events to update live bet totals between polls.
3. **State machine:** Track the current `stage`. When stage transitions (0→1 or 1→2), trigger a full refresh and push update immediately.

### Computing `sol_protocol_fee` from events
For each `BetsPlaced` event in the round:
```python
for bet in event.net_amounts:
    cycle_split = bet * cycle_sol_split_pct / 100
    gross = bet + cycle_split  # reverse to gross
    protocol_fee = gross * protocol_fee_pct / 100
    
    referral_cut = compute_referral(gross, player_data.referrer_faction_id)
    effective_fee = protocol_fee - referral_cut
    stakers_fee = effective_fee * stakers_pct / 100
    treasury = effective_fee - stakers_fee
    
    buybacks += treasury * 70 / 100
    dev_fee += treasury * 27 / 100
    compute_fee += treasury * 3 / 100
```

> **Note:** The jackpot pot (`GlobalGameState.jackpot_pot`) is **dBTC only** — it is NOT part of the SOL fee flow. It accumulates from the 5% dBTC emission split each round and is paid out as dBTC when hit. Do not confuse it with the SOL prize pot (`sol_prize_pot_vault`) which pays exact winners in SOL.

### Ticket tracking
`BetsPlaced.used_ticket` and `ticket_type_index` tell you which tier was consumed. Map ticket value (from `GlobalConfig.ticket_tiers`) to the distribution counts.

### Jackpot pot display
If the frontend wants to show the jackpot pot in fiat/SOL terms, convert `GlobalGameState.jackpot_pot` (dBTC base units) using the current dBTC/SOL price from the Raydium pool. The pot itself is always denominated in dBTC on-chain.

---

## 8. Edge Cases

| Scenario | Handling |
|---|---|
| Empty round (no bets) | `total_users == 0`, winner picked randomly, all pools = 0, stage goes directly to 2 |
| Jackpot hit but no bettors on that faction | `JackpotRolledOver` emitted, pot stays in global state, `jackpot_rewards_index = 0` |
| Same-faction direction with no bettors | Unallocated share redirected to exact-winners pool automatically |
| Winning faction has no stakers | Staker rewards redirected to exact winners via index bump |
| Scheduled entropy slot ages out | `used_entropy_fallback = true`, latest slot hash used instead |
| Round finalized but jackpot not distributed | Backend should still show `jackpot_hit=true` and `jackpot_distributed=false`. `distribute_jackpot_rewards(round_id)` can be called retroactively now. |
| Faction war settled mid-round | `war_state` may be empty/seed. Backend shows `is_active=false` until next bet initializes new cycle. |

---

*Last updated: 2025-05-07*
