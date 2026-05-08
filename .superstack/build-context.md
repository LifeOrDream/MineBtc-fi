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
  }
}
```
