#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

use frame_support::pallet_prelude::{BoundedVec, ConstU32};
use frame_system::pallet_prelude::BlockNumberFor;
use scale_info::prelude::{vec, vec::Vec};
use sp_core::crypto::KeyTypeId;

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"orac");

mod price_providers;
use price_providers::{CryptoCompareProvider, PriceProvider};

pub const SCALING_FACTOR: f64 = 10000.0;

pub mod crypto {
    use crate::KEY_TYPE;
    use sp_core::sr25519::Signature as Sr25519Signature;
    use sp_runtime::{
        app_crypto::{app_crypto, sr25519},
        traits::Verify,
        MultiSignature, MultiSigner,
    };
    app_crypto!(sr25519, KEY_TYPE);

    pub struct OracleAuthId;

    impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for OracleAuthId {
        type RuntimeAppPublic = Public;
        type GenericSignature = sp_core::sr25519::Signature;
        type GenericPublic = sp_core::sr25519::Public;
    }

    impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature>
        for OracleAuthId
    {
        type RuntimeAppPublic = Public;
        type GenericSignature = sp_core::sr25519::Signature;
        type GenericPublic = sp_core::sr25519::Public;
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
    use frame_support::{pallet_prelude::*, traits::BuildGenesisConfig};
    use frame_system::{
        offchain::{AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer},
        pallet_prelude::*,
    };
    use scale_info::{prelude::fmt, TypeInfo};
    use sp_runtime::{offchain::http, sp_std::str};

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config + CreateSignedTransaction<Call<Self>> + fmt::Debug
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type AuthorityId: AppCrypto<Self::Public, Self::Signature>;
    }

    /// Oracle configuration
    #[pallet::storage]
    pub type MinNodesForTrustedAggregation<T> = StorageValue<_, u32>;

    #[pallet::storage]
    pub type FeedAge<T> = StorageValue<_, u16>;

    #[pallet::storage]
    pub type OutliersRange<T> = StorageValue<_, u32>;

    #[pallet::storage]
    pub type DivergencePercentage<T> = StorageValue<_, u32>;

    /// NodesPrices store latest price for each node
    /// about Identity hasher https://docs.substrate.io/build/runtime-storage/#common-substrate-hashers
    #[pallet::storage]
    pub type NodesPrices<T: Config> = StorageMap<
        Hasher = Identity,
        Key = T::AccountId,
        Value = (u32, BlockNumberFor<T>),
        QueryKind = OptionQuery,
    >;

    /// price after nodes "consensus"
    /// The first value is the median price
    /// The second value is the age of the median price
    #[pallet::storage]
    pub type Price<T> = StorageValue<_, (u32, u16)>;

