#![allow(deprecated)]
#![allow(clippy::let_unit_value)]
#![allow(clippy::type_complexity)]

//! # pallet-civ-monetary
//!
//! Adaptive UBI issuance and fee-recycling engine.
//!
//! ## Core formula
//!
//! ```text
//! Daily_UBI = FCI / EMA(price) × ActivityFactor
//!
//! FCI           = Food Cost Index (governance-set, ~$4/day)
//! EMA(price)    = 0.9 × old + 0.1 × new  (oracle-fed)
//! ActivityFactor = (50 + min(score,100)) / 100  ∈ [0.50, 1.50]
//! ```
//!
//! ## Three phases
//!
//! | Phase       | Condition                          | UBI source       |
//! |-------------|-----------------------------------|------------------|
//! | Bootstrap   | Minted < 80% target               | Mint             |
//! | Transition  | 80% ≤ Minted < target              | Mint + FeePool   |
//! | Equilibrium | Minted ≥ target **AND** FeePool ≥ N days reserve | FeePool only |
//!
//! Equilibrium requires a FeePool reserve of at least `EquilibriumReserveDays`
//! of daily demand. Without this gate the protocol transitions to Equilibrium
//! before the pool can sustain UBI, causing immediate FeePoolEmpty failures.
//!
//! ## Safety mechanisms
//!
//! - **Oracle rate limit**: single price update ≤ 20% change from current EMA
//! - **Hard daily cap**: per-person per-day mint ≤ `MaxDailyMintPerPerson`
//! - **Auto-stabiliser**: weekly ±0.2% adjustment of α/β/γ based on pool health
//!
//! ## Known limitations (see TOKENOMICS.md)
//!
//! - Reflexive price-issuance loop not proven stable at scale
//! - Oracle is admin-only in V0.1 (single point of failure)
//! - Median estimate is approximate
//! - Monte Carlo simulation not yet complete

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
    use sp_runtime::{
        traits::{CheckedAdd, Saturating, Zero},
        Perbill,
    };

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // ── Protocol constants ────────────────────────────────────────────────

    /// ~24 h at 6 s/block.
    const CLAIM_INTERVAL: u32 = 14_400;
    /// ~365 days — inactivity / wealth decay cycle.
    const DECAY_INTERVAL: u32 = 5_256_000;
    /// ~7 days — auto-stabiliser run.
    const STABILISE_INTERVAL: u32 = 100_800;
    /// Price stored as integer × 10^8.  $1.00 = 100_000_000.
    pub const PRICE_PRECISION: u64 = 100_000_000;
    /// Max single oracle update: ±20% of current EMA.
    const MAX_PRICE_CHANGE_PCT: u64 = 20;
    /// β and γ hard ceiling regardless of stabiliser.
    const MAX_DECAY_RATE: Perbill = Perbill::from_percent(5);

    // ── Types ─────────────────────────────────────────────────────────────

    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum Phase {
        Bootstrap,
        Transition,
        Equilibrium,
    }

    // ── Config ────────────────────────────────────────────────────────────

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: Currency<Self::AccountId>;
        type Personhood: PersonhoodProvider<Self::AccountId>;
        /// Admin / oracle origin (V0.1: EnsureRoot; V0.2: multi-source feed).
        type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// FCI in PRICE_PRECISION units. e.g. 400_000_000 = $4.00.
        #[pallet::constant]
        type FoodCostIndex: Get<u64>;
        /// Total target supply (governance proxy for GPI in V0.1).
        #[pallet::constant]
        type InitialTargetSupply: Get<BalanceOf<Self>>;
        /// FeePool must cover this many days of demand before Equilibrium.
        #[pallet::constant]
        type EquilibriumReserveDays: Get<u32>;
        /// Hard cap: max UBI coins minted per person per day.
        #[pallet::constant]
        type MaxDailyMintPerPerson: Get<u32>;
        /// Max accounts per apply_decay call.
        #[pallet::constant]
        type MaxDecayBatch: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ── Storage ───────────────────────────────────────────────────────────

    #[pallet::storage]
    #[pallet::getter(fn total_minted)]
    pub type TotalMinted<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn fee_pool)]
    pub type FeePool<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn target_supply)]
    pub type TargetSupply<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// EMA of market price in PRICE_PRECISION units. Default $1.00.
    #[pallet::storage]
    #[pallet::getter(fn ema_price)]
    pub type EmaPrice<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn last_claim)]
    pub type LastClaim<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BlockNumberFor<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn last_activity)]
    pub type LastActivity<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BlockNumberFor<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn activity_score)]
    pub type ActivityScore<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn tx_fee_rate)]
    pub type TxFeeRate<T: Config> = StorageValue<_, Perbill, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn inactivity_rate)]
    pub type InactivityRate<T: Config> = StorageValue<_, Perbill, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn wealth_decay_rate)]
    pub type WealthDecayRate<T: Config> = StorageValue<_, Perbill, ValueQuery>;

    // ── Genesis ───────────────────────────────────────────────────────────

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        pub initial_price: Option<u64>,
        pub tx_fee_rate: Option<Perbill>,
        pub inactivity_rate: Option<Perbill>,
        pub wealth_decay_rate: Option<Perbill>,
        #[serde(skip)]
        pub _ph: core::marker::PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            TargetSupply::<T>::put(T::InitialTargetSupply::get());
            EmaPrice::<T>::put(self.initial_price.unwrap_or(PRICE_PRECISION));
            TxFeeRate::<T>::put(self.tx_fee_rate.unwrap_or(Perbill::from_parts(3_000)));
            InactivityRate::<T>::put(self.inactivity_rate.unwrap_or(Perbill::from_parts(10_000)));
            WealthDecayRate::<T>::put(self.wealth_decay_rate.unwrap_or(Perbill::from_parts(8_000)));
        }
    }

    // ── Hooks ─────────────────────────────────────────────────────────────

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(n: BlockNumberFor<T>) -> Weight {
            if (n % STABILISE_INTERVAL.into()).is_zero() {
                Self::auto_stabilise();
            }
            Weight::zero()
        }
    }

    // ── Events ────────────────────────────────────────────────────────────

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        UbiClaimed {
            who: T::AccountId,
            amount: BalanceOf<T>,
            phase: Phase,
        },
        Transferred {
            from: T::AccountId,
            to: T::AccountId,
            amount: BalanceOf<T>,
            fee: BalanceOf<T>,
        },
        DecayApplied {
            who: T::AccountId,
            amount: BalanceOf<T>,
        },
        PriceUpdated {
            raw: u64,
            ema: u64,
        },
        RatesAdjusted {
            alpha: Perbill,
            beta: Perbill,
            gamma: Perbill,
        },
        TargetUpdated {
            new_target: BalanceOf<T>,
        },
    }

    // ── Errors ────────────────────────────────────────────────────────────

    #[pallet::error]
    pub enum Error<T> {
        NotVerified,
        CooldownActive,
        InsufficientBalance,
        FeePoolEmpty,
        Overflow,
        InvalidPrice,
        PriceChangeTooLarge,
        RateChangeTooLarge,
    }

    // ── Calls ─────────────────────────────────────────────────────────────

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Claim daily UBI. Amount = FCI / EMA(price) × ActivityFactor.
        #[pallet::call_index(0)]
        #[pallet::weight(50_000)]
        pub fn claim_ubi(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(T::Personhood::is_verified(&who), Error::<T>::NotVerified);

            let now = frame_system::Pallet::<T>::block_number();
            if let Some(last) = LastClaim::<T>::get(&who) {
                ensure!(
                    now >= last + CLAIM_INTERVAL.into(),
                    Error::<T>::CooldownActive
                );
            }

            let amount = Self::ubi_amount(&who);
            let phase = Self::current_phase();
            Self::issue_ubi(&who, amount, &phase)?;

            LastClaim::<T>::insert(&who, now);
            LastActivity::<T>::insert(&who, now);
            Self::deposit_event(Event::UbiClaimed { who, amount, phase });
            Ok(())
        }

        /// Transfer CIV. α fee → FeePool. Increments ActivityScore.
        #[pallet::call_index(1)]
        #[pallet::weight(30_000)]
        pub fn transfer(
            origin: OriginFor<T>,
            to: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;
            let fee = TxFeeRate::<T>::get() * amount;
            let total = amount.checked_add(&fee).ok_or(Error::<T>::Overflow)?;

            drop(T::Currency::withdraw(
                &from,
                total,
                WithdrawReasons::TRANSFER,
                ExistenceRequirement::KeepAlive,
            )?);

            drop(T::Currency::deposit_creating(&to, amount));
            FeePool::<T>::mutate(|p| *p = p.saturating_add(fee));
            ActivityScore::<T>::mutate(&from, |s| *s = (*s + 1).min(100));
            LastActivity::<T>::insert(&from, frame_system::Pallet::<T>::block_number());

            Self::deposit_event(Event::Transferred {
                from,
                to,
                amount,
                fee,
            });
            Ok(())
        }

        /// Apply inactivity and wealth decay to a batch of accounts.
        /// Should be called approximately once per DECAY_INTERVAL blocks.
        #[pallet::call_index(2)]
        #[pallet::weight(30_000u64.saturating_mul(accounts.len() as u64))]
        pub fn apply_decay(
            origin: OriginFor<T>,
            accounts: BoundedVec<T::AccountId, T::MaxDecayBatch>,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            let now = frame_system::Pallet::<T>::block_number();
            let median = Self::median_estimate();
            let beta = InactivityRate::<T>::get();
            let gamma = WealthDecayRate::<T>::get();

            for who in accounts.into_iter() {
                let bal = T::Currency::free_balance(&who);
                let mut decay = BalanceOf::<T>::zero();

                // Inactivity decay
                let idle = LastActivity::<T>::get(&who)
                    .map(|l| now.saturating_sub(l))
                    .unwrap_or(now);
                if idle >= DECAY_INTERVAL.into() {
                    decay = decay.saturating_add(beta * bal);
                }
                // Wealth decay — only above-median portion
                if bal > median {
                    decay = decay.saturating_add(gamma * bal.saturating_sub(median));
                }

                if !decay.is_zero() {
                    let actual = decay.min(bal);
                    let _ = T::Currency::withdraw(
                        &who,
                        actual,
                        WithdrawReasons::FEE,
                        ExistenceRequirement::AllowDeath,
                    );
                    FeePool::<T>::mutate(|p| *p = p.saturating_add(actual));
                    Self::deposit_event(Event::DecayApplied {
                        who: who.clone(),
                        amount: actual,
                    });
                }
                ActivityScore::<T>::mutate(&who, |s| *s = s.saturating_sub(10));
            }
            Ok(())
        }

        /// Submit a price observation from the oracle.
        ///
        /// Safety: single update is rate-limited to ±MAX_PRICE_CHANGE_PCT%.
        /// This prevents an oracle compromise from crashing the system in one call.
        ///
        /// V0.1: admin-only.  V0.2: multi-source Chainlink aggregator.
        #[pallet::call_index(3)]
        #[pallet::weight(10_000)]
        pub fn update_price(origin: OriginFor<T>, raw: u64) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            ensure!(raw > 0, Error::<T>::InvalidPrice);

            let old = EmaPrice::<T>::get();
            // Rate limit: raw must be within ±20% of current EMA
            let max_delta = old.saturating_mul(MAX_PRICE_CHANGE_PCT) / 100;
            ensure!(
                raw >= old.saturating_sub(max_delta) && raw <= old.saturating_add(max_delta),
                Error::<T>::PriceChangeTooLarge
            );

            let new_ema = (old / 10).saturating_mul(9).saturating_add(raw / 10);
            EmaPrice::<T>::put(new_ema);
            Self::deposit_event(Event::PriceUpdated { raw, ema: new_ema });
            Ok(())
        }

        /// Governance: adjust decay rates. Max ±0.2% per call.
        #[pallet::call_index(4)]
        #[pallet::weight(5_000)]
        pub fn set_rates(
            origin: OriginFor<T>,
            alpha: Perbill,
            beta: Perbill,
            gamma: Perbill,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            let step = Perbill::from_parts(2_000); // 0.20%
            ensure!(
                Self::pdelta(TxFeeRate::<T>::get(), alpha) <= step
                    && Self::pdelta(InactivityRate::<T>::get(), beta) <= step
                    && Self::pdelta(WealthDecayRate::<T>::get(), gamma) <= step,
                Error::<T>::RateChangeTooLarge
            );
            TxFeeRate::<T>::put(alpha);
            InactivityRate::<T>::put(beta);
            WealthDecayRate::<T>::put(gamma);
            Self::deposit_event(Event::RatesAdjusted { alpha, beta, gamma });
            Ok(())
        }

        /// Governance: update target supply (GPI proxy until oracle is live).
        #[pallet::call_index(5)]
        #[pallet::weight(5_000)]
        pub fn set_target_supply(origin: OriginFor<T>, new_target: BalanceOf<T>) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            TargetSupply::<T>::put(new_target);
            Self::deposit_event(Event::TargetUpdated { new_target });
            Ok(())
        }
    }

    // ── Internals ─────────────────────────────────────────────────────────

    impl<T: Config> Pallet<T> {
        /// Current monetary phase.
        ///
        /// Equilibrium requires BOTH supply ≥ target AND FeePool ≥ reserve.
        /// Without the reserve check, Equilibrium starts with an empty pool
        /// and UBI immediately fails.
        pub fn current_phase() -> Phase {
            let minted = TotalMinted::<T>::get();
            let target = TargetSupply::<T>::get();
            let t80 = Perbill::from_percent(80) * target;

            if minted < t80 {
                return Phase::Bootstrap;
            }
            if minted < target {
                return Phase::Transition;
            }

            // Supply threshold met — check pool adequacy
            let count = T::Personhood::verified_count();
            let per_day = Self::per_person_base_ubi();
            let demand = per_day.saturating_mul(BalanceOf::<T>::from(count as u32));
            let required = demand.saturating_mul(T::EquilibriumReserveDays::get().into());

            if FeePool::<T>::get() >= required {
                Phase::Equilibrium
            } else {
                Phase::Transition // wait for pool to fill
            }
        }

        fn ubi_amount(who: &T::AccountId) -> BalanceOf<T> {
            let base = Self::per_person_base_ubi();
            let score = ActivityScore::<T>::get(who).min(100);
            // ActivityFactor = (50 + score) / 100
            Perbill::from_rational(50u32 + score, 100u32) * base
        }

        /// Base UBI = FCI / EMA(price), clamped to MaxDailyMintPerPerson.
        pub fn per_person_base_ubi() -> BalanceOf<T> {
            let price = EmaPrice::<T>::get().max(1);
            let fci = T::FoodCostIndex::get();
            let raw = (fci as u128)
                .saturating_mul(1_000_000u128)
                .checked_div(price as u128)
                .unwrap_or(0);
            let cap = BalanceOf::<T>::from(T::MaxDailyMintPerPerson::get());
            BalanceOf::<T>::from(raw.min(u32::MAX as u128) as u32).min(cap)
        }

        fn issue_ubi(who: &T::AccountId, amount: BalanceOf<T>, phase: &Phase) -> DispatchResult {
            if amount.is_zero() {
                return Ok(());
            }
            match phase {
                Phase::Bootstrap => {
                    drop(T::Currency::deposit_creating(who, amount));
                    TotalMinted::<T>::mutate(|m| *m = m.saturating_add(amount));
                }
                Phase::Equilibrium => {
                    ensure!(FeePool::<T>::get() >= amount, Error::<T>::FeePoolEmpty);
                    FeePool::<T>::mutate(|p| *p = p.saturating_sub(amount));
                    drop(T::Currency::deposit_creating(who, amount));
                }
                Phase::Transition => {
                    let gap = TargetSupply::<T>::get().saturating_sub(TotalMinted::<T>::get());
                    let from_mint = amount.min(gap);
                    let from_pool = amount.saturating_sub(from_mint);
                    if !from_mint.is_zero() {
                        drop(T::Currency::deposit_creating(who, from_mint));
                        TotalMinted::<T>::mutate(|m| *m = m.saturating_add(from_mint));
                    }
                    if !from_pool.is_zero() {
                        ensure!(FeePool::<T>::get() >= from_pool, Error::<T>::FeePoolEmpty);
                        FeePool::<T>::mutate(|p| *p = p.saturating_sub(from_pool));
                        drop(T::Currency::deposit_creating(who, from_pool));
                    }
                }
            }
            Ok(())
        }

        /// Approximate median = TotalMinted / (2 × VerifiedCount).
        /// Replaced by order-statistics in V0.2.
        fn median_estimate() -> BalanceOf<T> {
            let n = T::Personhood::verified_count();
            if n == 0 {
                return BalanceOf::<T>::zero();
            }
            TotalMinted::<T>::get() / BalanceOf::<T>::from(2u32 * n as u32)
        }

        fn pdelta(a: Perbill, b: Perbill) -> Perbill {
            if a >= b {
                a - b
            } else {
                b - a
            }
        }

        /// Weekly auto-stabiliser: nudge rates ±0.2% based on pool health.
        fn auto_stabilise() {
            let count = T::Personhood::verified_count();
            if count == 0 {
                return;
            }

            let per_day = Self::per_person_base_ubi();
            let weekly = per_day
                .saturating_mul(7u32.into())
                .saturating_mul((count as u32).into());
            let low = Perbill::from_percent(80) * weekly;
            let high = Perbill::from_percent(120) * weekly;
            let pool = FeePool::<T>::get();
            let step = Perbill::from_parts(2_000);

            let (beta, gamma) = if pool < low {
                (
                    (InactivityRate::<T>::get() + step).min(MAX_DECAY_RATE),
                    (WealthDecayRate::<T>::get() + step).min(MAX_DECAY_RATE),
                )
            } else if pool > high {
                (
                    InactivityRate::<T>::get().saturating_sub(step),
                    WealthDecayRate::<T>::get().saturating_sub(step),
                )
            } else {
                return;
            };

            InactivityRate::<T>::put(beta);
            WealthDecayRate::<T>::put(gamma);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::pallet;
    use super::pallet::{Error, Phase, PRICE_PRECISION};
    use crate as pallet_civ_monetary;
    use frame_support::{assert_noop, assert_ok, traits::ConstU32};
    use pallet_civ_identity::PersonhoodProvider;
    use sp_core::H256;
    use sp_runtime::{
        traits::{BlakeTwo256, IdentityLookup},
        BuildStorage, Perbill,
    };

    type Block = frame_system::mocking::MockBlock<Test>;
    frame_support::construct_runtime!(
        pub enum Test { System: frame_system, Balances: pallet_balances, CivMon: pallet_civ_monetary }
    );

    pub struct MockId;
    impl PersonhoodProvider<u64> for MockId {
        fn is_verified(who: &u64) -> bool {
            *who < 100
        }
        fn verified_count() -> u64 {
            10
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
    }
    frame_support::parameter_types! {
        pub const FCI:       u64  = 400_000_000; // $4.00
        pub const InitSupply: u128 = 1_000_000_000_000;
    }
    impl pallet::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type Currency = Balances;
        type Personhood = MockId;
        type AdminOrigin = frame_system::EnsureRoot<u64>;
        type FoodCostIndex = FCI;
        type InitialTargetSupply = InitSupply;
        type EquilibriumReserveDays = ConstU32<90>;
        type MaxDailyMintPerPerson = ConstU32<10_000>;
        type MaxDecayBatch = ConstU32<50>;
    }

    fn ext() -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap();
        pallet::GenesisConfig::<Test> {
            initial_price: Some(PRICE_PRECISION),
            tx_fee_rate: Some(Perbill::from_parts(3_000)),
            inactivity_rate: Some(Perbill::from_parts(10_000)),
            wealth_decay_rate: Some(Perbill::from_parts(8_000)),
            _ph: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();
        let mut e = sp_io::TestExternalities::new(t);
        e.execute_with(|| System::set_block_number(1));
        e
    }

    #[test]
    fn starts_bootstrap() {
        ext().execute_with(|| assert_eq!(CivMon::current_phase(), Phase::Bootstrap));
    }
    #[test]
    fn claim_works_first_time() {
        ext().execute_with(|| {
            assert_ok!(CivMon::claim_ubi(RuntimeOrigin::signed(1)));
            assert!(Balances::free_balance(1) > 0);
        });
    }
    #[test]
    fn unverified_cannot_claim() {
        ext().execute_with(|| {
            assert_noop!(
                CivMon::claim_ubi(RuntimeOrigin::signed(200)),
                Error::<Test>::NotVerified
            );
        });
    }
    #[test]
    fn cooldown_enforced() {
        ext().execute_with(|| {
            assert_ok!(CivMon::claim_ubi(RuntimeOrigin::signed(1)));
            assert_noop!(
                CivMon::claim_ubi(RuntimeOrigin::signed(1)),
                Error::<Test>::CooldownActive
            );
        });
    }
    #[test]
    fn second_claim_after_interval() {
        ext().execute_with(|| {
            assert_ok!(CivMon::claim_ubi(RuntimeOrigin::signed(1)));
            System::set_block_number(14_402);
            assert_ok!(CivMon::claim_ubi(RuntimeOrigin::signed(1)));
        });
    }
    #[test]
    fn higher_price_fewer_coins() {
        ext().execute_with(|| {
            // Set EMA high enough that base UBI < MaxDailyMintPerPerson cap (10_000),
            // otherwise the cap dominates and price has no visible effect.
            // base = FCI(400M) * 1M / price.  Need base < 10_000 → price > 40B.
            pallet::EmaPrice::<Test>::put(PRICE_PRECISION * 500); // $500
                                                                  // base = 400M * 1M / 50000M = 8000 — below cap
            assert_ok!(CivMon::claim_ubi(RuntimeOrigin::signed(1)));
            let bal1 = Balances::free_balance(1);

            // Double the price → base halves
            pallet::EmaPrice::<Test>::put(PRICE_PRECISION * 1000); // $1000
            System::set_block_number(14_402);
            assert_ok!(CivMon::claim_ubi(RuntimeOrigin::signed(2)));
            let bal2 = Balances::free_balance(2);

            assert!(
                bal2 < bal1,
                "higher price should yield fewer coins: {} vs {}",
                bal2,
                bal1
            );
        });
    }
    #[test]
    fn oracle_rate_limit() {
        ext().execute_with(|| {
            // Jump to 5× price in one call — should fail
            assert_noop!(
                CivMon::update_price(RuntimeOrigin::root(), PRICE_PRECISION * 5),
                Error::<Test>::PriceChangeTooLarge
            );
        });
    }
    #[test]
    fn transfer_fee_to_pool() {
        ext().execute_with(|| {
            assert_ok!(Balances::force_set_balance(
                RuntimeOrigin::root(),
                1,
                1_000_000_000
            ));
            // TxFeeRate is 0.0003% (Perbill::from_parts(3_000)) so we need
            // a large transfer to produce a non-zero fee after integer truncation.
            let r = CivMon::transfer(RuntimeOrigin::signed(1), 2, 100_000_000);
            assert!(r.is_ok(), "transfer failed: {:?}", r);
            let pool = CivMon::fee_pool();
            assert!(pool > 0, "fee_pool is 0 after transfer");
        });
    }
    #[test]
    fn equilibrium_requires_pool_reserve() {
        ext().execute_with(|| {
            // Force TotalMinted ≥ target but leave FeePool empty
            pallet::TotalMinted::<Test>::put(1_000_000_000_000u128);
            // Phase should stay Transition because pool is empty
            assert_eq!(CivMon::current_phase(), Phase::Transition);
            // Fill pool enough for 90 days
            let per_day = pallet::Pallet::<Test>::per_person_base_ubi();
            let needed = per_day * 10u128 * 90u128; // 10 users × 90 days
            pallet::FeePool::<Test>::put(needed + 1u128);
            assert_eq!(CivMon::current_phase(), Phase::Equilibrium);
        });
    }
    #[test]
    fn rate_change_limit() {
        ext().execute_with(|| {
            assert!(CivMon::set_rates(
                RuntimeOrigin::root(),
                Perbill::from_percent(5),
                Perbill::from_parts(10_000),
                Perbill::from_parts(8_000),
            )
            .is_err());
        });
    }
}
