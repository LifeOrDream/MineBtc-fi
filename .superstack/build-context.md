# Build Context

Most recent review snapshot. Used by `/review`-style flows to track
outstanding findings vs. what has been resolved. Update via `/review` or
`/learn`; do not edit by hand without good reason.

```json
{
  "review": {
    "security_score": "C+",
    "quality_score": "B",
    "findings": [
      {
        "severity": "P2",
        "category": "SBF dependency warning",
        "description": "cargo build-sbf and anchor build now pass for minebtc, but Solana's post-processing still reports an upstream mpl-core hooked/plugin stack frame at 4184 bytes, above the 4096-byte SBF stack guidance.",
        "fix": "Before mainnet, either upgrade/patch mpl-core or replace the remaining mpl-core helper paths with slimmer local instruction/account parsing so the SBF build is free of dependency stack warnings."
      }
    ],
    "resolved_findings": [
      {
        "severity": "P1",
        "category": "NFT marketplace inventory payment",
        "resolution": "Resolved 2026-05-14: degenbtc_market list_nft/cancel_listing/buy_listing now split SOL/rent payer from asset seller/buyer. MineBTC sweep/relist/expire inventory flows use inventory_sweep_vault as payer and inventory_pda only as NFT owner/recipient."
      },
      {
        "severity": "P1",
        "category": "NFT marketplace inventory accounting",
        "resolution": "Resolved 2026-05-14: inventory_finalize_sale now derives the canonical marketplace escrow PDA and refuses to finalize while the asset is still owned by inventory_pda or marketplace escrow."
      },
      {
        "severity": "P2",
        "category": "NFT marketplace proceeds routing",
        "resolution": "Resolved 2026-05-14: handle_inventory_proceeds no longer invokes System Program transfers from program-owned inventory_pda; it mutates lamports directly while preserving rent exemption."
      },
      {
        "severity": "P1",
        "category": "Tax vault routing",
        "resolution": "Resolved 2026-05-14: crank_distribute_tax and claim_faction_treasury_for_faction_war now require the canonical DegenBtcMining vault, vault-authority PDA, mint, and owner constraints before moving tax/reward tokens."
      },
      {
        "severity": "P1",
        "category": "Token-2022 post-fee accounting",
        "resolution": "Resolved 2026-05-14: faction treasury attribution and staking index updates now credit post-transfer-fee delivered amounts instead of pre-fee transfer inputs."
      },
      {
        "severity": "P2",
        "category": "Economy LP crank dust handling",
        "resolution": "Resolved 2026-05-14: add_lp_and_burn now returns WSOL and clears lp_operation_pending when pool-ratio/slippage limits reduce the LP operation to zero."
      },
      {
        "severity": "P1",
        "category": "User reward-claim accounting",
        "resolution": "Resolved 2026-05-14: round and autominer claim paths now reject malformed UserGameBet vector state, avoid panic unwrap/expect paths, require exact SOL prize-pot availability before payout/reload, and use checked autominer reload math instead of truncating u64 rounds into u32."
      },
      {
        "severity": "P1",
        "category": "Staking vault authority constraints",
        "resolution": "Resolved 2026-05-14: staking reward withdrawal paths now require canonical DegenBtcMining vault keys, vault-authority ownership, stored vault-auth bumps, writable mining state for distribution accounting, and custodian authority checks on unstake."
      },
      {
        "severity": "P2",
        "category": "HashBeast lifecycle constraints",
        "resolution": "Resolved 2026-05-14: HashBeast mint/stake/unstake/rebirth/breed flows now pin MPL Core program and canonical collection accounts more consistently, require admin-mint asset signing, prevent duplicate passive-stake entries, and validate ticket-vector alignment."
      },
      {
        "severity": "P1",
        "category": "Faction-war ranking overflow",
        "resolution": "Resolved 2026-05-14: faction-war rankings now compare u64 gameplay scores directly instead of casting to i64, and total score logging uses checked summation."
      },
      {
        "severity": "P1",
        "category": "Canonical faction-state routing",
        "resolution": "Resolved 2026-05-14: settle_round and faction treasury claims now require the canonical FactionState PDA for the winning/claimed faction, preventing stale or forged same-id faction accounts from receiving reward-index updates."
      },
      {
        "severity": "P1",
        "category": "Faction-war cycle boundary capture",
        "resolution": "Resolved 2026-05-14: add_lp_and_burn no longer snapshots a previous cycle's global current_round_id before the first current-cycle round is processed; first-round threshold crossings are captured lazily during settle_round."
      },
      {
        "severity": "P2",
        "category": "Faction-war reward lane conservation",
        "resolution": "Resolved 2026-05-14: SOL base/HB/MVP lanes now drain to treasury when the matching dBTC lane rounds to zero, and MVP claims aggregate all matching MVP slots for the same user."
      },
      {
        "severity": "P2",
        "category": "Economy oracle arithmetic",
        "resolution": "Resolved 2026-05-14: price-change calculations now clamp oversized percentage moves instead of wrapping through lossy i64/i32 casts."
      },
      {
        "severity": "P2",
        "category": "HashBeast breed account stack",
        "resolution": "Resolved 2026-05-14: BreedHashBeast moved several heavy Anchor account constraints into handler-side checks, clearing the project-owned SBF stack warning while preserving collection/vault/mint validation."
      }
    ],
    "ready_for_mainnet": false,
    "notes": "2026-05-14 follow-up NFT marketplace fixes on commit 20c51a0. The three marketplace blockers from that pass are fixed. 2026-05-14 game/economy/faction-war/tax audit then hardened canonical tax vault routing, Token-2022 post-fee accounting, LP dust handling, RPG-disabled reward docs, and setup script account wiring. 2026-05-14 user/stake/hashbeasts audit hardened reward-claim vector integrity, autominer reload accounting, staking vault authority constraints, and HashBeast MPL Core/collection constraints. Final 2026-05-14 pass fixed u64 ranking overflow, canonical FactionState checks in round/tax distribution, SOL-lane dust conservation, first-round cycle-boundary capture, price-change cast clamping, and the project-owned BreedHashBeast SBF stack warning. cargo fmt, cargo check -p minebtc, cargo check -p degenbtc_market, cargo test -p minebtc, cargo test -p degenbtc_market --lib, anchor build, node --check setup_scripts/do_txs.js, node --check setup_scripts/3_init_mineBTC.js, and git diff --check pass locally. anchor build still emits the existing upstream mpl-core hooked/plugin stack warning, so keep mainnet signoff gated on a clean SBF dependency story or explicit acceptance of that dependency risk."
  },
  "debug": {
    "issues_resolved": [
      {
        "error": "UnstakeMinebtc failed with Anchor ConstraintSeeds on user_position.",
        "cause": "Existing StakedPosition accounts stored bump=0, so unstake constraints using bump=user_position.bump expected a non-existent PDA instead of the canonical position PDA.",
        "fix": "Use the canonical account-constraint bump for unstake and store ctx.bumps.user_position when initializing new MineBTC/LP staking positions."
      }
    ],
    "last_debug_session": "2026-05-11T23:45:58Z"
  }
}
```
