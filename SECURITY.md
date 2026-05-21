# Security Policy

## Reporting security problems in MineBTC programs

**DO NOT CREATE A GITHUB ISSUE** to report a security problem.

Instead please use this [Report a Vulnerability](https://github.com/LifeOrDream/MineBtc-fi/security/advisories/new) link. Provide a helpful title, detailed description of the vulnerability, and an exploit proof-of-concept. Speculative submissions without proof-of-concept will be closed with no further consideration.

If you haven't done so already, please enable two-factor auth in your GitHub account.

Expect a response as fast as possible in the advisory, **typically within 72 hours**.

If you do not receive a response in the advisory, send an email to **pretentiouspunjabiguy@gmail.com** with the full URL of the advisory you have created. **DO NOT** include attachments or provide detail sufficient for exploitation regarding the security issue in this email. Only provide such details in the advisory.

If you do not receive a response on email either, please follow up with the team directly on Telegram at **https://tg.minebtc.fun/**. Mention that you submitted a security advisory and reference the advisory URL — do not paste exploit details in the public group.

## In scope

- MineBTC program: `1eotiTH2UxCpPMmtzUDGqf1b8dwM7AMKb8a2Tio51an`
- DegenBTC Marketplace program: `BCuofnvb7QUP6xLH83EEbKFNjxz5T5Jp4xLqfEdURYRg`
- dBTC token mint and its Token-2022 transfer-fee config: `CtAu3kc8cQ1jcDMmRTBsDHoPuE3sswCagQ3BuqFDC6dt`
- dBTC/SOL Raydium CP-Swap pool state: `F87M4sT6Wtfk4enVVbtM4ZnWsqCE9TXzL12Apwj3Cjtj`

## Out of scope

- Front-end / website bugs (`minebtc.fun`) — report informally on Telegram, not via security advisory.
- Issues in third-party programs we CPI into (Raydium CP-Swap, Metaplex Core, Token-2022, SPL Token). Report those upstream.
- Theoretical issues without a concrete exploit path.
- MEV / sandwich attacks against bettors — this is a public mempool, by design.
- Reorg / cluster-level Solana issues.

## What we want

In-scope findings that lead to **loss of user funds, theft of protocol funds, unauthorized admin actions, or permanent denial-of-service** of core game functions (round play, claims, mints, marketplace).

## Bounties

We do not run a fixed bounty program. We pay rewards at our discretion, proportional to severity and impact, paid in SOL or dBTC. No bounty for issues already publicly disclosed or reported by another party. We will not pursue legal action against good-faith researchers who follow this policy.

## Hall of fame

Credited at https://minebtc.fun/security (coming post-launch).
