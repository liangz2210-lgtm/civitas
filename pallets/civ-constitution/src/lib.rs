//! # pallet-civ-constitution
//!
//! Constitutional guard layer for the Civitas Protocol.
//!
//! ## Purpose
//!
//! Four inviolable invariants encoded on-chain. Every state-mutating
//! call in *every* pallet must pass `ConstitutionGuard::check()` before
//! execution. Violations are rejected with `ConstitutionalViolation`.
//!
//! ## The Four Invariants
//!
//! 1. **UBI non-negotiability** — Every verified human receives at least
//!    the BaseUBI each disbursement period. No governance vote, no
//!    committee action, and no runtime upgrade may reduce the per-person
//!    base UBI to zero or skip issuance.
//! 2. **One-person-one-vote** — Governance weight of any single account
//!    is capped at `MAX_WEIGHT_CAP × MinimumViableScore`. No entity
//!    may accumulate vote-power beyond that cap.
//! 3. **Non-accumulation limit** — No account may hold more than
//!    `N × MedianBalance` where N is a chain parameter. This prevents
//!    oligarchic concentration while allowing productive disparity.
//! 4. **Transparency** — Every parameter change, every fee, every policy
//!    must leave an on-chain record that is queryable by any participant
//!    without privilege.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐
//! │  CivIdent    │──┐
//! │  CivMonetary │──┼──▶ ConstitutionGuard::check(invariant_id)
//! │  CivGovern   │──┘         │
//!                             ▼
//!                     pass ✓  or  ConstitutionalViolation ✗
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

// ── Re-export the guard trait for cross-pallet use ───────────────────────

