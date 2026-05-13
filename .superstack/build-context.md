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
      },
      {
        "severity": "P1",
        "category": "NFT identity / collection binding",
        "description": "HashBeast mint and breed paths accept an optional, unchecked collection account, so program-valid HashBeast metadata can be created for assets outside the configured Metaplex Core collection.",
        "fix": "Make the HashBeast collection required and bind it to HashBeastConfig.hashbeast_collection on genesis/admin/whitelist/breed/rebirth/stake/unstake paths, then pass that bound account into every MPL Core CPI."
      },
      {
        "severity": "P2",
        "category": "Economics / stale oracle state",
        "description": "breed_hashbeasts prices against FloorHistory.current_anchor() without checking last_snapshot_at, so stale floor data can keep breeding below the live market floor or overpriced after floor moves.",
        "fix": "Require last_snapshot_at to be recent before breeding, or force a fresh floor snapshot before breed pricing can use the 1.5x floor guard."
      },
      {
        "severity": "P3",
        "category": "Arithmetic safety",
        "description": "add_tickets_to_player increments free_tickets_remaining with unchecked u64 addition.",
        "fix": "Use checked_add for the ticket counter update and reject zero-value ticket tiers at config/update time."
      }
    ],
    "ready_for_mainnet": false,
    "notes": "2026-05-14 focused audit of game.rs, faction_war.rs, economy.rs, user.rs, and hashbeasts.rs. cargo check -p minebtc, cargo test -p minebtc (159/159), focused hashbeasts unit tests (14/14), and git diff --check pass. Not production ready until the add_lp_and_burn POL SOL drain path, snapshot oracle guards, mutation-counter claim liveness issue, and HashBeast collection-binding issue are fixed; SBF/Anchor build and adversarial integration tests still recommended."
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
