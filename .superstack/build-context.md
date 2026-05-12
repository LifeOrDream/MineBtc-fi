# Build Context

Most recent review snapshot. Used by `/review`-style flows to track
outstanding findings vs. what has been resolved. Update via `/review` or
`/learn`; do not edit by hand without good reason.

```json
{
  "review": {
    "security_score": "B+",
    "quality_score": "B",
    "findings": [],
    "ready_for_mainnet": false,
    "notes": "All previously-tracked findings (Doge → HashBeast rename, max_supply removal, marketplace-cpi binding, sweep stale-pop, snapshot anchor guard, MAX_EXPIRES burn boundary, expire close panic) have been resolved. cargo check, cargo test -p minebtc --lib (47/47), and anchor idl build pass. Mainnet readiness held at false until SBF stack-offset diagnostics are reduced or explicitly accepted after stress-test on devnet."
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
