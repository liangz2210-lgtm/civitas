# Civitas Protocol (CIV)

**Adaptive Universal Basic Income on Substrate.**
*人类文明的自适应基础收入协议*

[![CI](https://github.com/civitas-protocol/civitas/actions/workflows/ci.yml/badge.svg)](https://github.com/civitas-protocol/civitas/actions)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

---

## What it is

A Substrate-based protocol that issues a daily UBI to every verified
human being, anchored to the **minimum cost of survival** rather than
to a fixed supply or a USD peg.

```
Daily_UBI_coins = FCI / EMA(CIV_price) × ActivityFactor

FCI  = Food Cost Index  (~$4 / day)
EMA  = exponential moving average of market price

Price $1.00  →  4.00 CIV / day
Price $4.00  →  1.00 CIV / day
Price $100   →  0.04 CIV / day
```

When price rises fewer coins are issued (scarcity maintained).
When price falls more coins are issued (purchasing power defended).
Speculators and UBI recipients use the same token.

---

## What this is NOT

- Not a stablecoin — price floats freely
- Not a fixed-supply asset — supply tracks productivity
- Not investment advice

See [docs/RISKS.md](docs/RISKS.md) for an honest risk register.
The death-spiral risk is real and **not yet fully mitigated** (see `sim-001`).

---

## Architecture

```
pallet-civ-identity   proof of personhood, social vouching, audit system
pallet-civ-monetary   UBI issuance, fee recycling, auto-stabiliser
pallet-civ-governance three-condition unlock, age-weighted voting, 4D score,
                      tiered proposals, public-goods funding
```

### Identity — social vouching, no admin key

No whitelist. No sudo. Accounts are verified by social vouching:
3 existing verified accounts must vouch. The technical committee's
only identity powers are the one-time genesis bootstrap and audit
certification — no ongoing control.

### Monetary — three phases

| Phase | Condition | UBI source |
|-------|-----------|-----------|
| Bootstrap | Minted < 80% target | Mint |
| Transition | 80% ≤ Minted < target | Mint + FeePool |
| Equilibrium | Minted ≥ target **and** FeePool ≥ 90d reserve | FeePool only |

The Equilibrium entry gate prevents the protocol from declaring
victory before the fee pool can sustain UBI.

### Governance — three-condition unlock

Voting is locked until all three conditions are met:

1. Chain age ≥ 180 days
2. Verified identities ≥ 10,000
3. Identity audit certified complete

Voting weight = GovernanceScore × AgeFactor, where AgeFactor grows
linearly from 0% to 100% over one year. New accounts cannot
immediately capture governance even if they accumulated history before
the audit.

---

## Quick start

```bash
curl https://sh.rustup.rs -sSf | sh
rustup target add wasm32-unknown-unknown

git clone https://github.com/civitas-protocol/civitas
cd civitas
cargo test --all
```

---

## Runtime integration (brief)

```rust
// Cargo.toml
pallet-civ-identity  = { git = "https://github.com/civitas-protocol/civitas" }
pallet-civ-monetary  = { git = "https://github.com/civitas-protocol/civitas" }
pallet-civ-governance = { git = "https://github.com/civitas-protocol/civitas" }

// runtime/src/lib.rs
impl pallet_civ_identity::Config for Runtime {
    type CommitteeOrigin = EnsureRoot<AccountId>;  // replace with multisig
    type VouchThreshold  = ConstU32<3>;
    type MaxBatchSize    = ConstU32<50>;
    // ...
}

parameter_types! {
    pub const FCI:  u64  = 400_000_000;           // $4.00
    pub const Target: u128 = 100_000_000 * UNITS; // 100M CIV
}

impl pallet_civ_monetary::Config for Runtime {
    type Personhood             = CivIdentity;
    type FoodCostIndex          = FCI;
    type InitialTargetSupply    = Target;
    type EquilibriumReserveDays = ConstU32<90>;
    type MaxDailyMintPerPerson  = ConstU32<10_000>;
    // ...
}

impl pallet_civ_governance::Config for Runtime {
    type Personhood      = CivIdentity;
    type WBalance        = ConstU32<25>;
    type WCirculation    = ConstU32<25>;
    type WReferral       = ConstU32<25>;
    type WDonation       = ConstU32<25>;
    // ...
}
```

---

## Repository layout

```
civitas/
├── Cargo.toml
├── LICENSE
├── README.md
├── ROADMAP.md              development plan + claimable tasks
├── CONTRIBUTING.md
├── docs/
│   ├── TOKENOMICS.md       supply model and known risks
│   └── RISKS.md            honest risk register
├── .github/workflows/
│   └── ci.yml
└── pallets/
    ├── civ-identity/
    ├── civ-monetary/
    └── civ-governance/
```

---

## Contribute

See [ROADMAP.md](ROADMAP.md) for open tasks and claiming instructions.

The most critical open task is `sim-001` — a Monte Carlo simulation to
quantify death-spiral probability. If you have economics or simulation
experience this is where to start.

**Apache-2.0 — Fork it. Build it. Live it.**