    /// oracle genesis config definition and associated macros
    // see https://docs.substrate.io/reference/how-to-guides/basics/configure-genesis-state/
    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub min_nodes_for_trusted_aggregation: u32,
        pub feed_age: u16,
        pub outliers_range: u32,
        pub divergence_percentage: u32,
        // Ties `T` to `GenesisConfig` because is needed for `impl<T: Config> BuildGenesisConfig ...`
        pub _marker: PhantomData<T>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                min_nodes_for_trusted_aggregation: Default::default(),
                feed_age: Default::default(),
                outliers_range: Default::default(),
                divergence_percentage: Default::default(),
                _marker: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            <MinNodesForTrustedAggregation<T>>::put(&self.min_nodes_for_trusted_aggregation);
            <FeedAge<T>>::put(&self.feed_age);
            <OutliersRange<T>>::put(&self.outliers_range);
            <DivergencePercentage<T>>::put(&self.divergence_percentage);
        }
    }

    // Information about whether the aggregation happened or not
    #[derive(Clone, PartialEq, Encode, Decode, TypeInfo, Debug)]
    pub enum AggregationStatus {
        AggregationPerformed {
            non_outliers: u16,
            non_outlier_prices: Vec<u32>,
            outliers: u16,
            outlier_prices: Vec<u32>,
        },
        AggregationNotPerformed,
    }

    // Aggregation status flag
    #[derive(Clone, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, Debug)]
    pub enum Flag {
        Ok,
        NotEnoughNodes,
        NoPreviousMedian,
    }

    /// pallet events
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        StoredPrice {
            price: u32,
            who: T::AccountId,
            when: BlockNumberFor<T>,
        },
        Status {
            median_price: u32,
            flag: Flag,
            participating_nodes: u32,
            age: u16,
            block: BlockNumberFor<T>,
            status: AggregationStatus,
        },
    }

    /// pallet calls
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight((0, Pays::No))]
        pub fn store_price(origin: OriginFor<T>, price: u32) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let when = <frame_system::Pallet<T>>::block_number();
            NodesPrices::<T>::insert(&who, (price, when));
            Self::deposit_event(Event::StoredPrice {
                price,
                who: who.clone(),
                when,
            });
            Ok(())
        }
    }

    /// pallet auxiliary methods
    impl<T: Config> Pallet<T> {
        pub fn fetch_price() -> Result<u32, http::Error> {
            CryptoCompareProvider::fetch_price()
        }
    }

    /// pallet hooks
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        // Offchain worker that triggers the extrinsic submitting a price to the
        // NodePrices storage
        fn offchain_worker(_n: BlockNumberFor<T>) {
            log::info!("Starting offchain worker to query price");
            let mut acc_list = Signer::<T, T::AuthorityId>::keystore_accounts();
            match acc_list.next() {
                Some(signer_account) if acc_list.next().is_none() => {
                    let signer = Signer::<T, T::AuthorityId>::all_accounts()
                        .with_filter(vec![signer_account.clone().public]);
                    if signer.can_sign() {
                        match Self::fetch_price() {
                            Ok(price) => {
                                let result = signer.send_single_signed_transaction(
                                    &signer_account,
                                    Call::store_price { price },
                                );
                                if result.is_some_and(|res| res.is_ok()) {
                                    log::info!(
                                        "[{:?}]: submit transaction success.",
                                        signer_account.id
                                    )
                                } else {
                                    log::error!(
                                        "[{:?}]: submit transaction failure.",
                                        signer_account.id
                                    )
                                }
                            }
                            Err(e) => {
                                log::error!(
                                    "[{:?}]: failed to fetch price: {:?}",
                                    signer_account.id,
                                    e
                                );
                            }
                        }
                    }
                }
                Some(_accounts) => log::error!("More than one account. Expected only one"),
                None => log::error!("No account available for oracle"),
            }
        }

        fn on_finalize(n: BlockNumberFor<T>) {
            log::info!("Aggregating median price for block {:?}", n);
            if let Some((
                min_nodes_for_trusted_aggregation,
                feed_age,
                outliers_range,
                divergence_percentage,
            )) = Self::get_oracle_config()
            {
                let mut participating_nodes: u32 = 0;
                let prices = NodesPrices::<T>::iter_values()
                    .by_ref()
                    .filter_map(|(p, a)| {
                        if (n - a) <= feed_age.into() {
                            participating_nodes += 1;
                            Some(p)
                        } else {
                            None
                        }
                    })
                    .collect();
                let (median_price, age, flag, status): (u32, u16, Flag, crate::AggregationStatus) =
                    if min_nodes_for_trusted_aggregation <= participating_nodes {
                        log::info!(
                            "{:?} nodes submitted a price. Aggregating median price ...",
                            participating_nodes
                        );
                        Self::aggregate(prices, outliers_range, divergence_percentage)
                    } else {
                        log::error!("Not enough nodes for trusted aggregation. Reusing median ...");
                        Self::reuse_previous_median()
                    };
                Price::<T>::put((median_price, age));
                log::info!(
                    "Median price for block {:?} is {:?} with status: {:?}",
                    n,
                    median_price,
                    flag
                );
                Self::deposit_event(Event::Status {
                    median_price,
                    flag,
                    participating_nodes,
                    age,
                    block: n,
                    status,
                })
            } else {
                log::error!("Couldn't fetch Oracle Config");
            }
        }
    }
}

