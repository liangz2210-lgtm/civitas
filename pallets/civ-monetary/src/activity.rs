//! # Activity Score — Social Participation Tracker
//!
//! Each verified account accumulates an activity score based on:
//!   - UBI claim frequency (baseline)
//!   - Transfer participation (circulation signal)
//!   - Governance participation (vote count)
//!   - Referral activity (referrals made / successful)
//!
//! The `ActivityFactor` multiplies the base UBI:
//!   activity_factor = 0.5 + 0.5 * (score / MAX_SCORE)
//!
//! So a totally inactive account still receives 50% of base UBI,
//! and a maximally active account receives 100%.
//!
//! V0.1: manual score updates. V0.2: automatic hooks from
//! transfers / governance / referral pallets.

#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime::traits::Saturating;

/// Maximum activity score (cap).
pub const MAX_ACTIVITY_SCORE: u32 = 10_000;

/// Minimum activity factor = 0.5 (in perbill).
pub const MIN_ACTIVITY_FACTOR: sp_runtime::Perbill = sp_runtime::Perbill::from_percent(50);

/// Compute the activity factor from a raw score.
/// Returns Perbill in [50%, 100%].
pub fn activity_factor(score: u32) -> sp_runtime::Perbill {
    if score >= MAX_ACTIVITY_SCORE {
        sp_runtime::Perbill::one()
    } else {
        let base = sp_runtime::Perbill::from_percent(50);
        let bonus = sp_runtime::Perbill::from_parts(
            (score as u64 * 500_000_000 / MAX_ACTIVITY_SCORE as u64) as u32,
        );
        base.saturating_add(bonus)
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use sp_runtime::Perbill;

    #[test]
    fn zero_score_gives_min_factor() {
        assert_eq!(activity_factor(0), Perbill::from_percent(50));
    }

    #[test]
    fn max_score_gives_full_factor() {
        assert_eq!(
            activity_factor(MAX_ACTIVITY_SCORE),
            Perbill::from_percent(100)
        );
    }

    #[test]
    fn half_score_gives_75_percent() {
        let f = activity_factor(MAX_ACTIVITY_SCORE / 2);
        assert_eq!(f, Perbill::from_percent(75));
    }
}
