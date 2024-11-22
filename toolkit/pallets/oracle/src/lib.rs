#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

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

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        StoredPrice {
            price: u32,
            who: T::AccountId,
            when: BlockNumberFor<T>,
        },
    }

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

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
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
