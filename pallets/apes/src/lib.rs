#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	// use frame_support::{
	// 	dispatch::{DipatchResult, DispatchResultWithPostInfo},
	// 	pallet_prelude::*,
	// 	sp_runtime::traits::{Hash, Zero},
	// 	traits::{Currency, ExistenceRequirement, Randomness},
	// };

	// use frame_support::prelude::*;
	use frame_support::{
		pallet_prelude::*,
		sp_runtime::traits::Hash,
		traits::{tokens::ExistenceRequirement, Currency, Randomness},
		transactional,
	};
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_io::hashing::blake2_128;

	#[cfg(feature = "std")]
	use frame_support::serde::{Deserialize, Serialize};

	type AccountOf<T> = <T as frame_system::Config>::AccountId;
	type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Struct for holding Ape information
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	#[codec(mel_bound())]
	pub struct Ape<T: Config> {
		pub dna: [u8; 16],
		pub price: Option<BalanceOf<T>>,
		pub owner: AccountOf<T>,
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// The currency handler for the Apes pallet
		type Currency: Currency<Self::AccountId>;
		type ApeRandomness: Randomness<Self::Hash, Self::BlockNumber>;

		#[pallet::constant]
		type MaxApeOwned: Get<u32>;
	}

	// Errors
	#[pallet::error]
	pub enum Error<T> {
		ApeCountOverflow,
		/// An account cannot own more APes than `MaxApeCount`.
		ExceedMaxApeOwned,
		/// Buyer cannot be the owner.
		BuyerIsApeOwner,
		/// Cannot transfer a ape to its owner.
		TransferToSelf,
		/// This ape already exists
		ApeExists,
		/// This ape doesn't exist
		ApeNotExist,
		/// Handles checking that the ape is owned by the account transferring, buying or setting a price for it.
		NotApeOwner,
		/// Ensures the Ape is for sale.
		ApeNotForSale,
		/// Ensures that the buying price is greater than the asking price.
		ApeBidPriceTooLow,
		/// Ensures that an account has enough funds to purchase a Ape.
		NotEnoughBalance,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Created(T::AccountId, T::Hash),
		PriceSet(T::AccountId, T::Hash, Option<BalanceOf<T>>),
		Transferred(T::AccountId, T::AccountId, T::Hash),
		Bought(T::AccountId, T::AccountId, T::Hash, BalanceOf<T>),
	}

	#[pallet::storage]
	#[pallet::getter(fn ape_count)]
	/// Keeps track of the number of Apes in existence.
	pub(super) type ApeCount<T: Config> = StorageValue<_, u64, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn apes)]
	pub(super) type Apes<T: Config> = StorageMap<_, Twox64Concat, T::Hash, Ape<T>>;

	#[pallet::storage]
	#[pallet::getter(fn apes_owned)]
	/// Keeps track of what accounts own what Ape.
	pub(super) type ApesOwned<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, BoundedVec<T::Hash, T::MaxApeOwned>, ValueQuery>;

	// Our pallet's genesis configuration.
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub apes: Vec<(T::AccountId, [u8; 16])>,
	}

	// Required to implement default for GenesisConfig.
	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> GenesisConfig<T> {
			GenesisConfig { apes: vec![] }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			// When building a kitty from genesis config, we require the dna and gender to be supplied.
			for (account, dna) in &self.apes {
				let _ = <Pallet<T>>::mint(account, Some(dna.clone()));
			}
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn create_ape(origin: OriginFor<T>) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let ape_id = Self::mint(&sender, None)?;

			log::info!("An ape is born with ID: {:?}.", ape_id);

			Self::deposit_event(Event::Created(sender, ape_id));

			Ok(())
		}
		/// Updates Ape price and updates storage.
		#[pallet::weight(100)]
		pub fn set_price(
			origin: OriginFor<T>,
			ape_id: T::Hash,
			new_price: Option<BalanceOf<T>>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			// Ensure the kitty exists and is called by the kitty owner
			ensure!(Self::is_ape_owner(&ape_id, &sender)?, <Error<T>>::NotApeOwner);

			let mut ape = Self::apes(&ape_id).ok_or(<Error<T>>::ApeNotExist)?;

			ape.price = new_price.clone();
			<Apes<T>>::insert(&ape_id, ape);

			// Deposit a "PriceSet" event.
			Self::deposit_event(Event::PriceSet(sender, ape_id, new_price));

			Ok(())
		}
		/// Any account that holds an ape can send it to another Account. This will reset the asking
		/// price of the ape, marking it not for sale.
		#[pallet::weight(100)]
		pub fn transfer(origin: OriginFor<T>, to: T::AccountId, ape_id: T::Hash) -> DispatchResult {
			let from = ensure_signed(origin)?;

			// Ensure the kitty exists and is called by the kitty owner
			ensure!(Self::is_ape_owner(&ape_id, &from)?, <Error<T>>::NotApeOwner);

			// Verify the kitty is not transferring back to its owner.
			ensure!(from != to, <Error<T>>::TransferToSelf);

			// Verify the recipient has the capacity to receive one more kitty
			let to_owned = <ApesOwned<T>>::get(&to);
			ensure!((to_owned.len() as u32) < T::MaxApeOwned::get(), <Error<T>>::ExceedMaxApeOwned);

			Self::transfer_ape_to(&ape_id, &to)?;

			Self::deposit_event(Event::Transferred(from, to, ape_id));

			Ok(())
		}
		/// Buy a saleable Ape. The bid price provided from the buyer has to be equal or higher
		/// than the ask price from the seller.
		/// Marking this method `transactional` so when an error is returned, we ensure no storage is changed.
		#[transactional]
		#[pallet::weight(100)]
		pub fn buy_ape(
			origin: OriginFor<T>,
			ape_id: T::Hash,
			bid_price: BalanceOf<T>,
		) -> DispatchResult {
			let buyer = ensure_signed(origin)?;

			// Check the ape exists and buyer is not the current ape owner
			let ape = Self::apes(&ape_id).ok_or(<Error<T>>::ApeNotExist)?;
			ensure!(ape.owner != buyer, <Error<T>>::BuyerIsApeOwner);

			// Check the ape is for sale and the ape ask price <= bid_price
			if let Some(ask_price) = ape.price {
				ensure!(ask_price <= bid_price, <Error<T>>::ApeBidPriceTooLow);
			} else {
				Err(<Error<T>>::ApeNotForSale)?;
			}

			// Check the buyer has enough free balance
			ensure!(T::Currency::free_balance(&buyer) >= bid_price, <Error<T>>::NotEnoughBalance);

			// Verify the buyer has the capacity to receive one more kitty
			let to_owned = <ApesOwned<T>>::get(&buyer);
			ensure!((to_owned.len() as u32) < T::MaxApeOwned::get(), <Error<T>>::ExceedMaxApeOwned);

			let seller = ape.owner.clone();

			// Transfer the amount from buyer to seller
			T::Currency::transfer(&buyer, &seller, bid_price, ExistenceRequirement::KeepAlive)?;

			// Transfer the kitty from seller to buyer
			Self::transfer_ape_to(&ape_id, &buyer)?;

			Self::deposit_event(Event::Bought(buyer, seller, ape_id, bid_price));

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn gen_dna() -> [u8; 16] {
			let payload = (
				T::ApeRandomness::random(&b"dna"[..]).0,
				<frame_system::Pallet<T>>::extrinsic_index().unwrap_or_default(),
				<frame_system::Pallet<T>>::block_number(),
			);
			payload.using_encoded(blake2_128)
		}

		// Helper to mint an ape.
		pub fn mint(owner: &T::AccountId, dna: Option<[u8; 16]>) -> Result<T::Hash, Error<T>> {
			let ape = Ape::<T> {
				dna: dna.unwrap_or_else(Self::gen_dna),
				price: None,
				owner: owner.clone(),
			};

			let ape_id = T::Hashing::hash_of(&ape);

			// Performs this operation first as it may fail
			let new_count = Self::ape_count().checked_add(1).ok_or(<Error<T>>::ApeCountOverflow)?;

			// Check if the kitty does not already exist in our storage map
			ensure!(Self::apes(&ape_id) == None, <Error<T>>::ApeExists);

			// Performs this operation first because as it may fail
			<ApesOwned<T>>::try_mutate(&owner, |ape_vec| ape_vec.try_push(ape_id))
				.map_err(|_| <Error<T>>::ExceedMaxApeOwned)?;

			<Apes<T>>::insert(ape_id, ape);
			<ApeCount<T>>::put(new_count);
			Ok(ape_id)
		}

		pub fn is_ape_owner(ape_id: &T::Hash, account: &T::AccountId) -> Result<bool, Error<T>> {
			match Self::apes(ape_id) {
				Some(ape) => Ok(ape.owner == *account),
				None => Err(<Error<T>>::ApeNotExist),
			}
		}
		#[transactional]
		pub fn transfer_ape_to(ape_id: &T::Hash, to: &T::AccountId) -> Result<(), Error<T>> {
			let mut ape = Self::apes(&ape_id).ok_or(<Error<T>>::ApeNotExist)?;

			let prev_owner = ape.owner.clone();

			// Remove `kitty_id` from the KittyOwned vector of `prev_kitty_owner`
			<ApesOwned<T>>::try_mutate(&prev_owner, |owned| {
				if let Some(ind) = owned.iter().position(|&id| id == *ape_id) {
					owned.swap_remove(ind);
					return Ok(());
				}
				Err(())
			})
			.map_err(|_| <Error<T>>::ApeNotExist)?;

			// Update the kitty owner
			ape.owner = to.clone();
			// Reset the ask price so the kitty is not for sale until `set_price()` is called
			// by the current owner.
			ape.price = None;

			<Apes<T>>::insert(ape_id, ape);

			<ApesOwned<T>>::try_mutate(to, |vec| vec.try_push(*ape_id))
				.map_err(|_| <Error<T>>::ExceedMaxApeOwned)?;

			Ok(())
		}
	}
}
