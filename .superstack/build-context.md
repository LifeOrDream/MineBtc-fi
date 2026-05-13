# Build Context

Most recent review snapshot. Used by `/review`-style flows to track
outstanding findings vs. what has been resolved. Update via `/review` or
`/learn`; do not edit by hand without good reason.

```json
{
  "review": {
    "security_score": "B+",
    "quality_score": "B+",
    "findings": [
      {
        "severity": "P2",
        "category": "SBF dependency warning",
        "description": "cargo build-sbf and anchor build now pass for minebtc, but Solana's post-processing still reports an upstream mpl-core hooked/plugin stack frame at 4184 bytes, above the 4096-byte SBF stack guidance.",
        "fix": "Before mainnet, either upgrade/patch mpl-core or replace the remaining mpl-core helper paths with slimmer local instruction/account parsing so the SBF build is free of dependency stack warnings."
      }
    ],
    "ready_for_mainnet": false,
    "notes": "2026-05-14 follow-up audit of game.rs, faction_war.rs, economy.rs, user.rs, and related HashBeast collection custody paths. Previous P0/P1 findings are fixed in current code: POL SOL movement is bound to the configured Raydium pool and canonical WSOL ATA; snapshot_price has minimum output/deviation guards; mutation counters saturate; war claim payout enforces HB score invariants; and HashBeast stake/unstake now require the bound collection account. This pass also boxed AddLpAndBurn account wrappers to remove the program-owned try_accounts SBF stack warning. cargo check -p minebtc, cargo test -p minebtc (159/159), anchor build --program-name minebtc, cargo build-sbf --manifest-path programs/mineBTC/Cargo.toml, and git diff --check pass. Mainnet readiness is held only on the remaining upstream mpl-core stack warning; devnet iteration is acceptable."
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
