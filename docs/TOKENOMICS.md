# Civitas Protocol — Tokenomics

> **Honesty notice**
> This document distinguishes between what is proven, what is plausible,
> and what is unvalidated. Sections marked ⚠️ are known risks.

---

## Supply Model

Total target supply: **100,000,000 CIV** (governance-adjustable).

This target is a soft ceiling, not a hard cap. In Bootstrap phase the
protocol mints freely. The target is a policy variable linked to a
governance proxy for global productivity (GPI). A real GPI oracle is
planned for V0.2.

### Three phases

```
Phase        Condition                                  UBI source
─────────────────────────────────────────────────────────────────
Bootstrap    Minted < 80% target                       Mint
Transition   80% ≤ Minted < target                    Mint + FeePool
Equilibrium  Minted ≥ target AND FeePool ≥ 90d reserve FeePool only
```

The Equilibrium entry gate (FeePool ≥ 90 days of demand) was added
after analysis showed that without it, the protocol transitions to
Equilibrium with an empty pool and UBI immediately fails.

---

## UBI Formula

```
Daily_UBI_coins = FCI / EMA(price) × ActivityFactor

FCI           = Food Cost Index (default $4.00, 8-decimal precision)
EMA(price)    = 0.9 × old_ema + 0.1 × oracle_price
ActivityFactor ∈ [0.50, 1.50]
```

When price rises → fewer coins issued (scarcity maintained).
When price falls → more coins issued (purchasing power defended).

### ⚠️ Known risk: reflexive loop

If price falls rapidly, issuance increases, which may create further
selling pressure. Mitigations in place:

- Oracle rate limit: single update ≤ ±20% of current EMA
- Hard per-person daily cap: `MaxDailyMintPerPerson`
- EMA smoothing reduces response to short-term price swings

Whether these are sufficient at scale has **not been proven**.
Monte Carlo simulation is a priority task (see ROADMAP.md `sim-001`).

---

## Fee Recycling

Three fee mechanisms route value from accumulation back to circulation:

| Fee | Parameter | Formula | Exemption |
|-----|-----------|---------|-----------|
| Transaction | α = 0.3% | `volume × α` | None |
| Inactivity | β = 1.0%/yr | `balance × β` if inactive ≥ 365d | None |
| Wealth decay | γ = 0.8%/yr | `γ × (balance − median)` | Below-median exempt |

All fees go to FeePool and are redistributed as UBI.

---

## ⚠️ Bootstrap–Equilibrium race condition

During Bootstrap, mint rate ≈ 67× FeePool accumulation rate at default
parameters (10 transactions/person/day at 0.3% fee). This means reaching
the target supply while FeePool is near-empty is the expected outcome
under normal growth.

The Equilibrium entry gate prevents a broken transition, but it also
means the protocol may stay in Transition indefinitely if transaction
volume is insufficient. This is a **design tension, not a bug** — the
protocol should not pretend to be sustainable before it actually is.

---

## Governance of parameters

All parameters adjustable by governance:
- FCI (Food Cost Index)
- TargetSupply
- α, β, γ (fee rates, ≤ 0.2% change per call)
- EquilibriumReserveDays
- MatchMultiplier (public-goods funding)

Rate limits prevent single-call parameter attacks.

---

## What this is NOT

- Not a fixed-supply deflationary asset (that would defeat the UBI purpose)
- Not a USD-pegged stablecoin (that imports inflation)
- Not a guaranteed store of value

CIV is a **purchasing-power-stable medium of exchange**. Its value
proposition is: one day's UBI always buys approximately one day's food.
Price appreciation is possible and expected, but not the primary goal.
