//! # Circuit Breaker — Issuance Fuse
//!
//! When chain-wide daily minting exceeds a configurable multiple of the
//! expected daily budget, the breaker trips and all further minting
//! (claim_ubi, fee-pool withdrawals) is halted until either:
//!   a) a cooldown time passes, OR
//!   b) root manually resets.
//!
//! V0.1: on-chain storage + check. Oracle / off-chain monitor in V0.2.

use frame_support::pallet_prelude::*;
use sp_runtime::traits::{Saturating, Zero};

/// Circuit breaker state.
#[derive(
    Clone,
    Encode,
    Decode,
    DecodeWithMemTracking,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
)]
pub enum BreakerState {
    /// Normal operation.
    #[default]
    Closed,
    /// Minting exceeded threshold — all minting blocked.
    Open { tripped_at: u32 },
}

/// Per-day mint tracking for the breaker.
#[derive(
    Clone,
    Encode,
    Decode,
    DecodeWithMemTracking,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
)]
pub struct DayMint<Balance> {
    pub day: u32,
    pub amount: Balance,
}

/// Check whether the circuit breaker allows minting.
/// Returns Ok(()) if minting is allowed, Err otherwise.
pub fn check_breaker(state: &BreakerState, cooldown_blocks: u32, now: u32) -> DispatchResult {
    match state {
        BreakerState::Closed => Ok(()),
        BreakerState::Open { tripped_at } => {
            if now >= tripped_at.saturating_add(cooldown_blocks) {
                Ok(())
            } else {
                Err(DispatchError::Other("CircuitBreaker: minting halted"))
            }
        }
    }
}

/// Record a mint and trip the breaker if the daily cap is exceeded.
/// Returns the new breaker state.
pub fn record_mint<Balance>(
    state: &BreakerState,
    day_mint: &mut DayMint<Balance>,
    amount: Balance,
    day: u32,
    daily_cap: Balance,
    now: u32,
    cooldown_blocks: u32,
) -> BreakerState
where
    Balance: Saturating + Zero + Copy + PartialOrd,
{
    // Auto-reset from Open if cooldown expired
    let current = match state {
        BreakerState::Open { tripped_at } => {
            if now >= tripped_at.saturating_add(cooldown_blocks) {
                BreakerState::Closed
            } else {
                return BreakerState::Open {
                    tripped_at: *tripped_at,
                };
            }
        }
        BreakerState::Closed => BreakerState::Closed,
    };

    // Day rollover
    if day_mint.day != day {
        day_mint.day = day;
        day_mint.amount = Balance::zero();
    }

    day_mint.amount = day_mint.amount.saturating_add(amount);

    if day_mint.amount > daily_cap {
        BreakerState::Open { tripped_at: now }
    } else {
        current
    }
}
