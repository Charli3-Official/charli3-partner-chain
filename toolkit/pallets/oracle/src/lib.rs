#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

use frame_support::pallet_prelude::{BoundedVec, ConstU32};
use frame_system::pallet_prelude::BlockNumberFor;
use scale_info::prelude::vec::Vec;
use sp_core::crypto::KeyTypeId;

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"orac");

mod price_providers;
use price_providers::{PriceProvider, CryptoCompareProvider};

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
    use codec::{Decode, Encode, MaxEncodedLen};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::BuildGenesisConfig;
    use frame_support::traits::BuildGenesisConfig;
    use frame_system::{
        offchain::{AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer},
        pallet_prelude::*,
    };
    use scale_info::{
        prelude::{fmt, vec},
        TypeInfo,
    };
    use scale_info::prelude::vec;
    use sp_runtime::offchain::{http};
    use sp_runtime::sp_std::str;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + SigningTypes
        + CreateSignedTransaction<Call<Self>>
        + pallet_timestamp::Config
        + fmt::Debug
    {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// AuthorityId for offchain signing. Uses the associated `Public`/`Signature` from SigningTypes.
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
    /// The first value is the median price
    /// The second value is the age of the median price
    #[pallet::storage]
    pub type Price<T> = StorageValue<_, (u32, BlockNumberFor<T>)>;

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

    // Error messages
    #[derive(Clone, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, Debug)]
    pub enum ErrorMessage {
        NotEnoughNodes,
        NoPreviousMedian,
    }

    // Aggregation status flag
    // Used to include more information about an uncommon aggregation if it occurs.
    #[derive(Clone, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, Debug)]
    pub enum Flag<T: Config> {
        Ok,
        Error {
            message: ErrorMessage,
            price_age: BlockNumberFor<T>,
        },
    }

    /// pallet events
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        StoredPrices {
            count: u16,
            who: T::AccountId,
            when: BlockNumberFor<T>,
        },
        AggregationStatus {
            median: u32,
            block: BlockNumberFor<T>,
            flag: Flag<T>,
            non_outliers: Vec<u32>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        InvalidOracleConfig,
    }

    /// pallet calls
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight((0, Pays::No))]
        pub fn store_prices(origin: OriginFor<T>, prices: Vec<(TradePair, u64)>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            // Only authorized oracle nodes can submit prices
            ensure!(
                AuthorizedOracleNodes::<T>::contains_key(&who),
                Error::<T>::UnauthorizedNode
            );

            let when = <frame_system::Pallet<T>>::block_number();

            prices.iter().for_each(|(tp, price)| {
                NodesPrices::<T>::insert(tp, &who, (price, when));
            });
            Self::deposit_event(Event::StoredPrices {
                count: TryInto::<u16>::try_into(prices.len())
                    .map_err(|_| sp_runtime::DispatchError::Other("CountOverflow"))?,
                who: who.clone(),
                when,
            });
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight((0, Pays::No))]
        pub fn store_signatures(
            origin: OriginFor<T>,
            signatures: Vec<(OracleMessage, T::Signature)>,
        ) -> DispatchResult {
            let who: T::AccountId = ensure_signed(origin)?;
            ensure!(
                AuthorizedOracleNodes::<T>::contains_key(&who),
                Error::<T>::UnauthorizedNode
            );

            let when = <frame_system::Pallet<T>>::block_number();

            signatures
                .clone()
                .into_iter()
                .for_each(|(message, signature)| {
                    let mut signature_bytes: AllocVec<u8> = signature.encode();
                    signature_bytes.remove(0);
                    let signature_encoded: [u8; 64] = signature_bytes
                        .try_into()
                        .expect("signature buffer should be exactly 64 bytes");
                    SignatureStorage::<T>::insert(message.timestamp, &who, signature_encoded);
                });

            Self::deposit_event(Event::StoredSignatures {
                who,
                when,
                signatures,
            });

            Ok(())
        }

    }

    /// pallet auxiliary methods
    impl<T: Config> Pallet<T> {
        pub fn fetch_prices(tickers: Vec<String>) -> Result<Vec<u32>, http::Error> {
            CryptoCompareProvider::fetch_prices(tickers)
        }
    }

    /// pallet hooks
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        // Offchain worker that triggers the extrinsic submitting a price to the
        // NodePrices storage
        fn offchain_worker(_n: BlockNumberFor<T>) {
            log::info!("Starting offchain worker to query price from configured sources");
            let mut acc_list = Signer::<T, T::AuthorityId>::keystore_accounts();
            match acc_list.next() {
                Some(signer_account) if acc_list.next().is_none() => {
                    let signer = Signer::<T, T::AuthorityId>::all_accounts()
                        .with_filter(vec![signer_account.clone().public]);
                    let msg = OracleMessage {
                        median_price: 700,
                        timestamp: 1746529250,
                    };
                    let cbor_hex: Box<str> = msg.to_cardano_cbor().encode_hex();
                    log::info!("Message cbor: {}", cbor_hex);

                    if signer.can_sign() {
                        if let Some(signed_message) = signer.sign_message(b"something").pop() {
                            log::info!("Account signed: {:?}", signed_message.0.id);
                            // SignatureStorage::<T>::put(signed_message.1);
                            // log::info!("Stored signed message");
                            log::info!("Signed message: {0:#?}", signed_message.1);
                        } else {
                            log::error!("Couldn't retrieve signature");
                        }
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
                _none => log::error!("No account available for oracle"),
            }
        }

        fn on_finalize(n: BlockNumberFor<T>) {
            // Calculate and store median price
            // let feed_age = FeedAge::<T>::get(); // config checker that returns these values
            log::info!("Aggregating median price for block {:?}", n);
            let feed_age: u32 = 2;
            let min_nodes_for_trusted_aggregation  = 2;
            let mut count : u32 = 0;
            let prices = NodesPrices::<T>::iter_values()
                .filter_map(|(p, a)| if a <= feed_age.into() {
                        count += 1;
                        Some(p)
                    } else {
                        None
                    }
                )
                .collect();
            if min_nodes_for_trusted_aggregation <= count {
                log::info!("{:?} nodes have submitted prices. Calculating median..", count);
                let mut sorted_prices = BoundedVec::<u32, ConstU32<32>>::truncate_from(prices);
                sorted_prices.sort();
                let median;
                let length = sorted_prices.len();
                if count % 2 == 0 {
                    median = sorted_prices[(length - 1) /2];
                } else {
                    median = (sorted_prices[(length - 1)/2] + sorted_prices[length/2]) / 2;
                }
                Price::<T>::put(median);
            } else {
                log::error!("Not enough nodes for trusted aggregation. Reusing median ...");
            }

        }
    }
}
