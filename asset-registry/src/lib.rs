#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::EnsureOrigin;
use frame_support::weights::constants::WEIGHT_PER_SECOND;
use frame_support::{log, pallet_prelude::*};
use frame_system::pallet_prelude::*;
pub use module::*;
use orml_traits::asset_registry::{FixedConversionRateProvider, WeightToFeeConverter};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, Member},
	DispatchResult,
};
use sp_std::prelude::*;
use xcm::latest::prelude::*;
use xcm_builder::TakeRevenue;
use xcm_executor::{traits::WeightTrader, Assets};

pub trait AssetIdAssigner<AssetId, AssetMetadata> {
	fn assign_id(asset_metadata: &AssetMetadata, last_id: Option<AssetId>) -> AssetId;
}

pub struct SequentialId<AssetId: AtLeast32BitUnsigned>(PhantomData<AssetId>);
impl<AssetId: AtLeast32BitUnsigned, AssetMetadata> AssetIdAssigner<AssetId, AssetMetadata> for SequentialId<AssetId> {
	fn assign_id(_asset_metadata: &AssetMetadata, last_id: Option<AssetId>) -> AssetId {
		match last_id {
			None => AssetId::zero(),
			Some(x) => x.saturating_add(AssetId::one()),
		}
	}
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type AssetMetadata: Parameter + Member + TypeInfo + Into<MultiLocation>;

		type AssetId: Parameter + Member + TypeInfo;

		type AuthorityOrigin: EnsureOrigin<<Self as frame_system::Config>::Origin>;

		type AssignAsset: AssetIdAssigner<Self::AssetId, Self::AssetMetadata>;
		// /// Weight information for extrinsics in this module.
		// type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		RegisteredAsset {
			asset_id: T::AssetId,
			metadata: T::AssetMetadata,
		},
	}

	/// The total issuance of a token type.
	#[pallet::storage]
	#[pallet::getter(fn get_metadata)]
	pub type Metadata<T: Config> = StorageMap<_, Twox64Concat, T::AssetId, T::AssetMetadata, OptionQuery>;

	/// The total issuance of a token type.
	#[pallet::storage]
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
		#[pallet::weight(0)]
		pub fn register_asset(origin: OriginFor<T>, metadata: T::AssetMetadata) -> DispatchResult {
			let _ = T::AuthorityOrigin::ensure_origin(origin)?;

			let location = metadata.clone().into();

			// if the location is already registered, use the existing id. Otherwise, get a
			// new one
			let asset_id = MultiLocationLookup::<T>::get(&location)
				.unwrap_or_else(|| T::AssignAsset::assign_id(&metadata, Self::last_asset_id()));

			Metadata::<T>::insert(&asset_id, &metadata);
			MultiLocationLookup::<T>::insert(location, &asset_id);

			Self::deposit_event(Event::<T>::RegisteredAsset { asset_id, metadata });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn fetch_metadata_by_location(location: &MultiLocation) -> Option<T::AssetMetadata> {
		let asset_id = MultiLocationLookup::<T>::get(location)?;
		Metadata::<T>::get(asset_id)
	}
}

/// A default implementation for WeightToFeeConverter that takes a fixed
/// conversion rate.
pub struct FixedRateAssetRegistryTrader<P: FixedConversionRateProvider>(PhantomData<P>);
impl<P: FixedConversionRateProvider> WeightToFeeConverter for FixedRateAssetRegistryTrader<P> {
	fn convert_weight_to_fee(location: &MultiLocation, weight: Weight) -> Option<u128> {
		let fee_per_second = P::get_fee_per_second(location)?;
		let amount = fee_per_second.saturating_mul(weight as u128) / (WEIGHT_PER_SECOND as u128);
		Some(amount)
	}
}

pub struct BoughtWeight {
	weight: Weight,
	asset_location: MultiLocation,
	amount: u128,
}

/// A WeightTrader implementation that tries to buy weight using a single
/// currency. It tries all assets in `payment` and uses the first asset that can
/// cover the weight. This asset is then "locked in" - later calls to
/// `buy_weight` in the same xcm message only try the same asset.
/// This is because only a single asset can be refunded due to the return type
/// of `refund_weight`. Additional calls to `buy_weight` for the same asset are
/// handled correctly even if the `WeightToFeeConverter` is a non-linear
/// function.
pub struct AssetRegistryTrader<W: WeightToFeeConverter, R: TakeRevenue> {
	bought_weight: Option<BoughtWeight>,
	_phantom: PhantomData<(W, R)>,
}

impl<W: WeightToFeeConverter, R: TakeRevenue> WeightTrader for AssetRegistryTrader<W, R> {
	fn new() -> Self {
		Self {
			bought_weight: None,
			_phantom: Default::default(),
		}
	}

	fn buy_weight(&mut self, weight: Weight, payment: Assets) -> Result<Assets, XcmError> {
		log::trace!(
			target: "xcm::weight",
			"AssetRegistryTrader::buy_weight weight: {:?}, payment: {:?}",
			weight, payment,
		);

		for (asset, _) in payment.fungible.iter() {
			if let AssetId::Concrete(ref location) = asset {
				if matches!(self.bought_weight, Some(ref bought) if &bought.asset_location != location) {
					// we already bought another asset - don't attempt to buy this one since
					// we won't be able to refund it
					continue;
				}

				let (existing_weight, existing_fee) = match self.bought_weight {
					Some(ref x) => (x.weight, x.amount),
					None => (0, 0),
				};

				let new_weight = existing_weight.saturating_add(weight);

				if let Some(amount) = W::convert_weight_to_fee(location, new_weight) {
					let fee_increase = amount.saturating_sub(existing_fee);
					if let Ok(unused) = payment.clone().checked_sub((asset.clone(), fee_increase).into()) {
						self.bought_weight = Some(BoughtWeight {
							amount,
							weight: new_weight,
							asset_location: location.clone(),
						});
						return Ok(unused);
					}
				}
			}
		}
		Err(XcmError::TooExpensive)
	}

	fn refund_weight(&mut self, weight: Weight) -> Option<MultiAsset> {
		log::trace!(target: "xcm::weight", "AssetRegistryTrader::refund_weight weight: {:?}", weight);

		match self.bought_weight {
			Some(ref mut bought) => {
				let new_weight = bought.weight.saturating_sub(weight);
				let new_amount = W::convert_weight_to_fee(&bought.asset_location, new_weight)?;
				let refunded_amount = bought.amount.saturating_sub(new_amount);

				bought.weight = new_weight;
				bought.amount = new_amount;

				Some((AssetId::Concrete(bought.asset_location.clone()), refunded_amount).into())
			}
			None => None, // nothing to refund
		}
	}
}

impl<W: WeightToFeeConverter, R: TakeRevenue> Drop for AssetRegistryTrader<W, R> {
	fn drop(&mut self) {
		if let Some(ref bought) = self.bought_weight {
			R::take_revenue((AssetId::Concrete(bought.asset_location.clone()), bought.amount).into());
		}
	}
}
