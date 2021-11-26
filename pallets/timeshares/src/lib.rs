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

	type AccountOf<T> = <T as frame_system::Config>::AccountId;
	type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	// pub type RoomHash<T> = Room<AccountIdOf<T>, HashOf<T>>;
	// Struct for holding Kitty information.
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct Kitty<T: Config> {
		pub dna: [u8; 16],
		pub price: Option<BalanceOf<T>>,
		pub hotel_status: HotelStatus,
		pub owner: AccountOf<T>,
	}
	// Struct for holding Timeshare information.
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct Timeshare<T: Config> {
		pub hotel: AccountOf<T>,
		pub room_number: Vec<u8>,
		pub price: Option<BalanceOf<T>>,
		pub owner: AccountOf<T>,
		pub dna: [u8; 16],
	}
	// Struct for holding Room information.
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct Room<T: Config> {
		pub hotel: AccountOf<T>,
		pub room_number: Vec<u8>,
	}
	
	// impl MaxEncodedLen for Kitty {
	// 	fn max_encoded_len(&self) -> usize {
	// 		return self.max_encoded_len(50)
	// 	}
	// }

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub enum HotelStatus {
		Male,
		Female,
		Active,
		Suspended,
		Pending,
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	// #[pallet::generate_storage_info]
	pub struct Pallet<T>(_);

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

	// Errors.
	#[pallet::error]
	pub enum Error<T> {
		MaximumTimesharesForRoomReached,
		/// An account cannot own more Kitties than `MaxKittyCount`.
		ExceedMaxTimesharesPerRoom,
		/// Buyer cannot be the owner.
		BuyerIsKittyOwner,
		/// Cannot transfer a kitty to its owner.
		TransferToSelf,
		/// Handles checking whether the Kitty exists.
		TimeshareDoesntExist,
		/// Handles checking that the Kitty is owned by the account transferring, buying or setting
		/// a price for it.
		NotKittyOwner,
		/// Ensures the Kitty is for sale.
		KittyNotForSale,
		/// Ensures that the buying price is greater than the asking price.
		KittyBidPriceTooLow,
		/// Ensures that an account has enough funds to purchase a Kitty.
		NotEnoughBalance,
		/// Can't mind, duplicate ID
		MintingDuplicateTimeshare,
		HotelAlreadyActive,
		Forbidden,
		HotelNotActive,
		HotelNotRegistered,
	}

	// Events.
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		// Success(String, String), //Time, Date
		/// A new Timeshare was sucessfully created. \[sender, timeshare_id\]
		Created(T::AccountId, T::Hash),
		/// Kitty price was sucessfully set. \[sender, kitty_id, new_price\]
		PriceSet(T::AccountId, T::Hash, Option<BalanceOf<T>>),
		/// A Kitty was sucessfully transferred. \[from, to, kitty_id\]
		Transferred(T::AccountId, T::AccountId, T::Hash),
		/// A Kitty was sucessfully bought. \[buyer, seller, kitty_id, bid_price\]
		Bought(T::AccountId, T::AccountId, T::Hash, BalanceOf<T>),
	}

	#[pallet::storage]
	#[pallet::getter(fn kitty_cnt)]
	pub(super) type KittyCnt<T: Config> = StorageValue<_, u64, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn kitties)]
	pub(super) type Kitties<T: Config> = StorageMap<_, Twox64Concat, T::Hash, Kitty<T>>;

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
	#[pallet::getter(fn kitties_owned)]
	/// Keeps track of which timeshares belong to which room
	pub(super) type KittiesOwned<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
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
	>;

	#[pallet::storage]
	#[pallet::getter(fn hotel_statuses)]
	/// Keeps track of each hotel's status
	pub(super) type HotelStatuses<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, HotelStatus>;

	#[pallet::storage]
	#[pallet::getter(fn admin_key)]
	pub type AdminAccountId<T: Config> = StorageValue<_, T::AccountId, ValueQuery>;

	// TODO Part IV: Our pallet's genesis configuration.
	// Our pallet's genesis configuration.
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub kitties: Vec<(T::AccountId, [u8; 16], HotelStatus)>,
		pub admin_account_id: T::AccountId,
	}

	// Required to implement default for GenesisConfig.
	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> GenesisConfig<T> {
			GenesisConfig { kitties: vec![], admin_account_id: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			// When building a kitty from genesis config, we require the dna and hotel_status to be
			// supplied.
			// for (acct, dna, hotel_status) in &self.kitties {
				// let _ = <Pallet<T>>::mint(acct, Some(dna.clone()), Some(hotel_status.clone()));
			// }
			AdminAccountId::<T>::put(&self.admin_account_id);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		// TODO Part III: create_kitty
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

		/// Set the price for a Kitty.
		/// Updates Kitty price and updates storage.
		#[pallet::weight(100)]
		pub fn set_price(
			origin: OriginFor<T>,
			kitty_id: T::Hash,
			new_price: Option<BalanceOf<T>>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			ensure!(Self::is_timeshare_owner(&kitty_id, &sender)?, <Error<T>>::NotKittyOwner);

			let mut kitty = Self::kitties(&kitty_id).ok_or(<Error<T>>::TimeshareDoesntExist)?;

			kitty.price = new_price.clone();

			<Kitties<T>>::insert(kitty_id, kitty);

			// Deposit a "PriceSet" event.
			Self::deposit_event(Event::PriceSet(sender, kitty_id, new_price));

			Ok(())
		}

		#[pallet::weight(100)]
		pub fn transfer(
			origin: OriginFor<T>,
			to: T::AccountId,
			kitty_id: T::Hash,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;

			// Ensure the kitty exists and is called by the kitty owner
			ensure!(Self::is_timeshare_owner(&kitty_id, &from)?, <Error<T>>::NotKittyOwner);

			// Verify the kitty is not transferring back to its owner.
			ensure!(from != to, <Error<T>>::TransferToSelf);

			// Verify the recipient has the capacity to receive one more kitty
			let to_owned = <KittiesOwned<T>>::get(&to);
			ensure!(
				(to_owned.len() as u32) < T::MaxTimesharesPerRoom::get(),
				<Error<T>>::ExceedMaxTimesharesPerRoom
			);

			Self::transfer_timeshare_to(&kitty_id, &to)?;

			Self::deposit_event(Event::Transferred(from, to, kitty_id));

			Ok(())
		}

		// buy_kitty
		#[transactional]
		#[pallet::weight(100)]
		pub fn buy_kitty(
			origin: OriginFor<T>,
			kitty_id: T::Hash,
			bid_price: BalanceOf<T>,
		) -> DispatchResult {
			let buyer = ensure_signed(origin)?;

			// Check the kitty exists and buyer is not the current kitty owner
			let kitty = Self::kitties(&kitty_id).ok_or(<Error<T>>::TimeshareDoesntExist)?;
			ensure!(kitty.owner != buyer, <Error<T>>::BuyerIsKittyOwner);

			// ACTION #6: Check if the Kitty is for sale.
			// Check the kitty is for sale and the kitty ask price <= bid_price
			if let Some(ask_price) = kitty.price {
				ensure!(ask_price <= bid_price, <Error<T>>::KittyBidPriceTooLow);
			} else {
				Err(<Error<T>>::KittyNotForSale)?;
			}

			// Check the buyer has enough free balance
			ensure!(T::Currency::free_balance(&buyer) >= bid_price, <Error<T>>::NotEnoughBalance);

			// ACTION #7: Check if buyer can receive Kitty.
			// Verify the buyer has the capacity to receive one more kitty
			let buyer_owned = <KittiesOwned<T>>::get(&buyer);
			ensure!(
				(buyer_owned.len() as u32) < T::MaxTimesharesPerRoom::get(),
				<Error<T>>::ExceedMaxTimesharesPerRoom
			);

			let seller = kitty.owner.clone();

			// Transfer the amount from buyer to seller
			//the '?' ensures Ok() is returned else exit early
			T::Currency::transfer(&buyer, &seller, bid_price, ExistenceRequirement::KeepAlive)?;

			// Transfer the kitty from seller to buyer
			Self::transfer_timeshare_to(&kitty_id, &buyer)?;

			// Deposit relevant Event
			Self::deposit_event(Event::Bought(buyer, seller, kitty_id, bid_price));

			Ok(())
		}

		/// For Admins to to set a hotel's status. "Active" to allow timeshares.
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
			let timeshare = Timeshare::<T> {
				price: price.clone(),
				owner: owner.clone(),
				hotel: owner.clone(),
				room_number: room_number.clone(),
				dna: Self::gen_dna(),
			};
			let timeshare_id = T::Hashing::hash_of(&timeshare);
			ensure!(!<Timeshares<T>>::contains_key(timeshare_id), <Error<T>>::MintingDuplicateTimeshare);
			//add timeshare to room
			let room = Room::<T> {
				hotel: owner.clone(),
				room_number: room_number,
			};
			let room_hash = T::Hashing::hash_of(&room);
			<RoomsTimeshares<T>>::try_mutate(room_hash, |vec| vec.try_push(timeshare_id))
				.map_err(|_| <Error<T>>::ExceedMaxTimesharesPerRoom)?;

			// Self::transfer_timeshare_to(&timeshare_id, owner)?;
			<TimesharesOwned<T>>::mutate(&owner, |timeshare_vec| {
				match timeshare_vec {
					None => {
						log::info!("New Timeshare Owner");
						let mut vec = Vec::<T::Hash>::new();
						vec.push(timeshare_id);
						<TimesharesOwned<T>>::insert(owner, vec);
						return;
					},
					Some(timeshare_vec) => {
						log::info!("Adding Timeshare pre-existing Owner");
						return timeshare_vec.push(timeshare_id); 
					}
				}
			});
			<Timeshares<T>>::insert(timeshare_id, timeshare);
			// <KittyCnt<T>>::put(new_cnt);
			// let now = Utc::now().naive_utc();
			// Self::deposit_event(Event::Success(now.format("%-I:%M %p").to_string(),
			// now.format("%Y-%m-%d").to_string()));
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

			// Remove `timeshare_id` from the KittyOwned vector of `prev_timeshare_owner`
			//TODO: change to TimeshareOwnec
			<KittiesOwned<T>>::try_mutate(&prev_owner, |owned| {
				if let Some(index) = owned.iter().position(|&id| id == *timeshare_id) {
					owned.swap_remove(index);
					return Ok(())
				}
				Err(())
			})
			.map_err(|_| <Error<T>>::TimeshareDoesntExist)?;

			// <TimesharesOwned<T>>::mutate(&prev_owner, |timeshare_vec| {
			// 	match timeshare_vec {
			// 		None => {
			// 			return Err(())
			// 		},
			// 		Some(timeshare_vec) => {
			// 			timeshare_vec.swap_remove(*timeshare_id); 
			// 			return Ok(())
			// 		}
			// 	}
			// });

			// Update the timeshare owner
			timeshare.owner = to.clone();
			// Reset the ask price so the timeshare is not for sale until `set_price()` is called
			// by the current owner.
			timeshare.price = None;

			<Timeshares<T>>::insert(timeshare_id, timeshare);

			//TODO: change to TimeshareOwned
			<KittiesOwned<T>>::try_mutate(to, |vec| vec.try_push(*timeshare_id))
				.map_err(|_| <Error<T>>::ExceedMaxTimesharesPerRoom)?;

			Ok(())
		}
	}
}
