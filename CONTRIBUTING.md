# Contributing to Civitas Protocol

## Setup

```bash
git clone https://github.com/civitas-protocol/civitas
cd civitas
rustup target add wasm32-unknown-unknown
cargo test --all
```

## Workflow

1. Check ROADMAP.md — pick an open task
2. Open a GitHub Issue: `[Claim] sim-001 — Monte Carlo simulation`
3. Fork → feature branch → PR against `develop`
4. CI must pass (fmt + clippy + test + WASM)
5. One reviewer approval required

## Code standards

- `cargo fmt --all` before committing
- `cargo clippy --all-targets -- -D warnings` must be clean
- All public items need doc comments
- Storage invariants must be documented
- New features need tests

## Commit format

```
feat(monetary): add equilibrium FeePool gate
fix(identity): prevent double-vouch across windows
test(governance): age-weight boundary conditions
docs(risks): add rational-expectations attack scenario
```

## Governance unlock conditions

Governance is locked until: chain age ≥ 180d, verified ≥ 10,000,
audit certified. Before unlock the committee controls parameters.
This is a deliberate V0.1 trade-off, not a permanent design.

## Be kind

This project exists to help everyone. Assume good intent.
