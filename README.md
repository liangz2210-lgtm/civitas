# Civitas Protocol (CIV)

**Adaptive Universal Basic Income on Substrate.**
*人类文明的自适应基础收入协议*

[![CI](https://github.com/liangz2210-lgtm/civitas/actions/workflows/ci.yml/badge.svg)](https://github.com/liangz2210-lgtm/civitas/actions)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

---

## What it is

A Substrate-based blockchain protocol that issues a regular UBI to every
verified human being — governed by the people who receive it, not by
politicians or central banks.

Key properties:
- **On-chain UBI**: Verified persons claim tokens each period.
- **Anti-inflation**: Exponential decay on idle balances + circuit breaker on issuance.
- **Quadratic governance**: Vote weight = √Score × AgeFactor — wealth alone won't dominate.
- **Constitutional guard**: Hard caps on wealth accumulation and governance power.
- **Self-governing**: Three-condition unlock transitions committee control → community control.

---

## Architecture

```
pallet-civ-identity     Personhood verification, vouching, challenges, referral tracking
pallet-civ-monetary     UBI issuance, activity multiplier, decay scheduler, circuit breaker
pallet-civ-governance   Quadratic-weight voting, 3-level proposals, quadratic funding, governance score
pallet-civ-constitution Wealth cap enforcement, governance weight ceiling, constitutional invariants
```

See [WHITEPAPER.md](WHITEPAPER.md) for the full design document.

---

## Quick start

```bash
# Prerequisites
rustup target add wasm32v1-none
sudo apt install protobuf-compiler   # Linux
# brew install protobuf               # macOS

git clone https://github.com/liangz2210-lgtm/civitas.git
cd civitas
cargo test --all
cargo check
```

---

## Core Concepts

### Identity — One Human, One Account

No whitelist, no sudo. V0.1 uses a trusted committee for verification.
Vouching requires 3 verified accounts. Challenges penalise fraudulent identities and their voucherers.

### Monetary — UBI with Decay

Every verified person can claim UBI once per period. The amount scales with on-chain activity.
Unclaimed funds and idle balances decay exponentially — a velocity incentive, not a tax.

Circuit breaker: if total issuance in a block exceeds the threshold, claims halt until next period.

### Governance — Quadratic Weight

Governance Score = weighted sum of (Balance, Circulation activity, Referral count, Donation).
Vote weight = √Score × AgeFactor. Three proposal levels with escalating quorum and timelocks.

Governance unlocks only when ALL three conditions are met:
1. Chain age >= 100,000 blocks (~14 days)
2. Verified identities >= 1,000
3. Identity audit certified by committee

### Constitution — Hard Invariants

- No account may hold more than 100× the median balance (wealth cap).
- No voter's weight may exceed 10,000 score units (power cap).
- Constitutional amendments require 75% supermajority + timelock.

---

## Repository layout

```
civitas/
├── Cargo.toml              workspace definition
├── WHITEPAPER.md            full design document
├── pallets/
│   ├── civ-identity/        personhood + vouching + referral
│   ├── civ-monetary/        UBI + decay + circuit breaker
│   ├── civ-governance/      voting + proposals + QF
│   └── civ-constitution/    wealth cap + power cap
├── runtime/                 Substrate runtime (Aura + Grandpa)
├── node/                    CLI node binary
└── .github/workflows/
    └── ci.yml
```

---

## Roadmap

See [WHITEPAPER.md §9](WHITEPAPER.md#9-roadmap) for V0.1 / V0.2 / V0.3 milestones.

---

## License

Apache-2.0 — Fork it. Build it. Live it.
