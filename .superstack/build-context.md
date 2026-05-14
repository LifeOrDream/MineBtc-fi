# Build Context

Most recent review snapshot. Used by `/review`-style flows to track
outstanding findings vs. what has been resolved. Update via `/review` or
`/learn`; do not edit by hand without good reason.

```json
{
  "review": {
    "security_score": "B+",
    "quality_score": "A-",
    "findings": [
      {
        "severity": "P2",
        "category": "SBF dependency warning",
        "description": "cargo build-sbf and anchor build now pass for minebtc, but Solana's post-processing still reports an upstream mpl-core hooked/plugin stack frame at 4184 bytes, above the 4096-byte SBF stack guidance.",
        "fix": "Before mainnet, either upgrade/patch mpl-core or replace the remaining mpl-core helper paths with slimmer local instruction/account parsing so the SBF build is free of dependency stack warnings."
      }
    ],
    "ready_for_mainnet": false,
    "notes": "2026-05-14 marketplace/floor/lootbox audit pass. Fixed base marketplace stale reclaim to check escrow ownership instead of seller ownership; tightened MineBTC floor wrappers around listing PDA, escrow PDA, collection, and asset binding; hardened floor snapshots with 17-sale minimum, day-zero marketplace-min bootstrap, queue/prior-anchor upward caps, and live median-entry escrow validation; added fresh-anchor guards for sweep/relist decisions; clamped program relists to marketplace min; made rebirth/lootbox delivery require the canonical HashBeast collection; and updated marketplace docs/index notes. cargo check -p minebtc -p degenbtc_market, cargo test -p minebtc -p degenbtc_market (160 total tests), anchor build, node --check setup_scripts/do_txs.js, and targeted git diff --check pass. Mainnet readiness is held only on the remaining upstream mpl-core stack warning; devnet iteration is acceptable."
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
