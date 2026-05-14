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
      }
    ],
    "ready_for_mainnet": false,
    "notes": "2026-05-14 follow-up NFT marketplace fixes on commit 20c51a0. The three marketplace blockers from that pass are fixed. 2026-05-14 game/economy/faction-war/tax audit then hardened canonical tax vault routing, Token-2022 post-fee accounting, LP dust handling, RPG-disabled reward docs, and setup script account wiring. cargo fmt --all --check, cargo check -p minebtc, cargo check -p degenbtc_market, cargo test -p minebtc --lib, cargo test -p degenbtc_market --lib, anchor build, node --check setup_scripts/do_txs.js, node --check setup_scripts/3_init_mineBTC.js, and git diff --check pass locally. anchor build still emits the existing upstream mpl-core hooked/plugin stack warning, so keep mainnet signoff gated on a clean SBF dependency story or explicit acceptance of that risk."
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
