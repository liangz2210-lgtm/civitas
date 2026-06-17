# Civitas Protocol — Risk Register

Known risks, their severity, and current mitigation status.
Unmitigated risks are marked ❌. Partial mitigations are ⚠️.

---

## Economic risks

### R1 — Death spiral (price → issuance → price)  ❌ partially mitigated

**Description:** Price falls → UBI issuance increases → more sell
pressure → price falls further. Self-reinforcing.

**Mitigations in place:**
- Oracle rate limit ±20% per update
- EMA smoothing (10% weight on new data)
- Hard daily cap per person

**NOT yet done:** Monte Carlo simulation to quantify probability and
severity. This is task `sim-001` in ROADMAP.md and should be completed
before mainnet.

---

### R2 — Equilibrium transition failure  ✅ mitigated

**Description:** Supply reaches target while FeePool is near-empty.
Protocol enters Equilibrium but cannot pay UBI. Immediate failure.

**Mitigation:** `can_enter_equilibrium()` requires FeePool ≥ 90 days
of demand. Without this gate, the protocol stays in Transition.

---

### R3 — Reflexive rational-expectations attack  ❌ not mitigated

**Description:** Actors who anticipate price decline can rationally
accelerate it by front-running UBI claims and selling, making the
decline self-fulfilling.

**Mitigation:** EMA smoothing delays but does not prevent. No
structural solution exists in V0.1. Requires circuit-breaker design.

---

## Identity risks

### R4 — Sybil attack during Bootstrap  ⚠️ partially mitigated

**Description:** Low barrier during early phase allows one actor to
create many accounts.

**Mitigation:** Social vouching (K=3 threshold, rate-limited).
Genesis bootstrap is public and witnessed.

**Gap:** Vouching is only as strong as the social graph. Collusion
among a group of real people could bootstrap a Sybil cluster.

---

### R5 — History laundering (governance attack)  ✅ mitigated

**Description:** Fake accounts created early accumulate clean-looking
on-chain history, then attack governance when it opens.

**Mitigations:**
1. Governance locked until audit certified (admin cannot rush this)
2. Account age weight: new accounts have 0% voting power, grows
   linearly to 100% over one year
3. 30-day public challenge window for flagged accounts

---

### R6 — Oracle compromise  ⚠️ partially mitigated

**Description:** Admin key compromise allows arbitrary price submission,
potentially collapsing the system in one block.

**Mitigations in V0.1:**
- Single update ≤ ±20% of current EMA (hard constraint in code)
- Hard per-person daily mint cap

**Gap:** Admin key is still a single point of failure. V0.2 must
replace with multi-source Chainlink aggregator.

---

## Governance risks

### R7 — Committee capture  ⚠️ accepted risk in V0.1

**Description:** Technical committee controls all parameters before
governance unlocks. Committee could abuse this.

**Mitigation:** Committee powers are narrowly scoped:
- Cannot mint tokens
- Cannot change the governance unlock conditions
- Cannot prevent the audit certification from being verified on-chain
- All committee actions are on-chain and auditable

**Accepted:** Some centralization in V0.1 is a deliberate trade-off
for simplicity. V0.2 replaces committee with a multi-sig DAO.

---

## Legal risks

### R8 — Securities classification  ❌ not addressed

**Description:** CIV may be classified as a security in some
jurisdictions under tests like the Howey Test (US) due to:
- Fixed total supply with appreciation narrative
- Early participant incentives (ActivityFactor)
- Referral reward structure

**Current status:** No legal review has been conducted.

**Recommendation:** Obtain legal opinions in key jurisdictions before
any public token sale, listing, or marketing campaign that emphasises
price appreciation. The protocol itself (code) is neutral; how it is
marketed is the risk surface.

---

## What we are NOT claiming

- We do not claim this protocol is economically stable at scale
- We do not claim the death-spiral risk is solved
- We do not claim the oracle design is sufficient for mainnet
- We do not claim legal compliance in any jurisdiction

These are honest engineering limitations of V0.1. They are documented
so that contributors can prioritise accordingly.
