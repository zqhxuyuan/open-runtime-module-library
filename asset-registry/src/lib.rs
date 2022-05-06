#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::large_enum_variant)]

use frame_support::pallet_prelude::*;
use frame_support::traits::EnsureOrigin;
use frame_support::transactional;
use frame_system::pallet_prelude::*;
pub use module::*;
use orml_traits::asset_registry::AssetProcessor;
use scale_info::TypeInfo;
use sp_runtime::traits::AtLeast32BitUnsigned;
use sp_runtime::{traits::Member, DispatchResult};
use sp_std::prelude::*;
use xcm::v2::prelude::*;
use xcm::VersionedMultiLocation;

pub use impls::*;
pub use weights::WeightInfo;

mod impls;
mod mock;
mod tests;
mod weights;

#[derive(scale_info::TypeInfo, Encode, Decode, Clone, Eq, PartialEq, Debug)]
pub struct AssetMetadata<Balance, CustomMetadata: Parameter + Member + TypeInfo> {
	pub decimals: u32,
	pub name: Vec<u8>,
	pub symbol: Vec<u8>,
	pub existential_deposit: Balance,
	pub location: Option<VersionedMultiLocation>,
	pub additional: CustomMetadata,
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Additional non-standard metadata to store for each asset
		type CustomMetadata: Parameter + Member + TypeInfo;

		/// The type used as a unique asset id,
		type AssetId: Parameter + Member + TypeInfo;

		/// The origin that is allowed to manipulate metadata.
		type AuthorityOrigin: EnsureOrigin<<Self as frame_system::Config>::Origin>;

		/// A filter ran upon metadata registration that assigns an is and
		/// potentially modifies the supplied metadata.
		type ProcessAsset: AssetProcessor<Self::AssetId, AssetMetadata<Self::Balance, Self::CustomMetadata>>;

		/// The balance type.
		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ Into<u128>;

		// /// Weight information for extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Asset was not found
		AssetNotFound,
		/// The version of the `Versioned` value used is not able to be
		/// interpreted.
		BadVersion,
		/// The asset id is invalid.
		InvalidAssetId,
		/// Another asset was already register with this location.
		ConflictingLocation,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		RegisteredAsset {
			asset_id: T::AssetId,
			metadata: AssetMetadata<T::Balance, T::CustomMetadata>,
		},
		UpdatedAsset {
			asset_id: T::AssetId,
			metadata: AssetMetadata<T::Balance, T::CustomMetadata>,
		},
		SetLocation {
			asset_id: T::AssetId,
			location: VersionedMultiLocation,
		},
	}

	/// The total issuance of a token type.
	#[pallet::storage]
	#[pallet::getter(fn get_metadata)]
	pub type Metadata<T: Config> =
		StorageMap<_, Twox64Concat, T::AssetId, AssetMetadata<T::Balance, T::CustomMetadata>, OptionQuery>;

	/// The total issuance of a token type.
	#[pallet::storage]
	#[pallet::getter(fn get_asset_id)]
	pub type MultiLocationLookup<T: Config> = StorageMap<_, Twox64Concat, MultiLocation, T::AssetId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn last_asset_id)]
	pub(crate) type LastAssetId<T: Config> = StorageValue<_, T::AssetId, OptionQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		_phantom: PhantomData<T>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				_phantom: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {}
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::WeightInfo::register_asset())]
		#[transactional]
		pub fn register_asset(
			origin: OriginFor<T>,
			metadata: AssetMetadata<T::Balance, T::CustomMetadata>,
			asset_id: Option<T::AssetId>,
		) -> DispatchResult {
			let _ = T::AuthorityOrigin::ensure_origin(origin)?;

			let (asset_id, metadata) = T::ProcessAsset::process_asset(asset_id, metadata)?;

			Self::insert_metadata(&asset_id, &metadata)?;

			Self::deposit_event(Event::<T>::RegisteredAsset { asset_id, metadata });

			Ok(())
		}

		#[pallet::weight(T::WeightInfo::update_asset())]
		#[transactional]
		pub fn update_asset(
			origin: OriginFor<T>,
			metadata: AssetMetadata<T::Balance, T::CustomMetadata>,
			asset_id: T::AssetId,
		) -> DispatchResult {
			let _ = T::AuthorityOrigin::ensure_origin(origin)?;

			Self::update_metadata(&asset_id, &metadata)?;

			Self::deposit_event(Event::<T>::UpdatedAsset { asset_id, metadata });

			Ok(())
		}

		#[pallet::weight(T::WeightInfo::set_asset_location())]
		#[transactional]
		pub fn set_asset_location(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			location: VersionedMultiLocation,
		) -> DispatchResult {
			let _ = T::AuthorityOrigin::ensure_origin(origin)?;

			let old_metadata = Metadata::<T>::get(&asset_id).ok_or(Error::<T>::AssetNotFound)?;
			let new_metadata = AssetMetadata {
				location: Some(location.clone()),
				..old_metadata
			};
			Self::update_metadata(&asset_id, &new_metadata)?;

			Self::deposit_event(Event::<T>::SetLocation { asset_id, location });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn update_metadata(
		asset_id: &T::AssetId,
		metadata: &AssetMetadata<T::Balance, T::CustomMetadata>,
	) -> DispatchResult {
		let old_metadata = Metadata::<T>::get(&asset_id).ok_or(Error::<T>::AssetNotFound)?;

		if let Some(location) = old_metadata.location {
			// remove the old location lookup
			let location: MultiLocation = location.try_into().map_err(|()| Error::<T>::BadVersion)?;
			MultiLocationLookup::<T>::remove(location);
		}

		Self::insert_metadata(asset_id, metadata)?;

		Ok(())
	}

	pub fn fetch_metadata_by_location(
		location: &MultiLocation,
	) -> Option<AssetMetadata<T::Balance, T::CustomMetadata>> {
		let asset_id = MultiLocationLookup::<T>::get(location)?;
		Metadata::<T>::get(asset_id)
	}

	fn insert_metadata(
		asset_id: &T::AssetId,
		metadata: &AssetMetadata<T::Balance, T::CustomMetadata>,
	) -> DispatchResult {
		// if the metadata contains a location, set the MultiLocationLookup
		if let Some(location) = metadata.location.clone() {
			let location: MultiLocation = location.try_into().map_err(|()| Error::<T>::BadVersion)?;
			ensure!(
				!MultiLocationLookup::<T>::contains_key(&location),
				Error::<T>::ConflictingLocation
			);
			MultiLocationLookup::<T>::insert(location, asset_id);
		}

		Metadata::<T>::insert(&asset_id, &metadata);

		Ok(())
	}
}
