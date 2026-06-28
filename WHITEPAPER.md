# Civitas Protocol — Whitepaper V0.1

> *Civitas*: Latin for "citizenship, community, the public good."

---

## 1. Vision

Civitas is a **UBI-native blockchain** that turns "universal basic income" from a political promise into an on-chain protocol — one that is **transparent, accountable, and self-governing**.

Every verified person receives a regular, on-chain income. No politician votes on it. No central bank prints it. The rules are code — and the code is governed by the very people it serves.

## 2. Problem Statement

| Issue | Legacy UBI | Civitas |
|---|---|---|
| **Who funds it?** | Taxpayers (coercive) | Protocol issuance (non-dilutive decay) |
| **Who gets it?** | Political definition | Cryptographic personhood verification |
| **Who governs it?** | Bureaucracy | Token-weighted quadratic governance |
| **Can it be audited?** | Opaque budgets | Every satoshi on-chain |
| **Inflation risk?** | Uncontrolled printing | Built-in decay circuits |

## 3. Architecture Overview

```
┌───────────────────────────────────────────────────────┐
│                    Civitas Runtime                      │
│                                                         │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │ CivIdentity  │  │ CivMonetary  │  │ CivGovernance │  │
│  │             │  │             │  │               │  │
│  │ • Verify    │  │ • UBI claim  │  │ • Propose    │  │
│  │ • Vouch     │  │ • Decay     │  │ • Vote        │  │
│  │ • Challenge │  │ • Circuit    │  │ • Quadratic  │  │
│  │ • Referral  │  │   breaker   │  │ • QF match    │  │
│  └──────┬──────┘  └──────┬──────┘  └───────┬───────┘  │
│         │                │                  │           │
│  ┌──────┴────────────────┴──────────────────┴───────┐  │
│  │              CivConstitution                       │  │
│  │  • Wealth cap enforcement                         │  │
│  │  • Governance weight ceiling                      │  │
│  │  • Constitutional invariants                     │  │
│  └───────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────┘
```

## 4. Core Pallets

### 4.1 CivIdentity — Personhood

**Purpose**: One identity per human. Sybil-resistant verification.

- **Verification**: Committee-verified accounts. V0.1 uses a trusted committee; V0.2 migrates to social-graph + ZK proofs.
- **Vouching**: Verified members vouch for newcomers. Vouching is stake-weighted — vouchees who get penalised slash the voucherer.
- **Challenge**: Any verified person can challenge a suspicious identity. If the challenge succeeds, the identity is revoked and the voucherer is slashed.
- **Referral Tracking**: Maps (referrer, referral) pairs. Feeds into governance R-component.

**Key Storage**:
- `Verified<AccountId, bool>` — verification status
- `Vouches<AccountId, AccountId, BlockNumber>` — vouch graph
- `PenaltyPoints<AccountId, u32>` — misbehavior score
- `AuditCertified<bool>` — governance unlock flag

### 4.2 CivMonetary — UBI Engine

**Purpose**: Regular UBI payments with anti-inflation mechanics.

- **UBI Claim**: Every verified person can claim once per period. The amount is `base_rate × activity_multiplier`.
- **Activity Tracking**: On-chain actions (transfers, governance, referrals) boost the multiplier. Idle accounts get only the base rate.
- **Exponential Decay**: Unclaimed UBI and idle balances decay over time. This replaces "money printing" with a circulation incentive — spend or participate, or your nominal balance shrinks.
- **Circuit Breaker**: If total issuance in a block exceeds a threshold, the circuit breaker halts claims until the next period. Prevents flash-minting attacks.

**Key Storage**:
- `ClaimedAt<AccountId, BlockNumber>` — last claim timestamp
- `ActivityScore<AccountId, u64>` — on-chain activity metric
- `DecaySchedule<BlockNumber, Perbill>` — epoch → decay rate
- `CircuitBreakerActive<bool>` — emergency halt flag

