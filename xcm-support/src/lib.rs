//! # XCM Support Module.
//!
//! ## Overview
//!
//! The XCM support module provides supporting traits, types and
//! implementations, to support cross-chain message(XCM) integration with ORML
//! modules.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::{ensure, traits::Contains, weights::Weight};

use sp_runtime::traits::{CheckedConversion, Convert};
use sp_std::{convert::TryFrom, marker::PhantomData, prelude::*};

use xcm::latest::prelude::*;
use xcm_executor::traits::{FilterAssetLocation, MatchesFungible, ShouldExecute};

use orml_traits::location::{Parse, Reserve};

pub use currency_adapter::MultiCurrencyAdapter;
use frame_support::pallet_prelude::Get;

mod currency_adapter;

mod tests;

/// A `MatchesFungible` implementation. It matches concrete fungible assets
/// whose `id` could be converted into `CurrencyId`.
pub struct IsNativeConcrete<CurrencyId, CurrencyIdConvert>(PhantomData<(CurrencyId, CurrencyIdConvert)>);
impl<CurrencyId, CurrencyIdConvert, Amount> MatchesFungible<Amount> for IsNativeConcrete<CurrencyId, CurrencyIdConvert>
where
	CurrencyIdConvert: Convert<MultiLocation, Option<CurrencyId>>,
	Amount: TryFrom<u128>,
{
	fn matches_fungible(a: &MultiAsset) -> Option<Amount> {
		if let (Fungible(ref amount), Concrete(ref location)) = (&a.fun, &a.id) {
			if CurrencyIdConvert::convert(location.clone()).is_some() {
				return CheckedConversion::checked_from(*amount);
			}
		}
		None
	}
}

/// A `FilterAssetLocation` implementation. Filters multi native assets whose
/// reserve is same with `origin`.
pub struct MultiNativeAsset;
impl FilterAssetLocation for MultiNativeAsset {
	fn filter_asset_location(asset: &MultiAsset, origin: &MultiLocation) -> bool {
		if let Some(ref reserve) = asset.reserve() {
			if reserve == origin {
				return true;
			}
		}
		false
	}
}

pub struct ParentFilterAsset;
impl FilterAssetLocation for ParentFilterAsset {
	fn filter_asset_location(asset: &MultiAsset, origin: &MultiLocation) -> bool {
		if let Concrete(location) = &asset.id {
			// if asset's reserve is (Parent, Here)
			if location.chain_part() == Some(MultiLocation::parent()) {
				match origin {
					// and origin location's parents = 1, and it's parenet account
					MultiLocation {
						parents: 1,
						interior: X1(AccountId32 { .. }),
					} => return true,
					_ => {}
				}
			}
		}
		false
	}
}

/// Handlers unknown asset deposit and withdraw.
pub trait UnknownAsset {
	/// Deposit unknown asset.
	fn deposit(asset: &MultiAsset, to: &MultiLocation) -> DispatchResult;

	/// Withdraw unknown asset.
	fn withdraw(asset: &MultiAsset, from: &MultiLocation) -> DispatchResult;
}

const NO_UNKNOWN_ASSET_IMPL: &str = "NoUnknownAssetImpl";

impl UnknownAsset for () {
	fn deposit(_asset: &MultiAsset, _to: &MultiLocation) -> DispatchResult {
		Err(DispatchError::Other(NO_UNKNOWN_ASSET_IMPL))
	}
	fn withdraw(_asset: &MultiAsset, _from: &MultiLocation) -> DispatchResult {
		Err(DispatchError::Other(NO_UNKNOWN_ASSET_IMPL))
	}
}

/// Extracts the `AccountId32` from the passed `location` if the network
/// matches.
pub struct RelaychainAccountId32Aliases<Network, AccountId>(PhantomData<(Network, AccountId)>);
impl<Network: Get<NetworkId>, AccountId: From<[u8; 32]> + Into<[u8; 32]> + Clone>
	xcm_executor::traits::Convert<MultiLocation, AccountId> for RelaychainAccountId32Aliases<Network, AccountId>
{
	fn convert(location: MultiLocation) -> Result<AccountId, MultiLocation> {
		let id = match location {
			MultiLocation {
				parents: 1,
				interior: X1(AccountId32 {
					id,
					network: NetworkId::Any,
				}),
			} => id,
			MultiLocation {
				parents: 1,
				interior: X1(AccountId32 { id, network }),
			} if network == Network::get() => id,
			_ => return Err(location),
		};
		Ok(id.into())
	}

	fn reverse(who: AccountId) -> Result<MultiLocation, AccountId> {
		Ok((
			1,
			AccountId32 {
				id: who.into(),
				network: Network::get(),
			},
		)
			.into())
	}
}

pub struct IsParent;
impl Contains<MultiLocation> for IsParent {
	fn contains(l: &MultiLocation) -> bool {
		l.contains_parents_only(1)
	}
}

/// when Relay an XCM message from a given `interior` location, if the given
/// `interior` is not `Here`, the destination will receive a xcm message
/// beginning with `DescendOrigin` as the first instruction. so the xcm message
/// format must match this order:
/// `DescendOrigin`,`WithdrawAsset`,`BuyExecution`,`Transact`.
pub struct AllowEquivalentParentAccountsFrom<T, Network>(PhantomData<(T, Network)>);
impl<T: Contains<MultiLocation>, Network: Get<NetworkId>> ShouldExecute
	for AllowEquivalentParentAccountsFrom<T, Network>
{
	fn should_execute<Call>(
		origin: &MultiLocation,
		message: &mut Xcm<Call>,
		max_weight: Weight,
		_weight_credit: &mut Weight,
	) -> Result<(), ()> {
		ensure!(T::contains(origin), ());
		Ok(())
		// let mut iter = message.0.iter_mut();
		// let i = iter.next().ok_or(())?;
		// match i {
		// 	DescendOrigin(X1(Junction::AccountId32 {
		// 		network: NetworkId::Any,
		// 		..
		// 	})) => (),
		// 	DescendOrigin(X1(Junction::AccountId32 { network, .. })) if network
		// == &Network::get() => (), 	_ => return Err(()),
		// }
		// let i = iter.next().ok_or(())?;
		// match i {
		// 	WithdrawAsset(..) | ReserveAssetDeposited(..) => (),
		// 	_ => return Err(()),
		// }
		// let i = iter.next().ok_or(())?;
		// match i {
		// 	BuyExecution {
		// 		weight_limit: Limited(ref mut weight),
		// 		..
		// 	} if *weight >= max_weight => {
		// 		*weight = max_weight;
		// 		()
		// 	}
		// 	_ => return Err(()),
		// }
		// let i = iter.next().ok_or(())?;
		// match i {
		// 	Transact {
		// 		origin_type: OriginKind::SovereignAccount,
		// 		..
		// 	} => Ok(()),
		// 	WithdrawAsset(..) => Ok(()),
		// 	_ => Err(()),
		// }
	}
}
