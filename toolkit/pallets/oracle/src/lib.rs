#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

use sp_core::crypto::KeyTypeId;

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"orac");

pub mod crypto {
    use super::KEY_TYPE;
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
    use frame_support::pallet_prelude::*;
    use frame_support::traits::BuildGenesisConfig;
    use frame_system::{
        offchain::{AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer},
        pallet_prelude::*,
    };
    use scale_info::prelude::vec;
    use sp_runtime::offchain::{http, Duration};
    use sp_runtime::serde::{Deserialize, Serialize};
    use sp_runtime::sp_std::str;
    use sp_runtime::Vec;

    #[derive(Serialize, Deserialize)]
    struct PriceResponse {
        USD: f64,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + CreateSignedTransaction<Call<Self>> {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type AuthorityId: AppCrypto<Self::Public, Self::Signature>;
    }

    /// Oracle configuration
    #[pallet::storage]
    pub type MinNodesForTrustedAggregation<T> = StorageValue<_, u32>;

    #[pallet::storage]
    pub type FeedAge<T: Config> = StorageValue<_, BlockNumberFor<T>>;

    #[pallet::storage]
    pub type OutliersRange<T> = StorageValue<_, u32>;

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
    /// first (naive) consensus version: average of all nodes prices
    #[pallet::storage]
    pub type Price<T> = StorageValue<_, u32>;

    /// oracle genesis config definition and associated macros
    // see https://docs.substrate.io/reference/how-to-guides/basics/configure-genesis-state/
    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub min_nodes_for_trusted_aggregation: u32,
        pub feed_age: BlockNumberFor<T>,
        pub outliers_range: u32,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                min_nodes_for_trusted_aggregation: Default::default(),
                feed_age: Default::default(),
                outliers_range: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            <MinNodesForTrustedAggregation<T>>::put(&self.min_nodes_for_trusted_aggregation);
            <FeedAge<T>>::put(&self.feed_age);
            <OutliersRange<T>>::put(&self.outliers_range);
        }
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

    impl<T: Config> Pallet<T> {
        pub fn fetch_price() -> Result<u32, http::Error> {
            // 2 seconds timeout for not hanging the node
            let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(2_000));

            let request = http::Request::get(
                "https://min-api.cryptocompare.com/data/price?fsym=ADA&tsyms=USD",
            );

            let pending = request
                .deadline(deadline)
                .send()
                .map_err(|_| http::Error::IoError)?;

            let response = pending
                .try_wait(deadline)
                .map_err(|_| http::Error::DeadlineReached)??;

            if response.code != 200 {
                log::warn!("Unexpected status code: {}", response.code);
                return Err(http::Error::Unknown);
            }

            let body = response.body().collect::<Vec<u8>>();
            let body_str = str::from_utf8(&body).map_err(|_| {
                log::warn!("Response was not valid UTF8");
                http::Error::Unknown
            })?;

            log::info!("Got price response: {}", body_str);

            // Parse the JSON response
            let price_data = serde_json::from_str::<PriceResponse>(body_str).map_err(|e| {
                log::warn!("Failed to parse price from response: {:?}", e);
                http::Error::Unknown
            })?;

            // price has 3 decimals
            let price = (price_data.USD * 1000.0) as u32;
            log::info!("ADA price * 1000: {}", price);

            Ok(price)
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

        fn on_finalize(_n: BlockNumberFor<T>) {
            // Calculate and store average price
            let (sum, count) = NodesPrices::<T>::iter_values()
                .fold((0u32, 0u32), |(sum, count), (price, _blocknumber)| {
                    (sum.saturating_add(price), count + 1)
                });

            if count > 0 {
                let average = sum / count;
                Price::<T>::put(average);
            }
        }
    }
}
