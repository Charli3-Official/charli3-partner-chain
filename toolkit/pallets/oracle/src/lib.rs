#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

use frame_support::pallet_prelude::{BoundedVec, ConstU32};
use frame_system::{
	offchain::{SignMessage, Signer},
	pallet_prelude::BlockNumberFor,
};
use hex::ToHex;
use pallet_timestamp::{self as timestamp};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::prelude::{vec, vec::Vec};
use sp_core::crypto::KeyTypeId;
use sp_runtime::SaturatedConversion;
use sp_std::boxed::Box;

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"orac");

mod price_providers;
use price_providers::{CryptoCompareProvider, PriceProvider};

pub const SCALING_FACTOR: f64 = 10000.0;

pub mod crypto {
	use super::KEY_TYPE;
	use sp_core::ed25519::Signature as Ed25519Signature;
	use sp_runtime::{
		app_crypto::{app_crypto, ed25519},
		traits::Verify,
		MultiSignature, MultiSigner,
	};
	app_crypto!(ed25519, KEY_TYPE);

	pub struct OracleAuthId;

	impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for OracleAuthId {
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::ed25519::Signature;
		type GenericPublic = sp_core::ed25519::Public;
	}

	impl frame_system::offchain::AppCrypto<<Ed25519Signature as Verify>::Signer, Ed25519Signature>
		for OracleAuthId
	{
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::ed25519::Signature;
		type GenericPublic = sp_core::ed25519::Public;
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{pallet_prelude::*, traits::BuildGenesisConfig};
	use frame_system::{
		offchain::{AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer},
		pallet_prelude::*,
	};
	use minicbor::encode::Encoder;
	use parity_scale_codec::alloc::vec::Vec as AllocVec;
	use scale_info::{prelude::fmt, TypeInfo};
	use sp_core::hashing::blake2_256;
	use sp_runtime::{offchain::http, sp_std::str};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config + timestamp::Config + CreateSignedTransaction<Call<Self>> + fmt::Debug
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
	pub type Price<T> = StorageValue<_, (OracleMessage, u16)>;

	/// Signatures are indexed by oracle message timestamp.
	/// Second key is the signatory pub key, value is the signature bytes.
	#[pallet::storage]
	pub type SignatureStorage<T: Config> = StorageDoubleMap<
		Hasher1 = Twox64Concat,
		Key1 = u64,
		Hasher2 = Identity,
		Key2 = T::AccountId,
		Value = [u8; 64],
		QueryKind = OptionQuery,
	>;

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
	pub enum AggregationStatus<T: Config> {
		AggregationPerformed {
			non_outliers: u16,
			non_outlier_prices: Vec<u32>,
			outliers: u16,
			outlier_prices: Vec<u32>,
			rewards: Vec<T::AccountId>,
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
		StoredSignature {
			message: OracleMessage,
			who: T::AccountId,
			when: BlockNumberFor<T>,
			signature: T::Signature,
		},
		Status {
			median_price: u32,
			flag: Flag,
			participating_nodes: u32,
			age: u16,
			block: BlockNumberFor<T>,
			status: AggregationStatus<T>,
		},
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, Debug, MaxEncodedLen, TypeInfo)]
	pub struct OracleMessage {
		pub median_price: u32,
		pub timestamp: u64,
		pub rewards: BoundedVec<[u8; 32], ConstU32<64>>, // Vec of byte arrays for ed25519 public keys
	}

	impl Default for OracleMessage {
		fn default() -> Self {
			OracleMessage { median_price: 0, timestamp: 0, rewards: BoundedVec::default() }
		}
	}

	impl OracleMessage {
		pub fn to_cardano_cbor(&self) -> AllocVec<u8> {
			let mut buf = AllocVec::new();
			let mut encoder = Encoder::new(&mut buf);

			// Write tag 121
			encoder.tag(minicbor::data::Tag::new(121)).unwrap();

			// Start indefinite-length array
			encoder.begin_array().unwrap();

			// Add median_price
			encoder.u32(self.median_price).unwrap();

			// Add timestamp (as u32 if it fits, otherwise as u64)
			if self.timestamp <= u32::MAX as u64 {
				encoder.u32(self.timestamp as u32).unwrap();
			} else {
				encoder.u64(self.timestamp).unwrap();
			}

			// Add rewards as array of byte strings
			encoder.begin_array().unwrap();
			for reward_account in &self.rewards {
				encoder.bytes(reward_account).unwrap();
			}
			encoder.end().unwrap(); // End rewards array

			// End main array
			encoder.end().unwrap();

			buf
		}

		pub fn cardano_cbor_hash(&self) -> [u8; 32] {
			let cbor_data = self.to_cardano_cbor();
			blake2_256(&cbor_data)
		}
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
			Self::deposit_event(Event::StoredPrice { price, who: who.clone(), when });
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight((0, Pays::No))]
		pub fn store_signature(
			origin: OriginFor<T>,
			message: OracleMessage,
			signature: T::Signature,
		) -> DispatchResult {
			let who: T::AccountId = ensure_signed(origin)?;
			let when = <frame_system::Pallet<T>>::block_number();

			let mut signature_bytes: AllocVec<u8> = signature.encode();
			signature_bytes.remove(0);
			let signature_encoded: [u8; 64] =
				signature_bytes.try_into().expect("signature buffer should be exactly 64 bytes");
			SignatureStorage::<T>::insert(message.timestamp, &who, signature_encoded);

			Self::deposit_event(Event::StoredSignature { message, who, when, signature });

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
						if let Ok(price) = Self::fetch_price().map_err(|e| {
							log::error!(
								"[{:?}]: failed to fetch price: {:?}",
								signer_account.id,
								e
							);
						}) {
							let result = signer.send_single_signed_transaction(
								&signer_account,
								Call::store_price { price },
							);
							if result.is_some_and(|res| res.is_ok()) {
								log::info!(
									"[{:?}]: submit store price transaction success.",
									signer_account.id
								)
							} else {
								log::error!(
									"[{:?}]: submit store price transaction failure.",
									signer_account.id
								)
							}
						}
						if let Some((message, signature)) = Self::sign_oracle_message(&signer) {
							let result = signer.send_single_signed_transaction(
								&signer_account,
								Call::store_signature { message, signature },
							);
							if result.is_some_and(|res| res.is_ok()) {
								log::info!(
									"[{:?}]: submit store signature transaction success.",
									signer_account.id
								)
							} else {
								log::error!(
									"[{:?}]: submit store signature transaction failure.",
									signer_account.id
								)
							}
						}
					}
				},
				Some(_accounts) => log::error!("More than one account. Expected only one"),
				_none => log::error!("No account available for oracle"),
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
				let prices = NodesPrices::<T>::iter()
					.by_ref()
					.filter_map(|(k, (p, a))| {
						if (n - a) <= feed_age.into() {
							participating_nodes += 1;
							Some((k, p))
						} else {
							None
						}
					})
					.collect();
				let (oracle_message, age, flag, status): (
					OracleMessage,
					u16,
					Flag,
					crate::AggregationStatus<T>,
				) = if min_nodes_for_trusted_aggregation <= participating_nodes {
					log::info!(
						"{:?} nodes submitted a price. Aggregating median price ...",
						participating_nodes
					);
					Self::aggregate(prices, outliers_range, divergence_percentage)
				} else {
					log::error!("Not enough nodes for trusted aggregation. Reusing median ...");
					Self::reuse_previous_median()
				};
				Price::<T>::put((&oracle_message, age));
				log::info!(
					"Median price for block {:?} is {:?} with status: {:?}",
					n,
					oracle_message.median_price,
					flag
				);
				Self::deposit_event(Event::Status {
					median_price: oracle_message.median_price,
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
		acc_and_prices: Vec<(T::AccountId, u32)>,
		outliers_range: u32,
		divergence_percentage: u32,
	) -> (OracleMessage, u16, Flag, crate::AggregationStatus<T>) {
		let mut acc_and_prices =
			BoundedVec::<(T::AccountId, u32), ConstU32<32>>::truncate_from(acc_and_prices);
		acc_and_prices.sort_by_key(|k| k.1);
		let sorted_acc_and_prices = acc_and_prices.to_vec();
		let (_addresses, sorted_prices): (Vec<T::AccountId>, Vec<u32>) =
			sorted_acc_and_prices.clone().into_iter().unzip();
		let median = Self::calculate_median(sorted_prices.clone());
		let (non_outlier_prices, outlier_prices) =
			Self::filter_outliers(sorted_prices, median, outliers_range, divergence_percentage);
		let rewards: Vec<T::AccountId> = sorted_acc_and_prices
			.into_iter()
			.filter_map(
				|(account, price)| {
					if non_outlier_prices.contains(&price) {
						Some(account)
					} else {
						None
					}
				},
			)
			.collect();
		// Get timestamp in milliseconds
		let now_millis = timestamp::Pallet::<T>::get().saturated_into::<u64>();
		let msg = OracleMessage {
			median_price: median,
			timestamp: now_millis,
			rewards: BoundedVec::truncate_from(
				rewards
					.iter()
					.map(|id| {
						let account_bytes = id.encode();
						let account_encoded: [u8; 32] = account_bytes
							.try_into()
							.expect("Account buffer should be exactly 32 bytes");
						account_encoded
					})
					.collect(),
			),
		};

		(
			msg,
			0,
			Flag::Ok,
			AggregationStatus::AggregationPerformed {
				non_outliers: non_outlier_prices.len() as u16,
				non_outlier_prices,
				outliers: outlier_prices.len() as u16,
				outlier_prices,
				rewards,
			},
		)
	}

	fn reuse_previous_median() -> (OracleMessage, u16, Flag, crate::AggregationStatus<T>) {
		if let Some((median, age)) = Price::<T>::get() {
			(median, age + 1, Flag::NotEnoughNodes, AggregationStatus::AggregationNotPerformed)
		} else {
			log::error!("Error: no median to reuse.");
			(
				OracleMessage::default(),
				0,
				Flag::NoPreviousMedian,
				AggregationStatus::AggregationNotPerformed,
			)
		}
	}

	fn calculate_median(prices: Vec<u32>) -> u32 {
		let length: usize = prices.len();

		if length % 2 == 1 {
			// Odd length: return the middle element
			prices[length / 2]
		} else {
			// Even length: average of the two middle elements
			let mid1 = prices[length / 2 - 1];
			let mid2 = prices[length / 2];
			(mid1 + mid2) / 2
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
		outliers_range: u32,
		divergence: u32,
	) -> (Vec<u32>, Vec<u32>) {
		let length: usize = prices.len();

		if length == 1 {
			return (prices, Vec::new());
		}

		let first_quartile =
			Self::calculate_median(prices.clone().into_iter().take(length / 2).collect());
		let third_quartile =
			Self::calculate_median(prices.clone().into_iter().skip(length / 2).collect());

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

	fn sign_oracle_message(
		signer: &Signer<T, <T as Config>::AuthorityId, frame_system::offchain::ForAll>,
	) -> Option<(OracleMessage, T::Signature)> {
		let (message, age) = Price::<T>::get()?;

		if age != 0 {
			return None;
		}

		log::info!("Prepared Message: {:?}", message);
		let cbor_hex: Box<str> = message.to_cardano_cbor().encode_hex();
		log::debug!("Message cbor: {}", cbor_hex);

		let msg_hash_digest = message.cardano_cbor_hash();
		let msg_hash_hex: Box<str> = msg_hash_digest.encode_hex();
		log::debug!("Message hash: {}", msg_hash_hex);

		let signed_message = match signer.sign_message(&msg_hash_digest).pop() {
			Some(signed) => signed,
			_none => {
				log::error!("Couldn't retrieve signature");
				return None;
			},
		};

		log::info!("Account signed: {:?}", signed_message.0.id);
		let hex_signature: Box<str> = signed_message.1.encode().encode_hex();
		log::info!("Signed message: {}", hex_signature);

		Some((message, signed_message.1))
	}
}
