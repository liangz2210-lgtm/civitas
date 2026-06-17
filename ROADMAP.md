# Civitas Protocol — Roadmap

**How to claim a task:** Open a GitHub Issue titled `[Claim] <task-id>`,
assign yourself, post weekly updates.

---

## V0.1 — Foundation (open source now)

Three Substrate pallets: identity, monetary, governance.
Social vouching, price-linked UBI, FeePool equilibrium gate,
three-condition governance unlock, age-weighted voting.

### Open tasks

| ID | Task | Skill | Priority |
|----|------|-------|----------|
| `sim-001` | **Monte Carlo simulation** — model price/issuance/pool dynamics; quantify death-spiral probability | Python, economics | 🔴 Critical |
| `test-001` | Benchmark weights for all three pallets | Rust, Substrate benchmarking | 🟠 High |
| `doc-001` | Translate README + RISKS.md to Spanish, Portuguese, Swahili, Hindi | Writing | 🟡 Medium |
| `doc-002` | Non-technical 1-page explainer (PDF) | Design, writing | 🟡 Medium |
| `ci-001` | Add `cargo-mutants` mutation testing to CI | CI/CD | 🟡 Medium |

---

## V0.2 — Real Oracle + BrightID

Remove admin-only oracle. Connect real identity verification.
Automate decay.

### Tasks

| ID | Task | Skill | Priority |
|----|------|-------|----------|
| `id-001` | **BrightID `linkTo` on-chain oracle pallet** | Rust, Substrate, BrightID API | 🔴 Critical |
| `oracle-001` | **Chainlink multi-source price feed** — replace admin `update_price` | Rust, Chainlink | 🔴 Critical |
| `mon-001` | Unique-counterparty tracking for ActivityScore (Bloom filter or StorageDoubleMap) | Rust | 🟠 High |
| `mon-002` | Time-weighted average balance for decay (replace snapshot) | Rust | 🟠 High |
| `mon-003` | Automatic decay scheduler via `on_initialize` | Rust | 🟠 High |
| `net-001` | Rococo / local testnet deployment | DevOps, Substrate | 🟠 High |
| `net-002` | Testnet faucet web UI | React/Next.js | 🟡 Medium |
| `audit-001` | Community security audit of V0.1 pallets | Rust, security | 🟠 High |

---

## V0.3 — Mobile App MVP

First user-facing application. Target: 10,000 testnet users.

Reference UX: GoodDollar daily claim + Worldcoin World App architecture.

### Tasks

| ID | Task | Skill | Priority |
|----|------|-------|----------|
| `app-001` | React Native scaffold (Expo + TypeScript + Polkadot API) | React Native | 🔴 Critical |
| `app-002` | Account Abstraction wallet (no private key exposure) | TypeScript, crypto | 🔴 Critical |
| `app-003` | **Daily UBI claim screen** — one button, countdown, animation | React Native, UX | 🔴 Critical |
| `app-004` | BrightID in-app verification flow | React Native, BrightID SDK | 🟠 High |
| `app-005` | Send / receive / QR payment | React Native | 🟠 High |
| `app-006` | Governance score dashboard | React Native, data viz | 🟡 Medium |
| `app-007` | Push notifications for governance proposals | React Native | 🟡 Medium |
| `app-008` | iOS TestFlight + Android APK CI pipeline | CI/CD, Fastlane | 🟠 High |
| `app-009` | On-chain proposal execution via `pallet-scheduler` | Rust, Substrate | 🟠 High |
| `app-010` | AI multilingual proposal summariser | Python, LLM API | 🟡 Medium |

---

## V0.4 — Social + Payment Layer

Merchant payments. Knowledge sharing. Task & contribution system.

### Tasks (open for early claims)

| ID | Task | Skill | Priority |
|----|------|-------|----------|
| `pay-001` | Merchant QR payment SDK (JS) | TypeScript | 🟠 High |
| `social-001` | On-chain social storage design (IPFS + hash anchoring) | Architecture | 🟠 High |
| `social-002` | Knowledge contribution reward engine | Rust, Substrate | 🟡 Medium |
| `task-001` | Contribution bounty system | Rust, Substrate | 🟡 Medium |
| `dash-001` | Public network health dashboard (supply, pool, phase) | React/Next.js | 🟡 Medium |

---

## V1.0 — Mainnet

**Launch criteria (all must be met before mainnet):**

- [ ] `sim-001` complete — death-spiral probability quantified and acceptable
- [ ] Third-party security audit — zero critical findings
- [ ] `id-001` complete — BrightID oracle live on testnet ≥ 90 days
- [ ] `oracle-001` complete — price oracle live on testnet ≥ 90 days
- [ ] ≥ 1,000,000 verified identities on testnet
- [ ] FeePool can sustain ≥ 80% of daily UBI without new mint (measured)
- [ ] Legal review completed in key jurisdictions

---

## Non-code contributions

| ID | Task | Skill |
|----|------|-------|
| `nc-001` | UX design (Figma) for all app screens | Figma, UX |
| `nc-002` | User testing in Southeast Asia / West Africa / Latin America | Community |
| `nc-003` | White paper academic review | Economics, sociology |
| `nc-004` | Economic simulation (Python/Jupyter) | Python, economics |
| `nc-005` | Merchant onboarding guide (PDF, multilingual) | Design, writing |