**Parameters (Genesis)**:
- `base_rate`: 100 tokens/period
- `period_blocks`: 7200 (~1 day at 12s blocks)
- `decay_rate`: 0.5%/period
- `circuit_breaker_threshold`: 500,000 tokens/block

### 4.3 CivGovernance — Quadratic Weight

**Purpose**: Governance by the people, for the people — with anti-plutocratic safeguards.

- **Governance Score** = f(Balance, Circulation activity, Referral count, Donation) — weighted sum of four components.
- **Quadratic Weight**: Vote weight = √Score × AgeFactor. A 100-score voter gets 10 votes, not 100. This reduces whale dominance.
- **Three-Level Proposals**: Parameter (low stakes), Feature (medium), Constitutional (high). Higher levels require higher quorum and longer timelocks.
- **Governance Unlock**: Governance activates only when ALL three conditions are met:
  1. Chain age ≥ `MIN_CHAIN_AGE` blocks
  2. Verified person count ≥ `MIN_VERIFIED`
  3. Committee certifies identity audit is complete
- **Quadratic Funding**: Community donations are matched by the protocol treasury at a variable multiplier. Small donors get proportionally more match — same QF mechanism as Gitcoin.

**Key Storage**:
- `Score<AccountId, u64>` — aggregate governance score
- `Components<AccountId, (u64,u64,u64,u64)>` — (B, T, R, D) breakdown
- `Proposals<u32, Proposal>` — governance proposals
- `Referrals<AccountId, Vec<ReferralRecord>>` — referral tracking

### 4.4 CivConstitution — Invariant Guard

**Purpose**: Hard constraints that the protocol cannot violate, even with governance.

- **Wealth Cap**: No account may hold more than `AccumulationRatio × median_balance`. If an account exceeds it, the excess is redistributed via the UBI pool.
- **Governance Weight Ceiling**: A single voter's weight cannot exceed `MaxWeightCap` regardless of score. Absolute power corrupts absolutely — so we cap it.
- **Constitutional Proposals**: The hardest proposal level. Requires 75% supermajority + timelock. Changes affect the invariant parameters themselves.

**Key Constants**:
- `MaxWeightCap`: 10,000 (score units)
- `AccumulationRatio`: 100 (× median)

## 5. Token Model

### 5.1 Issuance

- New tokens enter circulation ONLY through UBI claims.
- Total potential supply per period = `base_rate × verified_count`.
- Actual supply is lower because: (a) not everyone claims, (b) decay reduces idle balances, (c) circuit breaker caps issuance spikes.

### 5.2 Decay

Decay is not "burning" — it's a **demurrage** mechanism. The nominal balance decreases, but the purchasing power is preserved because everyone's nominal balance also decreases proportionally. It's a velocity incentive: use it or slowly lose it.

```
balance(t+1) = balance(t) × (1 - decay_rate)
```

With 0.5% per period, an idle balance halves in ~138 periods (~4.5 months).

### 5.3 No Pre-mine, No ICO

All tokens originate from UBI claims. Early adopters benefit from higher base rates (before decay kicks in), but they must be verified humans — not bots.

