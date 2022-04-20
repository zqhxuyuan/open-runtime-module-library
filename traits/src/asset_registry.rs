use frame_support::pallet_prelude::*;
use xcm::latest::prelude::*;

pub trait WeightToFeeConverter {
	fn convert_weight_to_fee(location: &MultiLocation, weight: Weight) -> Option<u128>;
}

pub trait FixedConversionRateProvider {
	fn get_fee_per_second(location: &MultiLocation) -> Option<u128>;
}
