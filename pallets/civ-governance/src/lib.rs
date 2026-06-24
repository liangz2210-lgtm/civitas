#![allow(deprecated)]
#![allow(clippy::let_unit_value)]
#![allow(clippy::type_complexity)]

//! # pallet-civ-governance
//!
//! Governance layer for the Civitas Protocol.
//!
//! ## Governance unlock — all three conditions must be met
//!
//! ```text
//! 1. Chain age  ≥ 180 days  (prevents rushed attacks at launch)
//! 2. Verified   ≥ 10,000    (prevents small-circle capture)
//! 3. Audit certified = true (cleans up pre-audit Sybil accounts first)
//! ```
//!
//! Before unlock the technical committee controls all parameters.
//! After unlock every change requires a participant vote.
//!
//! ## Account age weight (anti-Sybil governance protection)
//!
//! ```text
//! AgeFactor = min(account_age / ONE_YEAR, 1.0)
//! VoteWeight = GovernanceScore × AgeFactor
//! ```
//!
//! An account created before the audit but never re-verified has zero
//! voting power for one year. This neutralises history-laundering attacks
//! even if the audit missed some fake accounts.
//!
//! ## Four-dimensional governance score
//!
//! ```text
//! V(u) = w1·log2(B+1) + w2·T + w3·R + w4·D
//!
//! B = balance (log-compressed)
//! T = circulation (unique-counterparty transfers)
//! R = referral  (long-term activity of referred users)
//! D = donation  (voluntary protocol contributions)
//! ```
//!
//! ## Tiered proposals
//!
//! | Level | Scope           | Quorum | Timelock |
//! |-------|-----------------|--------|----------|
//! | 1     | Parameters      | 50 %   | 7 days   |
//! | 2     | Feature upgrade | 66 %   | 30 days  |
//! | 3     | Core mechanism  | 80 %   | 180 days |
//!
//! ## Public-goods funding
//!
//! ```text
//! Fund = SelfFund + CommunityFund + min(CommunityFund × M, Target)
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, WithdrawReasons},
    };
    use frame_system::pallet_prelude::*;
    use pallet_civ_identity::PersonhoodProvider;
    use sp_runtime::traits::{Saturating, Zero};

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // ── Constants ─────────────────────────────────────────────────────────

    const MIN_VERIFIED: u64 = 10_000;
    const MIN_CHAIN_AGE: u32 = 2_592_000; // ~180 days
    const ONE_YEAR: u32 = 5_256_000;
    const VOTING_PERIOD: u32 = 100_800; // ~7 days
    const TIMELOCK_L1: u32 = 100_800;
    const TIMELOCK_L2: u32 = 432_000;
    const TIMELOCK_L3: u32 = 2_592_000;
    const MIN_SCORE: u64 = 100;
    const MAX_MULTIPLIER: u32 = 5;

    // ── Types ─────────────────────────────────────────────────────────────

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
    pub enum Level {
        Parameter,
        Feature,
        CoreMechanism,
    }

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
    pub enum Status {
        Active,
        Passed,
        ReadyToExecute,
        Executed,
        Rejected,
    }

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
    #[scale_info(skip_type_params(T))]
    pub struct Proposal<T: Config> {
        pub id: u32,
        pub proposer: T::AccountId,
        pub level: Level,
        /// 32-byte SHA-256 / IPFS CID of the proposal text.
        pub hash: [u8; 32],
        pub aye: u64,
        pub nay: u64,
        pub status: Status,
        pub created: BlockNumberFor<T>,
        pub execute_after: BlockNumberFor<T>,
    }

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
    #[scale_info(skip_type_params(T))]
    pub struct Project<T: Config> {
        pub id: u32,
        pub proposer: T::AccountId,
        pub hash: [u8; 32],
        pub target: BalanceOf<T>,
        pub self_fund: BalanceOf<T>,
        pub community: BalanceOf<T>,
        pub matched: BalanceOf<T>,
        pub funded: bool,
    }

    // ── Config ────────────────────────────────────────────────────────────

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: Currency<Self::AccountId>;
        type Personhood: PersonhoodProvider<Self::AccountId>;
        /// Technical committee — controls params before governance unlocks.
        type CommitteeOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// Score dimension weights (must sum to 100).
        #[pallet::constant]
        type WBalance: Get<u32>;
        #[pallet::constant]
        type WCirculation: Get<u32>;
        #[pallet::constant]
        type WReferral: Get<u32>;
        #[pallet::constant]
        type WDonation: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ── Storage ───────────────────────────────────────────────────────────

    /// Block at which an account was first verified (age-weight baseline).
    #[pallet::storage]
    #[pallet::getter(fn verified_at)]
    pub type VerifiedAt<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BlockNumberFor<T>, OptionQuery>;

    /// Aggregate governance score.
    #[pallet::storage]
    #[pallet::getter(fn score)]
    pub type Score<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;

    /// Score components (B, T, R, D) for transparency.
    #[pallet::storage]
    pub type Components<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (u64, u64, u64, u64), ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_proposal)]
    pub type NextProposal<T: Config> = StorageValue<_, u32, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn proposals)]
    pub type Proposals<T: Config> = StorageMap<_, Twox64Concat, u32, Proposal<T>, OptionQuery>;

    #[pallet::storage]
    pub type Votes<T: Config> =
        StorageDoubleMap<_, Twox64Concat, u32, Blake2_128Concat, T::AccountId, bool, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_project)]
    pub type NextProject<T: Config> = StorageValue<_, u32, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn projects)]
    pub type Projects<T: Config> = StorageMap<_, Twox64Concat, u32, Project<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn match_multiplier)]
    pub type MatchMultiplier<T: Config> = StorageValue<_, u32, ValueQuery>;

    // ── Genesis ───────────────────────────────────────────────────────────

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        pub match_multiplier: Option<u32>,
        #[serde(skip)]
        pub _ph: core::marker::PhantomData<T>,
    }
    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            MatchMultiplier::<T>::put(self.match_multiplier.unwrap_or(2));
        }
    }

    // ── Events ────────────────────────────────────────────────────────────

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        GovernanceUnlocked,
        AgeRecorded {
            who: T::AccountId,
        },
        ScoreUpdated {
            who: T::AccountId,
            score: u64,
        },
        Proposed {
            id: u32,
            proposer: T::AccountId,
            level: Level,
        },
        Voted {
            id: u32,
            voter: T::AccountId,
            aye: bool,
            weight: u64,
        },
        StatusChanged {
            id: u32,
            status: Status,
        },
        ProjectOpened {
            id: u32,
            target: BalanceOf<T>,
        },
        Contributed {
            id: u32,
            by: T::AccountId,
            amount: BalanceOf<T>,
        },
        Matched {
            id: u32,
            amount: BalanceOf<T>,
        },
        Donated {
            by: T::AccountId,
            amount: BalanceOf<T>,
        },
    }

    // ── Errors ────────────────────────────────────────────────────────────

    #[pallet::error]
    pub enum Error<T> {
        NotVerified,
        Locked,
        LowScore,
        NotFound,
        AlreadyVoted,
        VotingClosed,
        NotPassed,
        TimelockActive,
        AlreadyFunded,
        InsufficientBalance,
    }

    // ── Calls ─────────────────────────────────────────────────────────────

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ── Score management ──────────────────────────────────────────────

        /// Record verification timestamp for age-weight calculation.
        /// Called once per account after it becomes verified.
        /// V0.2: replace with automatic cross-pallet hook.
        #[pallet::call_index(0)]
        #[pallet::weight(5_000)]
        pub fn record_age(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
            T::CommitteeOrigin::ensure_origin(origin)?;
            ensure!(T::Personhood::is_verified(&who), Error::<T>::NotVerified);
            if VerifiedAt::<T>::get(&who).is_none() {
                let now = frame_system::Pallet::<T>::block_number();
                VerifiedAt::<T>::insert(&who, now);
                Self::deposit_event(Event::AgeRecorded { who });
            }
            Ok(())
        }

        /// Increment T-component after a transfer (called by admin / hook).
        #[pallet::call_index(1)]
        #[pallet::weight(5_000)]
        pub fn record_transfer(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
            T::CommitteeOrigin::ensure_origin(origin)?;
            let (b, t, r, d) = Components::<T>::get(&who);
            Components::<T>::insert(&who, (b, t + 1, r, d));
            Self::recalc(&who);
            Ok(())
        }

        /// Donate to the protocol. Increments D-component.
        #[pallet::call_index(2)]
        #[pallet::weight(15_000)]
        pub fn donate(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(T::Personhood::is_verified(&who), Error::<T>::NotVerified);
            drop(T::Currency::withdraw(
                &who,
                amount,
                WithdrawReasons::TRANSFER,
                ExistenceRequirement::KeepAlive,
            )?);
            let (b, t, r, d) = Components::<T>::get(&who);
            let donated = amount.try_into().unwrap_or(u64::MAX);
            Components::<T>::insert(&who, (b, t, r, Self::log2(d.saturating_add(donated))));
            Self::recalc(&who);
            Self::deposit_event(Event::Donated { by: who, amount });
            Ok(())
        }

        // ── Governance unlock ─────────────────────────────────────────────

        /// Check and announce if governance has unlocked. Anyone can call.
        #[pallet::call_index(3)]
        #[pallet::weight(3_000)]
        pub fn check_unlock(origin: OriginFor<T>) -> DispatchResult {
            ensure_signed(origin)?;
            if Self::is_unlocked() {
                Self::deposit_event(Event::GovernanceUnlocked);
            }
            Ok(())
        }

        // ── Proposals ─────────────────────────────────────────────────────

        /// Submit a governance proposal.
        /// Governance must be unlocked and caller needs MIN_SCORE effective weight.
        #[pallet::call_index(4)]
        #[pallet::weight(25_000)]
        pub fn propose(origin: OriginFor<T>, level: Level, hash: [u8; 32]) -> DispatchResult {
            let proposer = ensure_signed(origin)?;
            ensure!(
                T::Personhood::is_verified(&proposer),
                Error::<T>::NotVerified
            );
            ensure!(Self::is_unlocked(), Error::<T>::Locked);
            ensure!(
                Self::weight_of(&proposer) >= MIN_SCORE,
                Error::<T>::LowScore
            );

            let now = frame_system::Pallet::<T>::block_number();
            let timelock = Self::timelock(&level);
            let id = NextProposal::<T>::mutate(|n| {
                let v = *n;
                *n += 1;
                v
            });

            Proposals::<T>::insert(
                id,
                Proposal {
                    id,
                    proposer: proposer.clone(),
                    level: level.clone(),
                    hash,
                    aye: 0,
                    nay: 0,
                    status: Status::Active,
                    created: now,
                    execute_after: now + VOTING_PERIOD.into() + timelock,
                },
            );
            Self::deposit_event(Event::Proposed {
                id,
                proposer,
                level,
            });
            Ok(())
        }

        /// Vote on an active proposal. Weight = Score × AgeFactor.
        #[pallet::call_index(5)]
        #[pallet::weight(15_000)]
        pub fn vote(origin: OriginFor<T>, id: u32, aye: bool) -> DispatchResult {
            let voter = ensure_signed(origin)?;
            ensure!(T::Personhood::is_verified(&voter), Error::<T>::NotVerified);
            ensure!(Self::is_unlocked(), Error::<T>::Locked);
            ensure!(
                Votes::<T>::get(id, &voter).is_none(),
                Error::<T>::AlreadyVoted
            );

            let now = frame_system::Pallet::<T>::block_number();
            let mut p = Proposals::<T>::get(id).ok_or(Error::<T>::NotFound)?;
            ensure!(p.status == Status::Active, Error::<T>::VotingClosed);
            ensure!(
                now < p.created + VOTING_PERIOD.into(),
                Error::<T>::VotingClosed
            );

            let w = Self::weight_of(&voter).max(1);
            if aye {
                p.aye = p.aye.saturating_add(w);
            } else {
                p.nay = p.nay.saturating_add(w);
            }

            Votes::<T>::insert(id, &voter, aye);
            Proposals::<T>::insert(id, &p);
            Self::deposit_event(Event::Voted {
                id,
                voter,
                aye,
                weight: w,
            });
            Ok(())
        }

        /// Finalise voting after the period ends.
        #[pallet::call_index(6)]
        #[pallet::weight(10_000)]
        pub fn finalise(origin: OriginFor<T>, id: u32) -> DispatchResult {
            ensure_signed(origin)?;
            let now = frame_system::Pallet::<T>::block_number();
            let mut p = Proposals::<T>::get(id).ok_or(Error::<T>::NotFound)?;
            ensure!(p.status == Status::Active, Error::<T>::VotingClosed);
            ensure!(
                now >= p.created + VOTING_PERIOD.into(),
                Error::<T>::VotingClosed
            );

            let quorum = match p.level {
                Level::Parameter => 50u64,
                Level::Feature => 66,
                Level::CoreMechanism => 80,
            };
            let total = p.aye.saturating_add(p.nay);
            let passed = total > 0 && p.aye.saturating_mul(100) >= total.saturating_mul(quorum);

            p.status = if passed {
                Status::Passed
            } else {
                Status::Rejected
            };
            let s = p.status.clone();
            Proposals::<T>::insert(id, p);
            Self::deposit_event(Event::StatusChanged { id, status: s });
            Ok(())
        }

        /// Mark passed proposal ready to execute once timelock has elapsed.
        #[pallet::call_index(7)]
        #[pallet::weight(5_000)]
        pub fn mark_ready(origin: OriginFor<T>, id: u32) -> DispatchResult {
            ensure_signed(origin)?;
            let now = frame_system::Pallet::<T>::block_number();
            let mut p = Proposals::<T>::get(id).ok_or(Error::<T>::NotFound)?;
            ensure!(p.status == Status::Passed, Error::<T>::NotPassed);
            ensure!(now >= p.execute_after, Error::<T>::TimelockActive);
            p.status = Status::ReadyToExecute;
            Proposals::<T>::insert(id, &p);
            Self::deposit_event(Event::StatusChanged {
                id,
                status: Status::ReadyToExecute,
            });
            Ok(())
        }

        // ── Public-goods funding ──────────────────────────────────────────

        /// Open a funding project. Proposer puts in skin-in-the-game deposit.
        #[pallet::call_index(8)]
        #[pallet::weight(15_000)]
        pub fn open_project(
            origin: OriginFor<T>,
            hash: [u8; 32],
            target: BalanceOf<T>,
            self_fund: BalanceOf<T>,
        ) -> DispatchResult {
            let proposer = ensure_signed(origin)?;
            ensure!(
                T::Personhood::is_verified(&proposer),
                Error::<T>::NotVerified
            );
            if !self_fund.is_zero() {
                drop(T::Currency::withdraw(
                    &proposer,
                    self_fund,
                    WithdrawReasons::TRANSFER,
                    ExistenceRequirement::KeepAlive,
                )?);
            }
            let id = NextProject::<T>::mutate(|n| {
                let v = *n;
                *n += 1;
                v
            });
            Projects::<T>::insert(
                id,
                Project {
                    id,
                    proposer: proposer.clone(),
                    hash,
                    target,
                    self_fund,
                    community: BalanceOf::<T>::zero(),
                    matched: BalanceOf::<T>::zero(),
                    funded: false,
                },
            );
            Self::deposit_event(Event::ProjectOpened { id, target });
            Ok(())
        }

        /// Contribute to a project. Triggers protocol match when target is reached.
        #[pallet::call_index(9)]
        #[pallet::weight(15_000)]
        pub fn contribute(origin: OriginFor<T>, id: u32, amount: BalanceOf<T>) -> DispatchResult {
            let by = ensure_signed(origin)?;
            ensure!(T::Personhood::is_verified(&by), Error::<T>::NotVerified);
            let mut proj = Projects::<T>::get(id).ok_or(Error::<T>::NotFound)?;
            ensure!(!proj.funded, Error::<T>::AlreadyFunded);

            drop(T::Currency::withdraw(
                &by,
                amount,
                WithdrawReasons::TRANSFER,
                ExistenceRequirement::KeepAlive,
            )?);

            proj.community = proj.community.saturating_add(amount);
            Self::deposit_event(Event::Contributed { id, by, amount });

            if proj.self_fund.saturating_add(proj.community) >= proj.target {
                let m = MatchMultiplier::<T>::get();
                let mat = (proj.community.saturating_mul(m.into())).min(proj.target);
                proj.matched = mat;
                proj.funded = true;
                Self::deposit_event(Event::Matched { id, amount: mat });
            }
            Projects::<T>::insert(id, proj);
            Ok(())
        }

        /// Committee sets the protocol-match multiplier (1 – MAX_MULTIPLIER).
        #[pallet::call_index(10)]
        #[pallet::weight(3_000)]
        pub fn set_multiplier(origin: OriginFor<T>, m: u32) -> DispatchResult {
            T::CommitteeOrigin::ensure_origin(origin)?;
            ensure!(m <= MAX_MULTIPLIER, DispatchError::Other("exceeds max"));
            MatchMultiplier::<T>::put(m);
            Ok(())
        }
    }

    // ── Internals ─────────────────────────────────────────────────────────

    impl<T: Config> Pallet<T> {
        /// All three governance unlock conditions.
        pub fn is_unlocked() -> bool {
            let now = frame_system::Pallet::<T>::block_number();
            now >= MIN_CHAIN_AGE.into()
                && T::Personhood::verified_count() >= MIN_VERIFIED
                && T::Personhood::audit_certified()
        }

        /// Effective vote weight = GovernanceScore × AgeFactor.
        pub fn weight_of(who: &T::AccountId) -> u64 {
            let s = Score::<T>::get(who);
            if s == 0 {
                return 0;
            }
            let now = frame_system::Pallet::<T>::block_number();
            let age: u32 = VerifiedAt::<T>::get(who)
                .map(|b| now.saturating_sub(b).try_into().unwrap_or(ONE_YEAR))
                .unwrap_or(0);
            let bps = (age.min(ONE_YEAR) as u64) * 100 / ONE_YEAR as u64;
            s.saturating_mul(bps) / 100
        }

        fn recalc(who: &T::AccountId) {
            let (b, t, r, d) = Components::<T>::get(who);
            let s = (T::WBalance::get() as u64).saturating_mul(Self::log2(b + 1))
                + (T::WCirculation::get() as u64).saturating_mul(t)
                + (T::WReferral::get() as u64).saturating_mul(r)
                + (T::WDonation::get() as u64).saturating_mul(d);
            let score = s / 100;
            Score::<T>::insert(who, score);
            Self::deposit_event(Event::ScoreUpdated {
                who: who.clone(),
                score,
            });
        }

        pub fn log2(n: u64) -> u64 {
            if n <= 1 {
                0
            } else {
                63 - n.leading_zeros() as u64
            }
        }

        fn timelock(level: &Level) -> BlockNumberFor<T> {
            match level {
                Level::Parameter => TIMELOCK_L1.into(),
                Level::Feature => TIMELOCK_L2.into(),
                Level::CoreMechanism => TIMELOCK_L3.into(),
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::pallet;
    use super::pallet::{Error, Level, Status};
    use crate as pallet_civ_governance;
    use frame_support::{assert_noop, assert_ok, traits::ConstU32};
    use pallet_civ_identity::PersonhoodProvider;
    use sp_core::H256;

    use sp_runtime::{
        traits::{BlakeTwo256, IdentityLookup},
        BuildStorage,
    };

    type Block = frame_system::mocking::MockBlock<Test>;
    frame_support::construct_runtime!(
        pub enum Test {
            System:   frame_system,
            Balances: pallet_balances,
            CivGov:   pallet_civ_governance,
        }
    );

    // Mock: accounts 1-20 are verified; audit certified.
    pub struct MockId;
    impl PersonhoodProvider<u64> for MockId {
        fn is_verified(who: &u64) -> bool {
            *who <= 20
        }
        fn verified_count() -> u64 {
            10_001
        }
        fn audit_certified() -> bool {
            true
        }
    }

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
        type AccountData = pallet_balances::AccountData<u128>;
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
    frame_support::parameter_types! { pub const ED: u128 = 1; }
    impl pallet_balances::Config for Test {
        type MaxLocks = ();
        type MaxReserves = ();
        type ReserveIdentifier = [u8; 8];
        type Balance = u128;
        type DustRemoval = ();
        type RuntimeEvent = RuntimeEvent;
        type ExistentialDeposit = ED;
        type AccountStore = System;
        type WeightInfo = ();
        type FreezeIdentifier = ();
        type MaxFreezes = ();
        type RuntimeHoldReason = ();
        type RuntimeFreezeReason = ();
        type DoneSlashHandler = ();
    }
    frame_support::parameter_types! {
        pub const WB: u32 = 25;
        pub const WC: u32 = 25;
        pub const WR: u32 = 25;
        pub const WD: u32 = 25;
    }
    impl pallet::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type Currency = Balances;
        type Personhood = MockId;
        type CommitteeOrigin = frame_system::EnsureRoot<u64>;
        type WBalance = WB;
        type WCirculation = WC;
        type WReferral = WR;
        type WDonation = WD;
    }

    fn ext() -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap();
        pallet::GenesisConfig::<Test> {
            match_multiplier: Some(2),
            _ph: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();
        let mut e = sp_io::TestExternalities::new(t);
        e.execute_with(|| System::set_block_number(1));
        e
    }

    // Advance chain to unlock conditions and give accounts age + score.
    fn unlock_and_age(ids: &[u64]) {
        // Satisfy chain-age condition
        System::set_block_number(2_600_000);
        for &id in ids {
            assert_ok!(CivGov::record_age(RuntimeOrigin::root(), id));
        }
        // Give full age: advance 1 more year
        System::set_block_number(2_600_000 + 5_256_001);
    }
    fn give_score(id: u64, s: u64) {
        pallet::Score::<Test>::insert(id, s);
    }

    // ── Unlock conditions ──────────────────────────────────────────────
    #[test]
    fn locked_initially() {
        ext().execute_with(|| assert!(!CivGov::is_unlocked()));
    }
    #[test]
    fn unlocks_when_conditions_met() {
        ext().execute_with(|| {
            System::set_block_number(2_600_000);
            // MockId: count=10_001, audit=true → only age was missing
            assert!(CivGov::is_unlocked());
        });
    }
    #[test]
    fn propose_fails_when_locked() {
        ext().execute_with(|| {
            give_score(1, 200);
            assert_noop!(
                CivGov::propose(RuntimeOrigin::signed(1), Level::Parameter, [0u8; 32]),
                Error::<Test>::Locked
            );
        });
    }

    // ── Age weight ─────────────────────────────────────────────────────
    #[test]
    fn no_age_record_zero_weight() {
        ext().execute_with(|| {
            give_score(1, 1000);
            assert_eq!(CivGov::weight_of(&1u64), 0);
        });
    }
    #[test]
    fn full_year_full_weight() {
        ext().execute_with(|| {
            System::set_block_number(1);
            assert_ok!(CivGov::record_age(RuntimeOrigin::root(), 1));
            give_score(1, 1000);
            System::set_block_number(5_256_002);
            assert_eq!(CivGov::weight_of(&1u64), 1000);
        });
    }
    #[test]
    fn half_year_half_weight() {
        ext().execute_with(|| {
            System::set_block_number(1);
            assert_ok!(CivGov::record_age(RuntimeOrigin::root(), 1));
            give_score(1, 1000);
            System::set_block_number(2_628_001);
            let w = CivGov::weight_of(&1u64);
            assert!((490..=510).contains(&w), "expected ~500, got {}", w);
        });
    }

    // ── Proposal lifecycle ─────────────────────────────────────────────
    #[test]
    fn full_lifecycle_passes() {
        ext().execute_with(|| {
            unlock_and_age(&[1, 2, 3]);
            give_score(1, 300);
            give_score(2, 200);
            give_score(3, 100);

            assert_ok!(CivGov::propose(
                RuntimeOrigin::signed(1),
                Level::Parameter,
                [0u8; 32]
            ));
            assert_ok!(CivGov::vote(RuntimeOrigin::signed(1), 0, true));
            assert_ok!(CivGov::vote(RuntimeOrigin::signed(2), 0, true));
            assert_ok!(CivGov::vote(RuntimeOrigin::signed(3), 0, false));
            assert_noop!(
                CivGov::vote(RuntimeOrigin::signed(1), 0, false),
                Error::<Test>::AlreadyVoted
            );

            let now = frame_system::Pallet::<Test>::block_number();
            System::set_block_number(now + 100_802);
            assert_ok!(CivGov::finalise(RuntimeOrigin::signed(99), 0));
            assert_eq!(CivGov::proposals(0).unwrap().status, Status::Passed);
        });
    }
    #[test]
    fn proposal_rejected_below_quorum() {
        ext().execute_with(|| {
            unlock_and_age(&[1, 2]);
            give_score(1, 100);
            give_score(2, 300);
            assert_ok!(CivGov::propose(
                RuntimeOrigin::signed(1),
                Level::Parameter,
                [1u8; 32]
            ));
            assert_ok!(CivGov::vote(RuntimeOrigin::signed(1), 0, true));
            assert_ok!(CivGov::vote(RuntimeOrigin::signed(2), 0, false));
            let now = frame_system::Pallet::<Test>::block_number();
            System::set_block_number(now + 100_802);
            assert_ok!(CivGov::finalise(RuntimeOrigin::signed(99), 0));
            assert_eq!(CivGov::proposals(0).unwrap().status, Status::Rejected);
        });
    }

    // ── Funding ────────────────────────────────────────────────────────
    #[test]
    fn project_match_triggered_at_target() {
        ext().execute_with(|| {
            Balances::force_set_balance(RuntimeOrigin::root(), 1, 50_000).ok();
            Balances::force_set_balance(RuntimeOrigin::root(), 2, 50_000).ok();
            assert_ok!(CivGov::open_project(
                RuntimeOrigin::signed(1),
                [0u8; 32],
                500,
                100
            ));
            assert_ok!(CivGov::contribute(RuntimeOrigin::signed(2), 0, 400));
            let p = CivGov::projects(0).unwrap();
            assert!(p.funded);
            // matched = min(400 × 2, 500) = 500
            assert_eq!(p.matched, 500u128);
        });
    }
    #[test]
    fn double_fund_rejected() {
        ext().execute_with(|| {
            Balances::force_set_balance(RuntimeOrigin::root(), 1, 50_000).ok();
            Balances::force_set_balance(RuntimeOrigin::root(), 2, 50_000).ok();
            assert_ok!(CivGov::open_project(
                RuntimeOrigin::signed(1),
                [0u8; 32],
                500,
                100
            ));
            assert_ok!(CivGov::contribute(RuntimeOrigin::signed(2), 0, 400));
            assert_noop!(
                CivGov::contribute(RuntimeOrigin::signed(2), 0, 1),
                Error::<Test>::AlreadyFunded
            );
        });
    }
}
