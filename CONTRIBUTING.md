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
   cargo clippy --all-targets -- -D warnings
   cargo check -p minebtc
   cargo test -p minebtc --lib
   anchor build -p minebtc
   ```
5. Commit with a descriptive message
6. Open a Pull Request against `main`

## Code Style

- Format with `cargo fmt`
- Lint with `cargo clippy`
- Follow existing naming conventions (`_internal` suffix for instruction implementations)
- Keep instruction logic in `programs/mineBTC/src/instructions/`
- Expose instructions in `programs/mineBTC/src/lib.rs`

## Product Terminology

Please keep docs, comments, setup scripts, and PR descriptions aligned with the current game model.

Use:

- `country` / `faction`
- `direction` (`Down`, `Neutral`, `Up`)
- `round`
- `epoch`
- `active index`
- `operator doge`

Avoid reintroducing:

- block-number betting language
- block-high / block-low wording
- legacy `factionHighestLowest` / `factionBoth` phrasing

The current product story is: **one country-direction bet powers both the live round game and the active epoch index market**.

Lead documentation with what is already live in the contracts. Prefer "live country arena", "active index", and "data pipeline" over generic "prediction market" shorthand when the latter is less precise.

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
