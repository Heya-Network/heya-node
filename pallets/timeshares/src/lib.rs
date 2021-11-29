#![cfg_attr(not(feature = "std"), no_std)]
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		pallet_prelude::*,
		sp_runtime::traits::Hash,
		traits::{Randomness, tokens::ExistenceRequirement, Currency},
		transactional,
	};
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_io::hashing::blake2_128;
	use scale_info::prelude::vec::Vec;
	// use codec::{MaxEncodedLen};
	// use chrono::*;
	#[cfg(feature = "std")]
	use frame_support::serde::{Deserialize, Serialize};

	/// Configure the pallet by specifying the parameters and types it depends on.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// The Currency handler for the Timeshares pallet.
		type Currency: Currency<Self::AccountId>;
		type Randomness: Randomness<Self::Hash, Self::BlockNumber>;
		#[pallet::constant]
		type MaxTimesharesPerRoom: Get<u32>;
	}

	type AccountOf<T> = <T as frame_system::Config>::AccountId;
	type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
	pub type HashOf<T> = <T as frame_system::Config>::Hash;

	// Errors.
	#[pallet::error]
	pub enum Error<T> {
		/// A room cannot have more timeshares than `MaxTimesharesPerRoom`.
		MaximumTimesharesForRoomReached,
		/// Buyer cannot be the owner.
		BuyerIsTimeshareOwner,
		/// Cannot transfer a timeshare to its owner.
		TransferToSelf,
		/// Handles checking whether the timeshare exists.
		TimeshareDoesntExist,
		/// Non-owner is attempting an action only the owner is allowed to perform
		NotTimeshareOwner,
		/// Ensures the timeshare is for sale.
		NotForSale,
		/// Ensures that the buying price is greater than the asking price.
		BidPriceTooLow,
		/// Ensures buyer account has enough free funds to purchase a timeshare.
		NotEnoughFreeBalance,
		/// Can't mint, duplicate ID
		MintingDuplicateTimeshare,
		HotelAlreadyActive,
		HotelAlreadyPending,
		Forbidden,
		HotelNotActive,
		HotelNotRegistered,
		NotFoundInTimesharesOwned,
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub enum HotelStatus {
		Active,
		Suspended,
		Pending,
	}

	// Events.
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		// Success(String, String), //Time, Date
		/// A new Timeshare was sucessfully created. \[sender, timeshare_id\]
		Created(T::AccountId, T::Hash),
		/// Timeshare price was sucessfully set. \[sender, timeshare_id, new_price\]
		PriceSet(T::AccountId, T::Hash, Option<BalanceOf<T>>),
		/// A Timeshare was sucessfully transferred. \[from, to, timeshare_id\]
		Transferred(T::AccountId, T::AccountId, T::Hash),
		/// A Timeshare was sucessfully bought. \[buyer, seller, timeshare_id, bid_price\]
		Bought(T::AccountId, T::AccountId, T::Hash, BalanceOf<T>),
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	// #[pallet::generate_storage_info]
	pub struct Pallet<T>(_);

	// Struct for holding Timeshare information.
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct Timeshare<T: Config> {
		pub hotel: AccountOf<T>, //TODO: make this hex so it's the same as room_hash?
		pub room_number: Vec<u8>,
		pub price: Option<BalanceOf<T>>,
		pub owner: AccountOf<T>, //TODO: make this hex so it's the same as room_hash?
		pub dna: [u8; 16],
		pub room_hash: T::Hash,
	}

	// Struct for holding Room information.
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct Room<T: Config> {
		pub hotel: AccountOf<T>,
		pub room_number: Vec<u8>,
	}

	#[pallet::storage]
	#[pallet::getter(fn timeshares)]
	pub(super) type Timeshares<T: Config> = StorageMap<_, Twox64Concat, T::Hash, Timeshare<T>>;

	#[pallet::storage]
	#[pallet::getter(fn rooms_timeshares)]
	/// Keeps track of which timeshares belong to which room
	pub(super) type RoomsTimeshares<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::Hash, //TODO: create type for hash of room
		BoundedVec<T::Hash, T::MaxTimesharesPerRoom>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn timeshares_owned)]
	/// Keeps track of which timeshares belong to which AccountId
	pub(super) type TimesharesOwned<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		Vec<T::Hash>, //TODO: create type hash of timeshare
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn hotel_statuses)]
	/// Keeps track of each hotel's status
	pub(super) type HotelStatuses<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, HotelStatus>;

	#[pallet::storage]
	#[pallet::getter(fn admin_key)]
	pub type AdminAccountId<T: Config> = StorageValue<_, T::AccountId, ValueQuery>;

	// Our pallet's genesis configuration.
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub seeded_timeshares: Vec<(T::AccountId, Vec<u8>, Option<BalanceOf<T>>)>,
		pub admin_account_id: T::AccountId,
	}

	// Required to implement default for GenesisConfig.
	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> GenesisConfig<T> {
			GenesisConfig { seeded_timeshares: vec![], admin_account_id: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			// When building a timeshare from the genesis config, we require the owner, room_number and price (can be None) to be supplied.
			for (acct, room_number, price) in &self.seeded_timeshares {
				let _ = <Pallet<T>>::mint(acct, room_number.clone(), price.clone());
			}
			//Set the admin_account_id from the chainspec for privileged extrinsics
			AdminAccountId::<T>::put(&self.admin_account_id);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {

		#[pallet::weight(0)]
		pub fn register_hotel(origin: OriginFor<T>) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let current_status = <HotelStatuses<T>>::get(&sender);
			ensure!(current_status == None, <Error<T>>::HotelAlreadyPending);
			<HotelStatuses<T>>::insert(sender, HotelStatus::Pending);
			Ok(())
		}

		#[pallet::weight(100)]
		pub fn create_timeshare(
			origin: OriginFor<T>, 
			room_number: Vec<u8>, 
			price: Option<BalanceOf<T>>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(Self::is_active(&sender)?, <Error<T>>::HotelNotActive);
			let timeshare_id = Self::mint(&sender, room_number, price)?; 
			log::info!("A timeshare is born with ID: {:?}.", timeshare_id);
			Self::deposit_event(Event::Created(sender, timeshare_id));
			Ok(())
		}

		/// Set the price for a Timeshare. Updates Timeshare price and updates storage.
		#[pallet::weight(100)]
		pub fn set_price(
			origin: OriginFor<T>,
			timeshare_id: T::Hash,
			new_price: Option<BalanceOf<T>>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			ensure!(Self::is_timeshare_owner(&timeshare_id, &sender)?, <Error<T>>::NotTimeshareOwner);

			let mut timeshare = Self::timeshares(&timeshare_id).ok_or(<Error<T>>::TimeshareDoesntExist)?;

			timeshare.price = new_price.clone();

			<Timeshares<T>>::insert(timeshare_id, timeshare);

			// Deposit a "PriceSet" event.
			Self::deposit_event(Event::PriceSet(sender, timeshare_id, new_price));

			Ok(())
		}

		#[pallet::weight(100)]
		pub fn transfer(
			origin: OriginFor<T>,
			to: T::AccountId,
			timeshare_id: T::Hash,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;

			// Ensure the timeshare exists and is called by the timeshare owner
			ensure!(Self::is_timeshare_owner(&timeshare_id, &from)?, <Error<T>>::NotTimeshareOwner);

			// Verify the owner is not transferring to themselves.
			ensure!(from != to, <Error<T>>::TransferToSelf);

			Self::transfer_timeshare_to(&timeshare_id, &to)?;

			Self::deposit_event(Event::Transferred(from, to, timeshare_id));

			Ok(())
		}

		/// Buy Timeshare
		#[transactional]
		#[pallet::weight(100)]
		pub fn buy_timeshare(
			origin: OriginFor<T>,
			timeshare_id: T::Hash,
			bid_price: BalanceOf<T>,
		) -> DispatchResult {
			let buyer = ensure_signed(origin)?;

			// Check the timeshare exists and buyer is not the current timeshare owner
			let timeshare = Self::timeshares(&timeshare_id).ok_or(<Error<T>>::TimeshareDoesntExist)?;
			ensure!(timeshare.owner != buyer, <Error<T>>::BuyerIsTimeshareOwner);

			// Check the timeshare is for sale and the timeshare ask price <= bid_price
			if let Some(ask_price) = timeshare.price {
				ensure!(ask_price <= bid_price, <Error<T>>::BidPriceTooLow);
			} else {
				Err(<Error<T>>::NotForSale)?;
			}

			// Check the buyer has enough free balance
			ensure!(T::Currency::free_balance(&buyer) >= bid_price, <Error<T>>::NotEnoughFreeBalance);

			let seller = timeshare.owner.clone();

			// Transfer the amount from buyer to seller. The '?' ensures Ok() is returned else exit early
			T::Currency::transfer(&buyer, &seller, bid_price, ExistenceRequirement::KeepAlive)?;

			// Transfer the timeshare from seller to buyer
			Self::transfer_timeshare_to(&timeshare_id, &buyer)?;

			// Deposit relevant Event
			Self::deposit_event(Event::Bought(buyer, seller, timeshare_id, bid_price));

			Ok(())
		}

		/// For Admins to set a hotel's status. "Active" to allow timeshares.
		#[pallet::weight(0)]
		pub fn set_hotel_status(origin: OriginFor<T>, hotel: T::AccountId, status: HotelStatus) -> DispatchResult {
			let admin = ensure_signed(origin)?;
			log::info!("signer ID: {:?}.", admin);
			log::info!("admin ID: {:?}.", AdminAccountId::<T>::get());
			// ensure!(admin == AdminAccountId::<T>::get(), Error::<T>::Forbidden);
			// let current_status = <HotelStatuses<T>>::get(&hotel);
			// if match current_status {
			// 	Some(s) => current_status = s,
			// 	None => Err(())
			// }
			// ensure!(matches!(current_status, status), <Error<T>>::HotelAlreadyActive);
			<HotelStatuses<T>>::insert(hotel, status);
			Ok(())
		}
	}

	//** Our helper functions.**//
	impl<T: Config> Pallet<T> {

		#[transactional]
		pub fn mint(
			owner: &T::AccountId,
			room_number: Vec<u8>,
			price: Option<BalanceOf<T>>,
		) -> Result<T::Hash, Error<T>> {
			let room = Room::<T> {
				hotel: owner.clone(),
				room_number: room_number.clone(),
			};
			let room_hash = T::Hashing::hash_of(&room);
			let timeshare = Timeshare::<T> {
				price: price.clone(),
				owner: owner.clone(),
				hotel: owner.clone(),
				room_number: room_number.clone(),
				dna: Self::gen_dna(),
				room_hash: room_hash,
			};
			let timeshare_id = T::Hashing::hash_of(&timeshare);
			ensure!(!<Timeshares<T>>::contains_key(timeshare_id), <Error<T>>::MintingDuplicateTimeshare);
			
			//add timeshare to room
			<RoomsTimeshares<T>>::try_mutate(room_hash, |vec| vec.try_push(timeshare_id))
				.map_err(|_| <Error<T>>::MaximumTimesharesForRoomReached)?;

			<TimesharesOwned<T>>::mutate(&owner, |timeshare_vec| {
				timeshare_vec.push(timeshare_id);
			});
			<Timeshares<T>>::insert(timeshare_id, timeshare);
			// let now = Utc::now().naive_utc();
			// Self::deposit_event(Event::Success(now.format("%-I:%M %p").to_string(),
			Ok(timeshare_id)
		}

		fn gen_dna() -> [u8; 16] {
			let payload = (
				T::Randomness::random(&b"dna"[..]).0,
				<frame_system::Pallet<T>>::block_number(),
			);
			payload.using_encoded(blake2_128)
		}

		pub fn is_timeshare_owner(timeshare_id: &T::Hash, acct: &T::AccountId) -> Result<bool, Error<T>> {
			match Self::timeshares(timeshare_id) {
				Some(timeshare) => Ok(timeshare.owner == *acct),
				None => Err(<Error<T>>::TimeshareDoesntExist),
			}
		}

		pub fn is_active(hotel: &T::AccountId) -> Result<bool, Error<T>> {
			match Self::hotel_statuses(hotel) {
				Some(status) => Ok(status == HotelStatus::Active),
				None => Err(<Error<T>>::HotelNotRegistered),
			}
		}

		#[transactional]
		pub fn transfer_timeshare_to(timeshare_id: &T::Hash, to: &T::AccountId) -> Result<(), Error<T>> {
			let mut timeshare = Self::timeshares(&timeshare_id).ok_or(<Error<T>>::TimeshareDoesntExist)?;

			let prev_owner = timeshare.owner.clone();

			<TimesharesOwned<T>>::try_mutate(&prev_owner, |owned| {
				if let Some(index) = owned.iter().position(|&id| id == *timeshare_id) {
					owned.swap_remove(index);
					return Ok(())
				}
				Err(())
			})
			.map_err(|_| <Error<T>>::NotFoundInTimesharesOwned)?;

			// Update the timeshare owner
			timeshare.owner = to.clone();
			// Reset the ask price so the timeshare is not for sale until `set_price()` is called
			// by the current owner.
			timeshare.price = None;
			//update Timeshares with updated timeshare
			<Timeshares<T>>::insert(timeshare_id, timeshare);

			//add timeshare_id to new owner's vec
			<TimesharesOwned<T>>::mutate(to, |vec| vec.push(*timeshare_id));

			Ok(())
		}
	}
}
