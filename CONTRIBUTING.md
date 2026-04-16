# Contributing to MineBTC

Thank you for your interest in contributing to MineBTC.

## Getting Started

### Prerequisites

- Rust 1.90.0+ (`rustup install 1.90.0`)
- Anchor CLI 0.31.1 (`cargo install --git https://github.com/coral-xyz/anchor --tag v0.31.1 anchor-cli`)
- Solana CLI 2.2.12+

### Setup

```bash
git clone https://github.com/LifeOrDream/MineBtc-fi.git
cd MineBtc-fi
anchor build -p minebtc
```

## Development Workflow

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/your-feature`)
3. Make your changes
4. Ensure code passes checks:
   ```bash
   cargo fmt --all -- --check
   cargo check -p minebtc
   cargo test -p minebtc --lib
   anchor build -p minebtc
   ```
5. Commit with a descriptive message
6. Open a Pull Request against `main`

## Code Style

- Format with `cargo fmt`
- Follow existing naming conventions (`_internal` suffix for instruction implementations)
- Keep instruction logic in `programs/mineBTC/src/instructions/`
- Expose instructions in `programs/mineBTC/src/lib.rs`

## Product Terminology

MineBTC is a **degen country arena game**. Keep docs, comments, and PR descriptions aligned with this framing.

Use:

- `country` / `faction`
- `direction` (`Down`, `Neutral`, `Up`)
- `round` — 60-second betting loop
- `rebase` — competitive cycle tied to LP burn, mutation leaderboard
- `gameplay doge` / `operator doge`
- `mutation` / `mutation score`

Do NOT use:

- "prediction market", "geopolitical risk", "intelligence", "data pipeline"
- "oracle" (in game context -- Raydium price oracle references are fine)
- "epoch" (replaced by "rebase")
- "active index", "index state"
- block-number betting language

The current product story: **Pick your country, bet SOL, your doge evolves, your country climbs, you earn dogeBTC.**

## Pull Request Guidelines

- Keep PRs focused on a single change
- Include a description of what changed and why
- Reference related issues if applicable
- Ensure CI checks pass

## Security Vulnerabilities

**Do NOT open GitHub issues for security vulnerabilities.**

Please report them via email to gm@minebtc.fun. See [SECURITY.md](SECURITY.md) for details.

## License

By contributing, you agree that your contributions will be licensed under the [Business Source License 1.1](LICENSE).
