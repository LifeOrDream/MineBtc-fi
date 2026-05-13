# Build Context

Most recent review snapshot. Used by `/review`-style flows to track
outstanding findings vs. what has been resolved. Update via `/review` or
`/learn`; do not edit by hand without good reason.

```json
{
  "review": {
    "security_score": "C",
    "quality_score": "B",
    "findings": [
      {
        "severity": "P0",
        "category": "CPI/account validation",
        "description": "add_lp_and_burn transfers earnmarked SOL to an unchecked caller-provided WSOL account before validating Raydium pool vault bindings; a fake zero-balance sol_vault can trigger an Ok early return and strand/drain POL SOL outside the buybacks vault.",
        "fix": "Bind sol_token_account to the authority PDA WSOL ATA, validate pool_state fields for amm_config, vaults, mints, LP mint, token programs and observation state before moving SOL, and move the zero-pool early return before the system transfer."
      },
      {
        "severity": "P1",
        "category": "Oracle/economics",
        "description": "snapshot_price accepts min_amount_out=0 and records zero-price snapshots when the swap output is zero or dust-sized, allowing griefed or manipulated rate/multiplier updates.",
        "fix": "Require a minimum swap size and non-zero dbtc_received/current_price, and add slippage/deviation bounds against recent price or a TWAP before appending to price_history."
      },
      {
        "severity": "P1",
        "category": "Claim liveness",
        "description": "round mutation counters are u8 and checked_add on successful claim-time mutations; high-traffic rounds can overflow and block later winners from claiming.",
        "fix": "Widen GameSession mutation counters to u16/u32 or saturate claim-time accounting so rewards cannot be held hostage by counter exhaustion."
      }
    ],
    "ready_for_mainnet": false,
    "notes": "2026-05-14 focused audit of game.rs, faction_war.rs, economy.rs, and user.rs. cargo check -p minebtc, cargo test -p minebtc (159/159), and git diff --check pass. Not production ready until the add_lp_and_burn POL SOL drain path, snapshot oracle guards, and mutation-counter claim liveness issue are fixed; SBF/Anchor build and adversarial integration tests still recommended."
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
