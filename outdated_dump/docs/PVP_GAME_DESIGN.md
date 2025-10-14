# MoonDoge PvP Game Design

> **Version 0.1 – Draft for internal review**  
> Last updated: 2025-06-19

---

## 1  Vision

MoonDoge PvP transforms passive moon-base building into a **high-stakes, casino-flavoured battle arena**.  Players wager SOL, attack opponents' economic modules, and siphon value (XP, DOGE_BTC, hash-power) while fighting for a **winner-takes-most** prize pot.  Damage persists until repaired, creating long-term rivalry loops.

_Target player:_ **Solana degen / gambler** who enjoys real economic risk and big dopamine spikes.

---

## 2  Session Flow
| Step | Description |
|------|-------------|
| **1 Lobby** | Player selects a ticket tier (0.1 → 999 SOL) and queues.  Moon-base must have **≥ 1 000 HP** across active modules.  Ticket amount is escrowed to the **PvPGame** PDA. |
| **2 Matchmaking** | Players are paired within the same tier.<br>Pot = `2 × ticket – 10 % treasury fee`. |
| **3 Battle (turn-based)** | Each 5-min turn the attacker chooses a module type to target and fires one Attack module.<br>• Damage is applied → HP & module efficiency drop instantly.<br>• Special *casino* effects (XP steal, loot steal, hash-leech, ammo explosion) may trigger. |
| **4 Victory** | Fight ends when opponent's **total remaining HP ≤ 0** *or* 15 turns expire and one side has inflicted more damage. |
| **5 Payout & Persistence** | Winner receives **90 % of pot**; 10 % remains burned/treasury.<br>All damage stays until repaired by owner (SOL sink). |

---

## 3  Per-Module Combat Effects
| Target | Primary Effect | Casino Twist | Economic Impact (persists) |
|--------|----------------|--------------|---------------------------|
| **Attraction** | Lose HP → XP/h reduced | 5 % chance to **double stolen XP** | Attacker steals `HP_dmg / max_HP × hourly_XP` immediately. |
| **Research** | HP loss lengthens cooldown | Rolls the lab's loot table for attacker (DOGE_BTC) | Successful roll deducts reward from defender's lab vault. |
| **Mining** | HP loss drops defender hash-power | **Hash Leech**: attacker gains 10 % of lost hash-power for match (stacks ≤ 50 %). | Defender earns fewer DOGE_BTC until repaired. |
| **Attack** | HP + missile clip reduced | 2 % chance for **magazine explosion** (+25 % dmg) | Defender's DPS lowered mid-match. |
| **Defense (future)** | Shield HP reduced | – | Subsequent hits deal +5 % dmg. |

**HP efficiency rule:** `output = base × (current_HP / max_HP)` with a floor of **10 %** when HP = 0.

---

## 4  Damage, Ammo & Repair
* **Damage formula:** `dmg_actual = base_dmg × missiles × random(0.9 – 1.1)`
* **Ammo:** Each shot burns 1 missile; reload time comes from upgraded `AttackStats`.
* **Repair cost:** `missing_HP × 0.001 SOL` (governable).
* **Cooldown:** 4-hour free repair cooldown or pay immediately.

---

## 5  Ticket Tiers & Multipliers
| Tier | SOL Range | XP-Steal × | Loot-Steal × | Hash-Leech × |
|------|-----------|-----------|--------------|--------------|
| **Micro** | 0.1 – 1 | 1.0 | 1.0 | 1.0 |
| **Standard** | 1 – 10 | 1.75 | 1.75 | 1.3 |
| **High-Roller** | 10 – 100 | 3.0 | 3.0 | 2.0 |
| **Whale** | 100 – 500 | 5.0 | 5.0 | 3.0 |
| **Kraken** | > 500 | 8.0 | 8.0 | 4.0 |

**Formula:** `mult = min(1 + log10(ticket_SOL), 8)`; applied after base calculations & bounded by global steal limits.

---

