# Security

MineBTC moves real SOL through betting rounds, mints real-supply dBTC, and runs a live NFT mint. If you find a bug that breaks any of that, we want to hear from you privately — not on Twitter.

## How to report

Open a private advisory at https://github.com/LifeOrDream/MineBtc-fi/security/advisories/new — please do not file a public GitHub issue.

We aim to acknowledge inside 72 hours. If the advisory goes quiet, ping us in Telegram (https://tg.minebtc.fun/) with the advisory URL only. Don't paste exploit details in the public chat.

Please report from a GitHub account with 2FA enabled.

## In scope

- MineBTC program: `1eotiTH2UxCpPMmtzUDGqf1b8dwM7AMKb8a2Tio51an`
- DegenBTC Marketplace program: `BCuofnvb7QUP6xLH83EEbKFNjxz5T5Jp4xLqfEdURYRg`

## Out of scope

- Front-end / website (`minebtc.fun`) — reach out on Telegram instead.
- Third-party programs we only CPI into (Raydium CP-Swap, Metaplex Core, Token-2022, SPL Token). Report upstream.
- Theoretical findings without a working PoC.
- MEV / sandwich attacks against bettors — Solana has a public mempool, by design.
- Reorgs and other cluster-level Solana issues.

## What we care about

Findings that lead to loss of user funds, theft of protocol funds, unauthorized admin actions, or permanent denial-of-service of round play, claims, mints, or the marketplace.

## Bounties

No fixed payout table. We pay at our discretion in SOL or dBTC, scaled to severity and impact. No bounty for issues already public, or already reported by someone else. We won't pursue legal action against good-faith researchers who follow this policy.
