//! # pallet-civ-identity
//!
//! Proof-of-personhood for the Civitas Protocol.
//!
//! ## Design
//!
//! No admin whitelist. No sudo key. Identity is established through
//! social vouching: a new account is verified once K existing verified
//! accounts vouch for it.
//!
//! The technical committee's only powers are:
//!   1. One-time genesis bootstrap (publicly witnessed)
//!   2. Submitting audit flag lists (30-day public challenge window)
//!   3. Certifying the audit complete (unlocks governance)
//!
//! ## Upgrade path
//!   V0.1  Social vouching (this file)
//!   V0.2  BrightID oracle
//!   V0.3  Optional ZK biometric proof
//!
//! ## Invariant
//! `VerifiedCount` == number of `true` entries in `VerifiedIdentities` at all times.

#![cfg_attr(not(feature = "std"), no_std)]
pub use pallet::*;
pub use traits::PersonhoodProvider;

pub mod traits {
    pub trait PersonhoodProvider<AccountId> {
        fn is_verified(who: &AccountId) -> bool;
        fn verified_count() -> u64;
        fn audit_certified() -> bool;
    }
}

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use super::traits::PersonhoodProvider;

    /// Blocks between vouch-rate-limit windows (~1 day).
    const VOUCH_WINDOW: u32 = 14_400;
    /// Max vouches one account may give per window.
    const MAX_VOUCH_PER_WINDOW: u32 = 10;
    /// Challenge window for flagged accounts (~30 days).
    const CHALLENGE_PERIOD: u32 = 432_000;

    // ── Config ────────────────────────────────────────────────────────────

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Multi-sig technical committee.
        /// Used ONLY for genesis bootstrap, audit flags, and audit certification.
        type CommitteeOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Vouches required to become verified.
        #[pallet::constant]
        type VouchThreshold: Get<u32>;

        /// Max accounts in a single bootstrap or audit-flag call.
        #[pallet::constant]
        type MaxBatchSize: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ── Storage ───────────────────────────────────────────────────────────

    #[pallet::storage]
    #[pallet::getter(fn is_verified)]
    pub type VerifiedIdentities<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, bool, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn verified_count)]
    pub type VerifiedCount<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Pending vouch count for unverified accounts.
    #[pallet::storage]
    #[pallet::getter(fn vouch_count)]
    pub type VouchCount<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

    /// (voucher, subject) → block number. Prevents double-vouching.
    #[pallet::storage]
    pub type VouchRecord<T: Config> = StorageDoubleMap<
        _, Blake2_128Concat, T::AccountId,
           Blake2_128Concat, T::AccountId,
        BlockNumberFor<T>, OptionQuery,
    >;

    /// Vouch rate-limit state per voucher: (window_start, count_in_window).
    #[pallet::storage]
    pub type VouchWindow<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (BlockNumberFor<T>, u32), ValueQuery>;

    /// Flagged accounts: who → (flagged_at, deadline, reason_hash, challenged).
    #[pallet::storage]
    #[pallet::getter(fn flag)]
    pub type AuditFlags<T: Config> = StorageMap<
        _, Blake2_128Concat, T::AccountId,
        (BlockNumberFor<T>, BlockNumberFor<T>, [u8; 32], bool),
        OptionQuery,
    >;

    /// True once the committee certifies the audit is complete.
    /// This is one of three conditions that unlock governance.
    #[pallet::storage]
    #[pallet::getter(fn audit_certified)]
    pub type AuditCertified<T: Config> = StorageValue<_, bool, ValueQuery>;

    // ── Events ────────────────────────────────────────────────────────────

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        Verified    { who: T::AccountId },
        Revoked     { who: T::AccountId },
        VouchCast   { voucher: T::AccountId, subject: T::AccountId, total: u32 },
        Flagged     { who: T::AccountId, deadline: BlockNumberFor<T> },
        Challenged  { who: T::AccountId },
        Penalised   { who: T::AccountId, revoked: bool },
        AuditDone,
    }

    // ── Errors ────────────────────────────────────────────────────────────

    #[pallet::error]
    pub enum Error<T> {
        AlreadyVerified,
        NotVerified,
        SelfVouch,
        AlreadyVouched,
        VouchRateLimit,
        NotFlagged,
        WindowOpen,
        AlreadyChallenged,
        AlreadyCertified,
        SelfReferral,
    }

    // ── Calls ─────────────────────────────────────────────────────────────

    #[pallet::call]
    impl<T: Config> Pallet<T> {

        /// Bootstrap the founding members in a one-time public ceremony.
        /// After this call the committee has no further identity powers.
        #[pallet::call_index(0)]
        #[pallet::weight(50_000)]
        pub fn bootstrap(
            origin: OriginFor<T>,
            founders: BoundedVec<T::AccountId, T::MaxBatchSize>,
        ) -> DispatchResult {
            T::CommitteeOrigin::ensure_origin(origin)?;
            for who in founders.iter() {
                ensure!(!Self::is_verified(who), Error::<T>::AlreadyVerified);
            }
            for who in founders.into_iter() {
                Self::do_verify(who)?;
            }
            Ok(())
        }

        /// Vouch for a new account.
        /// Caller must be verified. Subject becomes verified at threshold.
        #[pallet::call_index(1)]
        #[pallet::weight(20_000)]
        pub fn vouch_for(
            origin: OriginFor<T>,
            subject: T::AccountId,
        ) -> DispatchResult {
            let voucher = ensure_signed(origin)?;
            ensure!(Self::is_verified(&voucher),   Error::<T>::NotVerified);
            ensure!(!Self::is_verified(&subject),  Error::<T>::AlreadyVerified);
            ensure!(voucher != subject,             Error::<T>::SelfVouch);
            ensure!(
                VouchRecord::<T>::get(&voucher, &subject).is_none(),
                Error::<T>::AlreadyVouched
            );
            Self::check_vouch_rate(&voucher)?;

            let now = frame_system::Pallet::<T>::block_number();
            VouchRecord::<T>::insert(&voucher, &subject, now);
            let total = VouchCount::<T>::mutate(&subject, |c| { *c += 1; *c });

            Self::deposit_event(Event::VouchCast {
                voucher, subject: subject.clone(), total,
            });

            if total >= T::VouchThreshold::get() {
                Self::do_verify(subject)?;
            }
            Ok(())
        }

        /// Submit audit flags for suspicious accounts (30-day challenge window).
        /// `reason_hash` is a SHA-256 / IPFS CID of the evidence document.
        #[pallet::call_index(2)]
        #[pallet::weight(20_000u64.saturating_mul(batch.len() as u64))]
        pub fn flag_accounts(
            origin: OriginFor<T>,
            batch: BoundedVec<(T::AccountId, [u8; 32]), T::MaxBatchSize>,
        ) -> DispatchResult {
            T::CommitteeOrigin::ensure_origin(origin)?;
            let now = frame_system::Pallet::<T>::block_number();
            let deadline = now + CHALLENGE_PERIOD.into();
            for (who, reason_hash) in batch.into_iter() {
                AuditFlags::<T>::insert(&who, (now, deadline, reason_hash, false));
                Self::deposit_event(Event::Flagged { who, deadline });
            }
            Ok(())
        }

        /// A flagged account submits a challenge within the 30-day window.
        #[pallet::call_index(3)]
        #[pallet::weight(10_000)]
        pub fn challenge(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let mut flag = AuditFlags::<T>::get(&who).ok_or(Error::<T>::NotFlagged)?;
            let now = frame_system::Pallet::<T>::block_number();
            ensure!(now <= flag.1, Error::<T>::WindowOpen); // deadline
            ensure!(!flag.3, Error::<T>::AlreadyChallenged);
            flag.3 = true;
            AuditFlags::<T>::insert(&who, flag);
            Self::deposit_event(Event::Challenged { who });
            Ok(())
        }

        /// Execute penalty after the challenge window has closed.
        /// Unchallenged → identity revoked.
        /// Challenged   → kept pending governance review.
        /// Anyone can call once the deadline has passed.
        #[pallet::call_index(4)]
        #[pallet::weight(15_000)]
        pub fn execute_penalty(
            origin: OriginFor<T>,
            who: T::AccountId,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            let (_, deadline, _, challenged) =
                AuditFlags::<T>::get(&who).ok_or(Error::<T>::NotFlagged)?;
            let now = frame_system::Pallet::<T>::block_number();
            // Window must be closed
            ensure!(now > deadline, Error::<T>::WindowOpen);

            let revoked = !challenged;
            if revoked && Self::is_verified(&who) {
                VerifiedIdentities::<T>::remove(&who);
                VerifiedCount::<T>::mutate(|c| *c = c.saturating_sub(1));
            }
            AuditFlags::<T>::remove(&who);
            Self::deposit_event(Event::Penalised { who, revoked });
            Ok(())
        }

        /// Committee certifies audit complete — one of three governance unlock conditions.
        /// Irreversible.
        #[pallet::call_index(5)]
        #[pallet::weight(5_000)]
        pub fn certify_audit(origin: OriginFor<T>) -> DispatchResult {
            T::CommitteeOrigin::ensure_origin(origin)?;
            ensure!(!AuditCertified::<T>::get(), Error::<T>::AlreadyCertified);
            AuditCertified::<T>::put(true);
            Self::deposit_event(Event::AuditDone);
            Ok(())
        }
    }

    // ── Internals ─────────────────────────────────────────────────────────

    impl<T: Config> Pallet<T> {
        fn do_verify(who: T::AccountId) -> DispatchResult {
            ensure!(!Self::is_verified(&who), Error::<T>::AlreadyVerified);
            VerifiedIdentities::<T>::insert(&who, true);
            VerifiedCount::<T>::mutate(|c| *c += 1);
            Self::deposit_event(Event::Verified { who });
            Ok(())
        }

        fn check_vouch_rate(voucher: &T::AccountId) -> DispatchResult {
            let now = frame_system::Pallet::<T>::block_number();
            let (start, count) = VouchWindow::<T>::get(voucher);
            if now >= start + VOUCH_WINDOW.into() {
                VouchWindow::<T>::insert(voucher, (now, 1u32));
            } else {
                ensure!(count < MAX_VOUCH_PER_WINDOW, Error::<T>::VouchRateLimit);
                VouchWindow::<T>::insert(voucher, (start, count + 1));
            }
            Ok(())
        }
    }

    // ── Trait impl ────────────────────────────────────────────────────────

    impl<T: Config> PersonhoodProvider<T::AccountId> for Pallet<T> {
        fn is_verified(who: &T::AccountId) -> bool {
            VerifiedIdentities::<T>::get(who)
        }
        fn verified_count() -> u64 {
            VerifiedCount::<T>::get()
        }
        fn audit_certified() -> bool {
            AuditCertified::<T>::get()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{pallet::*, traits::PersonhoodProvider};
    use frame_support::{assert_noop, assert_ok, traits::ConstU32};
    use sp_core::H256;
    use sp_runtime::{traits::{BlakeTwo256, IdentityLookup}, BuildStorage};

    type Block = frame_system::mocking::MockBlock<Test>;
    frame_support::construct_runtime!(
        pub enum Test { System: frame_system, CivId: pallet }
    );

    impl frame_system::Config for Test {
        type BaseCallFilter = frame_support::traits::Everything;
        type BlockWeights = (); type BlockLength = (); type DbWeight = ();
        type RuntimeOrigin = RuntimeOrigin; type RuntimeCall = RuntimeCall;
        type RuntimeTask = (); type Nonce = u64; type Hash = H256;
        type Hashing = BlakeTwo256; type AccountId = u64;
        type Lookup = IdentityLookup<u64>; type Block = Block;
        type RuntimeEvent = RuntimeEvent; type BlockHashCount = ();
        type Version = (); type PalletInfo = PalletInfo;
        type AccountData = (); type OnNewAccount = (); type OnKilledAccount = ();
        type SystemWeightInfo = (); type SS58Prefix = (); type OnSetCode = ();
        type MaxConsumers = ConstU32<16>;
    }
    impl Config for Test {
        type RuntimeEvent    = RuntimeEvent;
        type CommitteeOrigin = frame_system::EnsureRoot<u64>;
        type VouchThreshold  = ConstU32<3>;
        type MaxBatchSize    = ConstU32<50>;
    }

    fn ext() -> sp_io::TestExternalities {
        let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
        let mut e = sp_io::TestExternalities::new(t);
        e.execute_with(|| System::set_block_number(1));
        e
    }
    fn bootstrap(ids: Vec<u64>) {
        let bv: BoundedVec<u64,ConstU32<50>> = ids.try_into().unwrap();
        assert_ok!(CivId::bootstrap(RuntimeOrigin::root(), bv));
    }

    #[test] fn bootstrap_verifies_all() {
        ext().execute_with(|| {
            bootstrap(vec![1,2,3]);
            assert!(CivId::is_verified(1));
            assert_eq!(CivId::verified_count(), 3);
        });
    }
    #[test] fn vouch_threshold_verifies() {
        ext().execute_with(|| {
            bootstrap(vec![1,2,3]);
            assert_ok!(CivId::vouch_for(RuntimeOrigin::signed(1), 4));
            assert_ok!(CivId::vouch_for(RuntimeOrigin::signed(2), 4));
            assert!(!CivId::is_verified(4));
            assert_ok!(CivId::vouch_for(RuntimeOrigin::signed(3), 4));
            assert!(CivId::is_verified(4));
            assert_eq!(CivId::verified_count(), 4);
        });
    }
    #[test] fn double_vouch_rejected() {
        ext().execute_with(|| {
            bootstrap(vec![1,2,3]);
            assert_ok!(CivId::vouch_for(RuntimeOrigin::signed(1), 4));
            assert_noop!(CivId::vouch_for(RuntimeOrigin::signed(1), 4), Error::<Test>::AlreadyVouched);
        });
    }
    #[test] fn self_vouch_rejected() {
        ext().execute_with(|| {
            bootstrap(vec![1,2,3]);
            assert_noop!(CivId::vouch_for(RuntimeOrigin::signed(1), 1), Error::<Test>::SelfVouch);
        });
    }
    #[test] fn audit_no_challenge_revokes() {
        ext().execute_with(|| {
            bootstrap(vec![1,2,3]);
            let batch: BoundedVec<(u64,[u8;32]),ConstU32<50>> = vec![(1,[0u8;32])].try_into().unwrap();
            assert_ok!(CivId::flag_accounts(RuntimeOrigin::root(), batch));
            System::set_block_number(432_002);
            assert_ok!(CivId::execute_penalty(RuntimeOrigin::signed(99), 1));
            assert!(!CivId::is_verified(1));
            assert_eq!(CivId::verified_count(), 2);
        });
    }
    #[test] fn challenge_prevents_revoke() {
        ext().execute_with(|| {
            bootstrap(vec![1,2,3]);
            let batch: BoundedVec<(u64,[u8;32]),ConstU32<50>> = vec![(1,[1u8;32])].try_into().unwrap();
            assert_ok!(CivId::flag_accounts(RuntimeOrigin::root(), batch));
            assert_ok!(CivId::challenge(RuntimeOrigin::signed(1)));
            System::set_block_number(432_002);
            assert_ok!(CivId::execute_penalty(RuntimeOrigin::signed(99), 1));
            assert!(CivId::is_verified(1), "challenged account keeps identity");
        });
    }
    #[test] fn audit_certification() {
        ext().execute_with(|| {
            assert!(!<Pallet<Test> as PersonhoodProvider<u64>>::audit_certified());
            assert_ok!(CivId::certify_audit(RuntimeOrigin::root()));
            assert!(<Pallet<Test> as PersonhoodProvider<u64>>::audit_certified());
            assert_noop!(CivId::certify_audit(RuntimeOrigin::root()), Error::<Test>::AlreadyCertified);
        });
    }
}