## 6  Implementation Checklist (Code-Side)
- [ ] Add **HP ≥ 1 000 gate** in `create_pvp_game_internal`.
- [ ] Extend `AttackStats` with `missiles_left` tracking (already present).
- [ ] Implement per-target **effect handlers** inside `pvp_attack_turn_internal`:
  - [ ] `apply_damage()` – generic HP reduction & efficiency update.
  - [ ] `steal_xp()` – attraction logic.
  - [ ] `steal_research_loot()` – research logic.
  - [ ] `hash_leech()` – mining logic.
- [ ] Emit new events: `PvPAttackResolved`, `XpStolen`, `LootStolen`, `HashLeech`.
- [ ] Update helper to recalc **global_hashpower** when mining HP changes.
- [ ] Add **repair function** callable after cooldown with SOL cost.
- [ ] Integrate **leaderboard** into `LevelStats` for PvP wins.

---

## 7  Economic Balancing Knobs
| Parameter | Default | Tuning Notes |
|-----------|---------|--------------|
| Ticket treasury cut | 10 % | Increase to fight inflation. |
| Attack XP steal cap | 25 % of hourly XP | Prevent runaway snowballing. |
| Research loot steal cap | 50 % of lab's max_reward | High risk / high reward. |
| Hash leech max | 50 % | Keeps griefing under control. |
| Repair cost | 0.001 SOL / HP | Adjust with SOL price volatility. |

## 7-A  Ticket-Size Reward Multipliers  
_Incentivise whales without breaking balance._  

| Ticket Tier | SOL Range | Multiplier on XP Steal | Multiplier on Research Loot | Multiplier on Hash-Leech | Notes |
|-------------|-----------|------------------------|-----------------------------|---------------------------|-------|
| **Micro**   | 0.1 – 1 SOL | 1.0× | 1.0× | 1.0× | Entry-level, learning zone |
| **Standard**| 1 – 10 SOL | 1.75× | 1.75× | 1.3× | Warming-up stakes |
| **High-Roller** | 10 – 100 SOL | 3.0× | 3.0× | 2.0× | Degens playground |
| **Whale** | 100 – 500 SOL | 5.0× | 5.0× | 3.0× | Serious money, serious rewards |
| **Kraken** | > 500 SOL | 8.0× | 8.0× | 4.0× | Spectacle fights, streamer bait |

**Implementation Hint**  
`multiplier = 1.0 + log10(ticket_sol) * 1.0` (capped at 8×)  
This preserves smooth scaling while letting big tickets feel **insanely juicy**.

These multipliers apply _after_ normal calculations:
* **XP Steal**: `base_steal × ticket_mult`, then capped by `Attack XP steal cap`.
* **Research Loot**: `loot_roll_reward × ticket_mult` (still bounded by lab vault & steal cap).
* **Hash-Leech**: `hashpower_stolen × ticket_mult` but never exceeds global 50 % max.

This keeps low-stake games fun while making high-stake battles _meaningfully_ more lucrative for degens willing to risk larger tickets.

---

## 8  Security & Abuse Mitigation
1. **Time Limits** – 5 min turn timeout avoids griefing.
2. **SOL Stake** – Sybil-resistant; bots must risk capital.
3. **HP Floor (10 %)** – Modules never fully disabled; prevents total shutdown farming.
4. **Match Caps** – One active PvP game per user.
5. **Randomness** – Use `keccak::hashv(slot,… )` as in research logic.

---

## 9  Future Extensions
- **Squad Battles** – 3 vs 3 moonbases.
- **NFT Doge Heroes** – Equip NFTs for attack/defense buffs.
- **Seasonal Leagues** – Monthly resets with escalating ticket tiers.
- **Borrowable Hashpower** – Flash-loan style wager boosts.

---

### 🚀 Let's build & iterate!
This design keeps Solana degen players hooked through **wagering, sabotage, random jackpots, and ever-scaling progression**.  
Feedback and balance testing are welcome before solidifying the on-chain implementation. 