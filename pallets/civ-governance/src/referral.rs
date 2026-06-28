//! # Referral helpers — shared by governance & identity pallets
//!
//! Shared data structures and pure functions for the Civitas referral
//! system.  Both `pallet-civ-governance` and `pallet-civ-identity`
//! depend on this module.

use frame_support::pallet_prelude::*;

/// Maximum referrals per user.
pub const MAX_REFERRALS_PER_USER: u32 = 100;

/// A single referral record.
///
/// Uses independent generic parameters instead of `T: Config`
/// so that the `TypeInfo` bound is satisfied without requiring
/// `T: TypeInfo` on the pallet Config.
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
)]
pub struct ReferralRecord<AccountId, BlockNumber> {
    /// The referrer (person who invited).
    pub referrer: AccountId,
    /// The referral (person who was invited).
    pub referral: AccountId,
    /// Block number when the referral was recorded.
    pub created_at: BlockNumber,
    /// Whether the referral has been activated (referrer verified).
    pub activated: bool,
}

/// Compute a referral score for a referrer given their referrals.
/// V0.1: simple count of activated referrals.
/// V0.2: time-weighted + activity-weighted.
pub fn referral_score<AccountId, BlockNumber>(
    referrals: &[ReferralRecord<AccountId, BlockNumber>],
) -> u32 {
    referrals.iter().filter(|r| r.activated).count() as u32
}

/// Check if a referrer has room for more referrals.
pub fn can_refer(count: u32) -> bool {
    count < MAX_REFERRALS_PER_USER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_refer_allows_up_to_max() {
        assert!(can_refer(0));
        assert!(can_refer(99));
        assert!(!can_refer(100));
    }
}
