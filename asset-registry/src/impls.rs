use crate::module::*;
use frame_support::weights::constants::WEIGHT_PER_SECOND;
use frame_support::{log, pallet_prelude::*};
use orml_traits::asset_registry::{AssetProcessor, FixedConversionRateProvider, WeightToFeeConverter};
use orml_traits::GetByKey;
use scale_info::TypeInfo;
use sp_runtime::traits::AtLeast32BitUnsigned;
use sp_runtime::traits::Bounded;
use sp_runtime::traits::Member;
use sp_std::prelude::*;
use xcm::v2::prelude::*;
use xcm_builder::TakeRevenue;
use xcm_executor::{traits::WeightTrader, Assets};

/// An AssetProcessor that assigns a sequential ID
pub struct SequentialId<AssetId, Metadata, T>(PhantomData<(AssetId, Metadata, T)>);

impl<AssetId: AtLeast32BitUnsigned, Metadata, T> AssetProcessor<AssetId, Metadata>
	for SequentialId<AssetId, Metadata, T>
where
	AssetId: AtLeast32BitUnsigned + Parameter + Member + TypeInfo,
	T: Config<AssetId = AssetId>,
{
	fn process_asset(id: Option<AssetId>, asset_metadata: Metadata) -> Result<(AssetId, Metadata), DispatchError> {
		let next_id = match LastAssetId::<T>::get() {
			None => AssetId::zero(),
			Some(x) => x.saturating_add(AssetId::one()),
		};

		match id {
			Some(explicit_id) if explicit_id > next_id => {
				// using a future id would conflict later, so return an error
				Err(Error::<T>::InvalidAssetId.into())
			}
			Some(explicit_id) => Ok((explicit_id, asset_metadata)),
			None => {
				LastAssetId::<T>::put(&next_id);
				Ok((next_id, asset_metadata))
			}
		}
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
					if fee_increase == 0 {
						// if the fee is set very low it lead to zero fees, in which case constructing
						// the fee asset item to subtract from payment would fail. Therefore, provide
						// early exit
						return Ok(payment);
					} else if let Ok(unused) = payment.clone().checked_sub((asset.clone(), fee_increase).into()) {
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

// Return Existential deposit of an asset. Implementing this trait allows the
// pallet to be used in the tokens::ExistentialDeposits config item
impl<T: Config> GetByKey<T::AssetId, T::Balance> for Pallet<T> {
	fn get(k: &T::AssetId) -> T::Balance {
		if let Some(metadata) = Self::get_metadata(k) {
			metadata.existential_deposit
		} else {
			// Asset does not exist - not supported
			T::Balance::max_value()
		}
	}
}