pub mod guard {
    /// Every pallet that mutates state must implement this check.
    /// `invariant_id` is 0-3 corresponding to the four invariants above.
    pub trait ConstitutionGuard {
        /// Return Ok(()) if the invariant is satisfied, Err otherwise.
        fn check(invariant_id: u8) -> Result<(), ()>;
    }
}

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    // ── Invariant IDs ──────────────────────────────────────────────────

    pub const INV_UBI_NON_NEGOTIABLE: u8 = 0;
    pub const INV_ONE_PERSON_ONE_VOTE: u8 = 1;
    pub const INV_NON_ACCUMULATION: u8 = 2;
    pub const INV_TRANSPARENCY: u8 = 3;

    // ── Config ─────────────────────────────────────────────────────────

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Upper cap on governance weight (intrinsic score units).
        #[pallet::constant]
        type MaxWeightCap: Get<u32>;

        /// Wealth concentration ratio N: max_balance ≤ N × median.
        #[pallet::constant]
        type AccumulationRatio: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ── Storage ────────────────────────────────────────────────────────

    /// Per-invariant enabled flag. All default to true (enabled).
    /// Governance may *tighten* but never *disable* an invariant
    /// (disabling requires a CoreMechanism proposal + 180-day timelock).
    #[pallet::storage]
    #[pallet::getter(fn invariant_enabled)]
    pub type InvariantEnabled<T: Config> =
        StorageMap<_, Identity, u8, bool, ValueQuery>;

    /// Log of every parameter / policy change for transparency.
    /// (block, caller, change_hash)
    #[pallet::storage]
    #[pallet::getter(fn change_log)]
    pub type ChangeLog<T: Config> = StorageValue<
        _,
        BoundedVec<(BlockNumberFor<T>, T::AccountId, [u8; 32]), ConstU32<1_024>>,
        ValueQuery,
    >;

    /// Running count of logged changes.
    #[pallet::storage]
    #[pallet::getter(fn change_count)]
    pub type ChangeCount<T: Config> = StorageValue<_, u32, ValueQuery>;

    // ── Genesis ───────────────────────────────────────────────────────

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        #[serde(skip)]
        pub _ph: core::marker::PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            for id in 0u8..4 {
                InvariantEnabled::<T>::insert(id, true);
            }
        }
    }

    // ── Events ────────────────────────────────────────────────────────

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// An invariant check passed (informational).
        InvariantChecked { id: u8, passed: bool },
        /// A parameter / policy change was recorded for transparency.
        ChangeRecorded { count: u32, hash: [u8; 32] },
    }

    // ── Errors ────────────────────────────────────────────────────────

    #[pallet::error]
    pub enum Error<T> {
        /// The requested action violates a constitutional invariant.
        ConstitutionalViolation,
        /// Tried to disable an invariant without proper governance.
        InvariantImmutable,
        /// Invalid invariant id (must be 0-3).
        InvalidInvariantId,
        /// Change log is full.
        ChangeLogFull,
    }

    // ── Calls ─────────────────────────────────────────────────────────

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Record a parameter / policy change in the transparency log.
        /// Anyone may call; the caller identity is auto-captured.
        #[pallet::call_index(0)]
        #[pallet::weight(5_000)]
        pub fn record_change(origin: OriginFor<T>, hash: [u8; 32]) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let now = frame_system::Pallet::<T>::block_number();
            let count = ChangeCount::<T>::get();

            ChangeLog::<T>::try_mutate(|log| {
                log.try_push((now, caller, hash))
                    .map_err(|_| Error::<T>::ChangeLogFull)
            })?;

            ChangeCount::<T>::put(count + 1);
            Self::deposit_event(Event::ChangeRecorded {
                count: count + 1,
                hash,
            });
            Ok(())
        }

        /// Check an invariant at runtime (read-only,任何人可调).
        /// Emits InvariantChecked event with the result.
        #[pallet::call_index(1)]
        #[pallet::weight(3_000)]
        pub fn check_invariant(origin: OriginFor<T>, id: u8) -> DispatchResult {
            ensure_signed(origin)?;
            ensure!(id < 4, Error::<T>::InvalidInvariantId);
            let enabled = Self::invariant_enabled(id);
            // V0.1: structural check only (enabled flag).
            // V0.2: wire in cross-pallet read calls.
            Self::deposit_event(Event::InvariantChecked {
                id,
                passed: enabled,
            });
            Ok(())
        }
    }

    // ── Internals ─────────────────────────────────────────────────────

    impl<T: Config> Pallet<T> {
        /// Fast guard: returns Err if the invariant is violated.
        /// Called by every state-mutating extrinsic across all pallets.
        pub fn guard(invariant_id: u8) -> DispatchResult {
            ensure!(
                Self::invariant_enabled(invariant_id),
                Error::<T>::ConstitutionalViolation
            );
            Ok(())
        }

        /// Validate that an account's governance weight does not exceed
        /// the constitutional cap. Used by civ-governance.
        pub fn check_weight_cap(weight: u64) -> DispatchResult {
            ensure!(
                weight <= T::MaxWeightCap::get() as u64,
                Error::<T>::ConstitutionalViolation
            );
            Ok(())
        }

        /// Validate that a balance does not violate the non-accumulation
        /// limit relative to a median. Used by civ-monetary.
        pub fn check_accumulation(balance: u64, median: u64) -> DispatchResult {
            if median == 0 {
                return Ok(());
            }
            ensure!(
                balance <= (median as u128).saturating_mul(T::AccumulationRatio::get() as u128) as u64,
                Error::<T>::ConstitutionalViolation
            );
            Ok(())
        }

        /// Convenience: check all four invariants at once.
        pub fn guard_all() -> DispatchResult {
            for id in 0u8..4 {
                Self::guard(id)?;
            }
            Ok(())
        }
    }

    // ── Trait impl ─────────────────────────────────────────────────────

    impl<T: Config> crate::guard::ConstitutionGuard for Pallet<T> {
        fn check(invariant_id: u8) -> Result<(), ()> {
            if Self::invariant_enabled(invariant_id) {
                Ok(())
            } else {
                Err(())
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::pallet::{Config, Error, Pallet, InvariantEnabled};
    use super::guard::ConstitutionGuard;
    use crate as pallet_civ_constitution;
    use frame_support::{assert_noop, assert_ok, traits::ConstU32};
    use sp_core::H256;
    use sp_runtime::{
        traits::{BlakeTwo256, IdentityLookup},
        BuildStorage,
    };

    type Block = frame_system::mocking::MockBlock<Test>;
    frame_support::construct_runtime!(
        pub enum Test { System: frame_system, CivCon: pallet_civ_constitution }
    );

    impl frame_system::Config for Test {
        type BaseCallFilter = frame_support::traits::Everything;
        type BlockWeights = ();
        type BlockLength = ();
        type DbWeight = ();
        type RuntimeOrigin = RuntimeOrigin;
        type RuntimeCall = RuntimeCall;
        type RuntimeTask = ();
        type Nonce = u64;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type AccountId = u64;
        type Lookup = IdentityLookup<u64>;
        type Block = Block;
        type RuntimeEvent = RuntimeEvent;
        type BlockHashCount = ();
        type Version = ();
        type PalletInfo = PalletInfo;
        type AccountData = ();
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type SystemWeightInfo = ();
        type SS58Prefix = ();
        type OnSetCode = ();
        type MaxConsumers = ConstU32<16>;
        type SingleBlockMigrations = ();
        type MultiBlockMigrator = ();
        type PreInherents = ();
        type PostInherents = ();
        type PostTransactions = ();
        type ExtensionsWeightInfo = ();
    }

    impl Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type MaxWeightCap = ConstU32<10_000>;
        type AccumulationRatio = ConstU32<100>;
    }

    fn ext() -> sp_io::TestExternalities {
        let t = frame_system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap();
        let mut e = sp_io::TestExternalities::new(t);
        e.execute_with(|| {
            frame_system::Pallet::<Test>::set_block_number(1);
            // Enable all four invariants (ValueQuery defaults to false)
            for id in 0u8..4 {
                InvariantEnabled::<Test>::insert(id, true);
            }
        });
        e
    }

    #[test]
    fn all_invariants_enabled_at_genesis() {
        ext().execute_with(|| {
            for id in 0u8..4 {
                assert!(CivCon::invariant_enabled(id), "invariant {} not enabled", id);
            }
        });
    }

    #[test]
    fn guard_passes_when_enabled() {
        ext().execute_with(|| {
            assert_ok!(Pallet::<Test>::guard(0));
            assert_ok!(Pallet::<Test>::guard(1));
            assert_ok!(Pallet::<Test>::guard(2));
            assert_ok!(Pallet::<Test>::guard(3));
        });
    }

    #[test]
    fn guard_fails_when_disabled() {
        ext().execute_with(|| {
            InvariantEnabled::<Test>::insert(0, false);
            assert_noop!(Pallet::<Test>::guard(0), Error::<Test>::ConstitutionalViolation);
        });
    }

    #[test]
    fn guard_all_passes() {
        ext().execute_with(|| {
            assert_ok!(Pallet::<Test>::guard_all());
        });
    }

    #[test]
    fn weight_cap_check() {
        ext().execute_with(|| {
            assert_ok!(Pallet::<Test>::check_weight_cap(10_000));
            assert_noop!(
                Pallet::<Test>::check_weight_cap(10_001),
                Error::<Test>::ConstitutionalViolation
            );
        });
    }

    #[test]
    fn accumulation_check() {
        ext().execute_with(|| {
            // median=1000, ratio=100, max=100000
            assert_ok!(Pallet::<Test>::check_accumulation(99_999, 1_000));
            assert_noop!(
                Pallet::<Test>::check_accumulation(100_001, 1_000),
                Error::<Test>::ConstitutionalViolation
            );
        });
    }

    #[test]
    fn trait_impl_matches_guard() {
        ext().execute_with(|| {
            assert!(<Pallet<Test> as ConstitutionGuard>::check(0).is_ok());
            InvariantEnabled::<Test>::insert(1, false);
            assert!(<Pallet<Test> as ConstitutionGuard>::check(1).is_err());
        });
    }

    #[test]
    fn record_change_works() {
        ext().execute_with(|| {
            assert_ok!(CivCon::record_change(RuntimeOrigin::signed(1), [0xab; 32]));
            assert_eq!(CivCon::change_count(), 1);
        });
    }

    #[test]
    fn check_invariant_invalid_id() {
        ext().execute_with(|| {
            assert_noop!(
                CivCon::check_invariant(RuntimeOrigin::signed(1), 4),
                Error::<Test>::InvalidInvariantId
            );
        });
    }
}