## 6. Governance Lifecycle

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐
│   Boot      │ ──▶ │  Governance  │ ──▶ │  Mature      │
│   Phase     │     │  Phase       │     │  Phase       │
│             │     │              │     │              │
│ Committee   │     │ Community    │     │ Constitutional│
│ controls    │     │ proposals + │     │ amendments   │
│ all params  │     │ voting       │     │ via 75%      │
│             │     │              │     │ supermajority │
└─────────────┘     └──────────────┘     └──────────────┘
```

**Boot Phase**: Committee sets all parameters. Required because there are no verified users yet to vote.

**Governance Phase**: Activated when chain age + verified count + audit conditions are met. Parameter and Feature proposals are community-voted. Committee retains constitutional veto until Mature Phase.

**Mature Phase**: Even constitutional amendments are community-governed. Committee becomes an advisory role.

## 7. Sybil Resistance

V0.1 relies on a **trusted committee** for identity verification. This is a bootstrap compromise — it's centralized but necessary.

V0.2 roadmap:
- **Social Graph Sybil Resistance**: Based on [BrightID](https://brightid.org) protocol — verified humans form a social graph; sybils can't forge verification connections.
- **Zero-Knowledge Proofs**: Verify personhood without revealing identity. "I am a unique human" without "I am Alice."
- **Reputation Staking**: Vouches require stake. Failed challenges slash the voucherer.

## 8. Comparison

| Feature | Civitas | CirclesUBI | GoodDollar | Manna |
|---|---|---|---|---|
| UBI mechanism | On-chain claim + decay | Peer-issued tokens | Reserve-backed | Airdrop |
| Governance | Quadratic weight | None | Centralized | Centralized |
| Sybil resistance | Committee → ZK | Social graph | Face scan | KYC |
| Anti-inflation | Decay + circuit breaker | Trust-based | Reserve | None |
| Wealth cap | Protocol-level | No | No | No |
| Self-sustaining | Yes (decay recycles) | Partially | No | No |

## 9. Roadmap

### V0.1 — Proof of Concept (Current)
- [x] 4 core pallets: Identity, Monetary, Governance, Constitution
- [x] Substrate frame v40 runtime
- [x] Committee-based verification
- [x] UBI claim with activity multiplier
- [x] Exponential decay scheduler
- [x] Circuit breaker
- [x] Quadratic-weight governance
- [x] Three-level proposals + timelocks
- [x] Quadratic funding (community donation matching)
- [x] Referral tracking
- [x] Constitutional wealth cap + governance weight cap
- [ ] Genesis chain that produces blocks
- [ ] Basic CLI wallet interaction

### V0.2 — Decentralization
- [ ] ZK personhood proofs
- [ ] Social-graph sybil resistance
- [ ] Automatic cross-pallet hooks (replace manual calls)
- [ ] On-chain treasury with quadratic funding
- [ ] WASM light client
- [ ] Mobile wallet

### V0.3 — Maturation
- [ ] Constitutional amendment via 75% supermajority
- [ ] Committee → advisory role
- [ ] Cross-chain identity bridging
- [ ] Governance-weighted delegation
- [ ] Formal verification of constitution invariants

## 10. Technical Stack

| Component | Choice | Reason |
|---|---|---|
| Blockchain framework | Substrate (frame v40) | Mature, modular, WASM runtime |
| Consensus | Aura (BABE future) | Fast finality for V0.1 |
| Finality | Grandpa | Proven BFT finality |
| Runtime | WASM (wasm32v1-none) | Substrate standard |
| Language | Rust | Safety + performance |
| Identity V0.1 | Committee multisig | Bootstrap pragmatism |
| Identity V0.2 | ZK + social graph | Decentralized sybil resistance |

## 11. License

Civitas Protocol is open-source under the **Apache-2.0** license.

---

## 12. Appendix: Key Constants

| Constant | Value | Unit |
|---|---|---|
| `BASE_RATE` | 100 | tokens/period |
| `PERIOD_BLOCKS` | 7,200 | blocks (~1 day) |
| `DECAY_RATE` | 0.5% | per period |
| `CIRCUIT_BREAKER_THRESHOLD` | 500,000 | tokens/block |
| `MIN_CHAIN_AGE` | 100,000 | blocks (~14 days) |
| `MIN_VERIFIED` | 1,000 | accounts |
| `VOTING_PERIOD` | 43,200 | blocks (~6 days) |
| `MAX_REFERRALS` | 100 | per referrer |
| `MATCH_MULTIPLIER` | 2× | default QF match |
| `MaxWeightCap` | 10,000 | score units |
| `AccumulationRatio` | 100 | × median balance |

---

*"The price of liberty is eternal vigilance." — Civitas makes vigilance algorithmic.*
