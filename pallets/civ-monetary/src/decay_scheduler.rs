//! # Decay Scheduler — Automatic Inactivity / Wealth Decay
//!
//! V0.1: On-demand batch decay. The scheduler tracks which accounts
//! need decay and provides a `run_decay_batch` extrinsic that applies
//! the current inactivity-rate and wealth-decay-rate to up to
//! `MaxDecayBatch` accounts per call.
//!
//! V0.2: On-chain scheduler (pallet-utility / pallet-scheduler) for
//! fully automatic periodic decay.

#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime::{traits::Saturating, Perbill};

/// Apply inactivity decay to an account.
/// Returns the decayed balance (or zero if fully decayed).
pub fn apply_inactivity_decay<Balance>(
    balance: Balance,
    last_active: u32,
    now: u32,
    rate: Perbill,
) -> Balance
where
    Balance: Saturating + Copy + From<u32> + Into<u64>,
{
    let days_inactive = now.saturating_sub(last_active) / 14_400; // blocks per day
    if days_inactive == 0 {
        return balance;
    }
    let capped = days_active_cap(days_inactive);
    // Linear approximation: (1 - d*r) clamped to >=0
    // Work in u64 to avoid Perbill x Balance trait issues
    let rate_parts = rate.deconstruct() as u64;
    let decay_parts = (rate_parts * capped as u64).min(1_000_000_000);
    let retain_parts = 1_000_000_000u64.saturating_sub(decay_parts) as u32;
    let balance_u64: u64 = balance.into();
    let result = balance_u64 * retain_parts as u64 / 1_000_000_000;
    Balance::from(result as u32)
}

/// Clamp days to 365 to avoid overflow in compound approximation.
fn days_active_cap(days: u32) -> u32 {
    days.min(365)
}

/// Apply wealth decay (flat Perbill on balance above median).
/// Only the *excess* above median is decayed.
pub fn apply_wealth_decay<Balance>(
    balance: Balance,
    median: Balance,
    rate: Perbill,
) -> Balance
where
    Balance: Saturating + Copy + PartialOrd + From<u32> + Into<u64>,
{
    if balance <= median {
        return balance;
    }
    let excess: u64 = balance.saturating_sub(median).into();
    // decay = rate * excess (parts-per-billion)
    let decay_u64 = excess * rate.deconstruct() as u64 / 1_000_000_000;
    let decay = Balance::from(decay_u64 as u32);
    balance.saturating_sub(decay)
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn zero_days_no_decay() {
        let b = apply_inactivity_decay(1_000u32, 100, 100, Perbill::from_percent(1));
        assert_eq!(b, 1_000);
    }

    #[test]
    fn wealth_decay_subtracts_excess() {
        // balance=2000, median=1000, rate=10%
        // excess=1000, decay=100, result=1900
        let b = apply_wealth_decay(2_000u32, 1_000u32, Perbill::from_percent(10));
        assert_eq!(b, 1_900);
    }

    #[test]
    fn wealth_decay_no_decay_below_median() {
        let b = apply_wealth_decay(800u32, 1_000u32, Perbill::from_percent(10));
        assert_eq!(b, 800);
    }

    #[test]
    fn days_capped() {
        assert_eq!(days_active_cap(500), 365);
        assert_eq!(days_active_cap(10), 10);
    }
}