impl<T: Config> Pallet<T> {
    fn aggregate(
        prices: Vec<u32>,
        outliers_range: u32,
        divergence_percentage: u32,
    ) -> (u32, u16, Flag, crate::AggregationStatus) {
        let mut prices = BoundedVec::<u32, ConstU32<32>>::truncate_from(prices);
        prices.sort();
        let sorted_prices = prices.to_vec();
        let length: usize = prices.len();
        let median = Self::calculate_median(sorted_prices.clone(), length);
        let (non_outlier_prices, outlier_prices) = Self::filter_outliers(
            sorted_prices,
            median,
            length,
            outliers_range,
            divergence_percentage,
        );
        (
            median,
            0,
            Flag::Ok,
            AggregationStatus::AggregationPerformed {
                non_outliers: non_outlier_prices.len() as u16,
                non_outlier_prices,
                outliers: outlier_prices.len() as u16,
                outlier_prices,
            },
        )
    }

    fn reuse_previous_median() -> (u32, u16, Flag, crate::AggregationStatus) {
        if let Some((median, age)) = Price::<T>::get() {
            (
                median,
                age + 1,
                Flag::NotEnoughNodes,
                AggregationStatus::AggregationNotPerformed,
            )
        } else {
            log::error!("Error: no median to reuse.");
            (
                0,
                0,
                Flag::NoPreviousMedian,
                AggregationStatus::AggregationNotPerformed,
            )
        }
    }

    fn calculate_median(prices: Vec<u32>, length: usize) -> u32 {
        if length > 0 {
            if length % 2 == 0 {
                prices[(length - 1) / 2]
            } else {
                (prices[(length - 1) / 2] + prices[length / 2]) / 2
            }
        } else {
            log::error!("Error: length of vec is 0. Vec: {:?}", prices);
            0
        }
    }

    fn get_oracle_config() -> Option<(u32, u16, u32, u32)> {
        if let Some(min_nodes) = MinNodesForTrustedAggregation::<T>::get() {
            if let Some(feed_age) = FeedAge::<T>::get() {
                if let Some(outliers_range) = OutliersRange::<T>::get() {
                    if let Some(divergence_percentage) = DivergencePercentage::<T>::get() {
                        Some((min_nodes, feed_age, outliers_range, divergence_percentage))
                    } else {
                        log::error!("Error fetching DivergencePercentage");
                        None
                    }
                } else {
                    log::error!("Error fetching OutliersRange");
                    None
                }
            } else {
                log::error!("Error fetching FeedAge");
                None
            }
        } else {
            log::error!("Error fetching MinNodesForTrustedAggregation");
            None
        }
    }

    fn filter_outliers(
        prices: Vec<u32>,
        median: u32,
        length: usize,
        outliers_range: u32,
        divergence: u32,
    ) -> (Vec<u32>, Vec<u32>) {
        let first_quartile = Self::calculate_median(
            prices.clone().into_iter().take(length / 2).collect(),
            length / 2,
        );
        let third_quartile = Self::calculate_median(
            prices.clone().into_iter().skip(length / 2).collect(),
            length / 2,
        );

        let interquartile_range = third_quartile - first_quartile;

        let lower_bound = first_quartile - (outliers_range * interquartile_range);
        let upper_bound = third_quartile + (outliers_range * interquartile_range);

        prices.into_iter().partition(|x| {
            (lower_bound <= *x && *x <= upper_bound)
                && Self::within_divergence(*x, median, divergence)
        })
    }

    fn within_divergence(x: u32, median: u32, divergence: u32) -> bool {
        let dif = Self::unsigned_sub(x, median);
        let fraction = (f64::from(dif) * SCALING_FACTOR) / f64::from(median);
        fraction <= divergence.into()
    }

    // substraction between two u32 can cause overflow
    fn unsigned_sub(x: u32, y: u32) -> u32 {
        if x <= y {
            y - x
        } else {
            x - y
        }
    }
}
