#![cfg(test)]

use super::para::AccountIdToMultiLocation;
use super::*;
use orml_traits::MultiCurrency;
use xcm_builder::{IsConcrete, LocationInverter};
use xcm_executor::traits::{MatchesFungible, WeightTrader};
use xcm_simulator::TestExt;

use crate::mock::para::RelayLocation;
use crate::mock::relay::KsmLocation;
use xcm_executor::Assets;
use frame_support::parameter_types;

#[test]
fn test_init_balance() {
	Relay::execute_with(|| {
		assert_eq!(RelayBalances::free_balance(&ALICE), INITIAL_BALANCE);
		assert_eq!(RelayBalances::free_balance(&BOB), 0);
		assert_eq!(RelayBalances::free_balance(&para_a_account()), 0);
		assert_eq!(RelayBalances::free_balance(&para_b_account()), 0);
	});

	ParaA::execute_with(|| {
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), INITIAL_BALANCE);
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &BOB), 0);

		assert_eq!(ParaTokens::free_balance(CurrencyId::A, &ALICE), 0);
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &ALICE), 0);

		assert_eq!(ParaBalances::free_balance(&ALICE), 0);
		assert_eq!(ParaBalances::free_balance(&BOB), 0);
		assert_eq!(ParaBalances::free_balance(&sibling_b_account()), 0);
		assert_eq!(ParaBalances::free_balance(&sibling_c_account()), 0);
	});

	ParaB::execute_with(|| {
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), INITIAL_BALANCE);
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &BOB), 0);
	});
}

// this is user case for relaychain transfer to parachain
#[test]
fn test_relay_assets_invert_reanchor() {
	// relay.rs runtime config: Ancestry = Here
	parameter_types! {
		pub Ancestry: MultiLocation = Here.into();
	}

	// in the case of relaychain reserve transfer assets to parachain, the destination is parachain
	let dest: MultiLocation = Parachain(2).into();

	// Here invert Parachain
	let inv_dest = LocationInverter::<Ancestry>::invert_location(&dest).unwrap();

	// the invert destination = Parent
	assert_eq!(inv_dest, (Parent, Here).into());
	assert_eq!(inv_dest, (1, Here).into());
	assert_eq!(inv_dest, Parent.into());

	// in the reserve transfer assets case, the relaychain will reanchor the original assets
	// and use the new reanchored assets as the argument of ReserveAssetDeposited instruction.
	// you can check the TransferReserveAsset instruction processing inside xcm-executor.
	let mut asset: MultiAsset = (Here, 100u128).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, (Parent, 100).into());
}

// this is user case for parachain transfer to relaychain
#[test]
fn test_parachain_invert_parent() {
	parameter_types! {
		pub Ancestry: MultiLocation = X1(Parachain(1)).into();
	}

	let dest: MultiLocation = Parent.into();

	// Parachain(1) invert Parent results: (0, Parachain(1))
	let inv_dest = LocationInverter::<Ancestry>::invert_location(&dest).unwrap();
	assert_eq!(inv_dest, (0, X1(Parachain(1))).into());

	// (Parent, 100).reanchor((0, Parachain(1))) results ((0, Here), 100)
	// A -> R -> R
	// parachain send xcm to dest: Parent(which is relaychain), and the assets is relaychain token
	// so in the relaychain, its assets is (Here, 100), because it's already in relaychain side.
	let mut asset: MultiAsset = (Parent, 100u128).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, ((0, Here), 100).into());

	// (Here, 100).reanchor((0, Parachain(1))) results ((0, Parachain(1)), 100)
	// A -> A -> R, although this is meaningless, because R can never recognize token A
	let mut asset: MultiAsset = (Here, 100u128).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, ((0, Parachain(1)), 100).into());

	// (Parent, 100).reanchor((0, Here)) results ((1, Here), 100)
	// the inv_dest is manual set, not invoked by invert_location
	// (Parent, 100) means relaychain assets, but the inv_dest=(0, Here) means current location
	// the resule of reanchor of assets will not changed. that's assets.reanchor((0, Here)) = assets.
	let inv_dest = (0, Here).into();
	let mut asset: MultiAsset = (Parent, 100u128).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, ((1, Here), 100).into());
	assert_eq!(asset, (Parent, 100).into());
}

#[test]
fn test_parachain_invert_sibling_unexpected() {
	// para.rs runtime config
	parameter_types! {
		pub Ancestry: MultiLocation = X1(Parachain(1)).into();
	}

	// if the original origin is in parachain, and the destination is an sibling parachain
	// then we could imaging this is an xcmp message which parachain send to parachain.
	// but here we may use the wrong destination expression. we should use ../Parachain(2) in other test.
	// because in the perspective side of origin parachain, the way to other parachain must passing parent.
	let dest: MultiLocation = X1(Parachain(2)).into();

	// Parachain(1) invert Parachain(2) results: (1, Here) = Parent
	// this result is un-expected. I thould it will be ../Para(1)
	let inv_dest = LocationInverter::<Ancestry>::invert_location(&dest).unwrap();
	assert_eq!(inv_dest, (Parent, Here).into());
	assert_eq!(inv_dest, Parent.into());

	// (Here, 100).reanchor(Parent) results ((Parent, Here), 100)
	let mut asset: MultiAsset = (Here, 100u128).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, ((Parent, Here), 100).into());
	assert_eq!(asset, ((1, Here), 100).into());
	assert_eq!(asset, (Parent, 100).into());

	// (Parent, 100).reanchor(Parent) results ((2, Here), 100)
	let mut asset: MultiAsset = (Parent, 100u128).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, ((2, Here), 100).into());

	// (Parachain(1), 100).reanchor(Parent) results ((1, Parachain(1)), 100)
	let mut asset: MultiAsset = ((0, Parachain(1)), 100u128).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, ((1, Parachain(1)), 100).into());

	let mut asset: MultiAsset = (Parachain(1), 100u128).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, ((1, Parachain(1)), 100).into());
}

#[test]
fn test_parachain_invert_sibling2() {
	// para.rs runtime config
	parameter_types! {
		pub Ancestry: MultiLocation = X1(Parachain(1)).into();
	}

	// if the original origin is in parachain, and the destination is an sibling parachain
	// then we could imaging this is an xcmp message which parachain send to parachain.
	let dest: MultiLocation = (Parent, Parachain(2)).into();

	// Parachain(1) invert (Parent,Parachain(2)) = (Parent, Parachain(1))
	let inv_dest = LocationInverter::<Ancestry>::invert_location(&dest).unwrap();
	assert_eq!(inv_dest, (1, Parachain(1)).into());

	// (Here, 100).reanchor((Parent,Parachain(1))) results ((Parent, Parachain(1)), 100)
	// this is for the case of A -> [A] -> B
	// (Here,100) means token A, then in para(2), (1, Para(1)) can express the meaning of token A asset
	let mut asset: MultiAsset = (Here, 100u128).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, ((Parent, Parachain(1)), 100).into());
	assert_eq!(asset, ((1, Parachain(1)), 100).into());

	// (Parent, 100).reanchor((Parent,Parachain(1)))
	// this is for the case of A -> R -> B. i.e. Karura transfer KSM to Bifrost.
	// so here (Parent, 100) means 100 KSM in karura, and in bifrost,
	// it also use (Parent, 100) to express 100 KSM.
	let mut asset: MultiAsset = (Parent, 100u128).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, (Parent, 100).into());

	// (Parachain(1), 100).reanchor((Parent,Parachain(1)))
	// it's meaningless, but here we use here just for the testcase
	let mut asset: MultiAsset = ((0, Parachain(1)), 100u128).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, ((1, X2(Parachain(1), Parachain(1))), 100).into());

	// (0, Parachain(1)) is like Parachain(1), so it's also meaningless
	let mut asset: MultiAsset = (Parachain(1), 100u128).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, ((1, X2(Parachain(1), Parachain(1))), 100).into());

	// (GeneralIndex(42), 100).reanchor((Parent,Parachain(1)))
	// the original is GeneralIndex belonging to origin parachain(1)
	// then in the Para(2) side, it needs first get into Para(1), then get into GeneralIndex
	let mut asset: MultiAsset = ((0, GeneralIndex(42)), 100u128).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, ((1, X2(Parachain(1), GeneralIndex(42))), 100).into());
}

#[test]
fn test_invert_reanchor_others() {
	// (Parent, 100).reanchor((1, Parachain(1))) results ((1, Here), 100)
	let inv_dest = (1, X1(Parachain(1))).into();
	let mut asset: MultiAsset = (Parent, 100u128).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, ((1, Here), 100).into());
	assert_eq!(asset, (Parent, 100).into());

	// (Parent, 100).reanchor((2, Parachain(1))) results ((2, Here), 100)
	let inv_dest = (2, X1(Parachain(1))).into();
	let mut asset: MultiAsset = (Parent, 100u128).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, ((2, Here), 100).into());

	// let mut m = MultiLocation::new(2, X1(PalletInstance(3)));
	// assert_eq!(m.prepend_with(MultiLocation::new(1, X2(Parachain(21), OnlyChild))), Ok(()));
	// assert_eq!(m, MultiLocation::new(1, X1(PalletInstance(3))));
	// ((2, X1(PalletInstance(3))), 100).reanchor(1, X2(Parachain(1), OnlyChild)) =
	let inv_dest = (1, X2(Parachain(1), OnlyChild)).into();
	let mut asset: MultiAsset = ((2, X1(PalletInstance(3))), 100).into();
	asset.reanchor(&inv_dest);
	assert_eq!(asset, ((1, X1(PalletInstance(3))), 100).into());
}

#[test]
fn test_asset_matches_fungible() {
	// use raw way: VersionedMultiAssets -> MultiAssets -> Vec<MultiAsset>
	// `KsmLocation` in `relay.rs` is `Here`
	let assets: VersionedMultiAssets = (Here, 100u128).into();
	let assets: MultiAssets = assets.try_into().unwrap();
	let assets: Vec<MultiAsset> = assets.drain();
	for asset in assets {
		let assets: u128 = IsConcrete::<KsmLocation>::matches_fungible(&asset.clone()).unwrap_or_default();
		assert_eq!(assets, 100u128);
	}

	// use convenient way, `KsmLocation` in `relay.rs` is `Here`
	let asset: MultiAsset = (Here, 100u128).into();
	let amount: u128 = IsConcrete::<KsmLocation>::matches_fungible(&asset.clone()).unwrap_or_default();
	assert_eq!(amount, 100u128);

	// `KsmLocation` in `relay.rs` is `Here`
	let asset: MultiAsset = (X1(Parachain(1)), 100u128).into();
	let assets: u128 = IsConcrete::<KsmLocation>::matches_fungible(&asset.clone()).unwrap_or_default();
	assert_eq!(assets, 0);

	// `RelayLocation` in `para.rs` is `Parent`
	let asset: MultiAsset = (Parent, 100u128).into();
	let assets: u128 = IsConcrete::<RelayLocation>::matches_fungible(&asset.clone()).unwrap_or_default();
	assert_eq!(assets, 100);

	let reserve_location = asset.reserve().unwrap();
	assert_eq!(reserve_location.contains_parents_only(1), true);
	assert_eq!(reserve_location, (Parent, Here).into());
}

#[test]
fn test_account_location_convert() {
	let account = Junction::AccountId32 {
		network: NetworkId::Any,
		id: ALICE.into(),
	};

	let origin_location = AccountIdToMultiLocation::convert(ALICE);
	let junction: Junctions = origin_location.try_into().unwrap();
	assert_eq!(junction, X1(account.clone()));

	let parent: MultiLocation = Parent.into();
	assert_eq!(parent.parents, 1);
	assert_eq!(parent.interior, Here);
	assert_eq!(parent.contains_parents_only(1), true);

	let destination: MultiLocation = MultiLocation::new(1, X2(Parachain(2), account.clone())).into();
	assert_eq!(destination.parents, 1);
	assert_eq!(destination.interior, X2(Parachain(2), account.clone()));

	let destination: MultiLocation = (Parent, Parachain(2), account.clone()).into();
	assert_eq!(destination.parents, 1);
	assert_eq!(destination.interior, X2(Parachain(2), account.clone()));

	let destination: MultiLocation = (Parent, account.clone()).into();
	assert_eq!(destination.parents, 1);
	assert_eq!(destination.interior, X1(account.clone()));

	let destination: MultiLocation = (Parachain(2), account.clone()).into();
	assert_eq!(destination.parents, 0);
	assert_eq!(destination.interior, X2(Parachain(2), account.clone()));

	let junction = X1(account.clone());
	let mut destination: MultiLocation = Parent.into();
	destination.append_with(junction).unwrap();
	assert_eq!(destination.parents, 1);
	assert_eq!(destination.interior, X1(account.clone()));
}

#[test]
fn test_parachain_convert_location_to_account() {
	use xcm_executor::traits::Convert;

	// ParentIsDefault
	let parent: MultiLocation = Parent.into();
	let account = para::LocationToAccountId::convert(parent);
	assert_eq!(account, Ok(DEFAULT));

	// SiblingParachainConvertsVia
	let destination: MultiLocation = (Parent, Parachain(1)).into();
	let account = para::LocationToAccountId::convert(destination);
	assert_eq!(account, Ok(sibling_a_account()));

	let alice = Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	};

	// AccountId32Aliases
	let destination: MultiLocation = (alice.clone()).into();
	let account = para::LocationToAccountId::convert(destination);
	assert_eq!(account, Ok(ALICE));

	// RelaychainAccountId32Aliases
	let destination: MultiLocation = (Parent, alice.clone()).into();
	let account = para::LocationToAccountId::convert(destination);
	assert_eq!(account, Ok(ALICE));

	// Error case 1: ../Parachain/Account
	let destination: MultiLocation = (Parent, Parachain(1), alice.clone()).into();
	let account = para::LocationToAccountId::convert(destination.clone());
	assert_eq!(account, Err(destination));

	// Error case 2: ./Parachain
	let destination: MultiLocation = (Parachain(1),).into();
	let account = para::LocationToAccountId::convert(destination.clone());
	assert_eq!(account, Err(destination));
}

#[test]
fn test_relaychain_convert_location_to_account() {
	use xcm_executor::traits::Convert;

	// ChildParachainConvertsVia
	let destination: MultiLocation = (Parachain(1),).into();
	let account = relay::SovereignAccountOf::convert(destination);
	assert_eq!(account, Ok(para_a_account()));

	let alice = Junction::AccountId32 {
		network: NetworkId::Any,
		id: ALICE.into(),
	};

	let alice_on_dot = Junction::AccountId32 {
		network: NetworkId::Polkadot,
		id: ALICE.into(),
	};

	// AccountId32Aliases
	let destination: MultiLocation = (alice.clone()).into();
	let account = relay::SovereignAccountOf::convert(destination);
	assert_eq!(account, Ok(ALICE));

	// AccountId32Aliases with unknown-network location
	let destination: MultiLocation = (alice_on_dot.clone()).into();
	let account = relay::SovereignAccountOf::convert(destination.clone());
	assert_eq!(account, Err(destination));
}

#[test]
fn test_parachain_convert_origin() {
	use xcm_executor::traits::ConvertOrigin;

	let alice = Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	};
	let alice_any = Junction::AccountId32 {
		network: NetworkId::Any,
		id: ALICE.into(),
	};
	let alice_on_dot = Junction::AccountId32 {
		network: NetworkId::Polkadot,
		id: ALICE.into(),
	};

	// supported destination convert with OriginKind::SovereignAccount
	let supported_sovereign_account_destination: Vec<MultiLocation> = vec![
		// ParentIsDefault: parent default account can be kind of sovereign account
		Parent.into(),
		// SiblingParachainConvertsVia: sibling parachain can be kind of sovereign account
		(Parent, Parachain(1)).into(),
		// AccountId32Aliases: current chain's account can be kind of sovereign account
		(alice.clone()).into(),
		// RelaychainAccountId32Aliases: relaychain's account can be kind of sovereign account(xcm-support feature)
		(Parent, alice.clone()).into(),
		(Parent, alice_any.clone()).into(),
	];

	// unsupported destination convert with OriginKind::SovereignAccount
	let unsupported_sovereign_account_destination: Vec<MultiLocation> = vec![
		// sibling parachain's account can't be kind of sovereign account
		(Parent, Parachain(1), alice.clone()).into(),
		// relaychain's account with unmatched network can't be kind of sovereign account
		(Parent, alice_on_dot.clone()).into(),
	];

	for destination in supported_sovereign_account_destination {
		let origin = para::XcmOriginToCallOrigin::convert_origin(destination, OriginKind::SovereignAccount);
		assert!(origin.is_ok());
	}
	for destination in unsupported_sovereign_account_destination {
		let origin = para::XcmOriginToCallOrigin::convert_origin(destination, OriginKind::SovereignAccount);
		assert!(origin.is_err());
	}

	let supported_native_destination: Vec<MultiLocation> = vec![
		// RelayChainAsNative
		Parent.into(),
		// SiblingParachainAsNative
		(Parent, Parachain(1)).into(),
		// SignedAccountId32AsNative
		(alice.clone()).into(),
	];

	let unsupported_native_destination: Vec<MultiLocation> = vec![
		(Parent, Parachain(1), alice.clone()).into(),
		(Parent, alice.clone()).into(),
	];

	for destination in supported_native_destination {
		let origin = para::XcmOriginToCallOrigin::convert_origin(destination, OriginKind::Native);
		assert!(origin.is_ok());
	}
	for destination in unsupported_native_destination {
		let origin = para::XcmOriginToCallOrigin::convert_origin(destination, OriginKind::Native);
		assert!(origin.is_err());
	}

	// XcmPassthrough
	let destination: MultiLocation = (Parent, Parachain(1), alice.clone()).into();
	let origin = para::XcmOriginToCallOrigin::convert_origin(destination.clone(), OriginKind::Xcm);
	assert!(origin.is_ok());
}

#[test]
fn test_call_weight_info() {
	use frame_support::weights::GetDispatchInfo;
	use para::{Call, Runtime};

	let expect_weight: u64 = 6000;
	let call = Call::System(frame_system::Call::<Runtime>::remark_with_event { remark: vec![1, 2, 3] });

	let weight = call.get_dispatch_info().weight;
	assert_eq!(weight, expect_weight);

	let call = Call::System(frame_system::Call::<Runtime>::remark_with_event { remark: vec![1, 2, 3] });
	let weight = call.get_dispatch_info().weight;
	assert_eq!(weight, expect_weight);

	let call = Call::Balances(pallet_balances::Call::<Runtime>::transfer { dest: BOB, value: 100 });
	let weight = call.get_dispatch_info().weight;
	assert_eq!(weight, 195952000);

	let call_para = Call::Balances(pallet_balances::Call::<Runtime>::transfer { dest: BOB, value: 100 });
	let call_relay = relay::Call::XcmPallet(pallet_xcm::Call::<relay::Runtime>::send {
		dest: Box::new(VersionedMultiLocation::V1(Parachain(2).into())),
		message: Box::new(VersionedXcm::from(Xcm(vec![Transact {
			origin_type: OriginKind::SovereignAccount,
			require_weight_at_most: 1000,
			call: call_para.encode().into(),
		}]))),
	});
	let weight = call_relay.get_dispatch_info().weight;
	assert_eq!(weight, 100000000);
}

#[test]
fn test_parachain_weigher_calculate() {
	use frame_support::weights::GetDispatchInfo;
	use para::{Call, Runtime, XcmConfig};

	let expect_weight: u64 = 195952000;
	let call = Call::Balances(pallet_balances::Call::<Runtime>::transfer { dest: BOB, value: 100 });

	let weight = call.get_dispatch_info().weight;
	assert_eq!(weight, expect_weight);

	let assets: MultiAsset = (Parent, 1).into();

	let instructions = vec![
		WithdrawAsset(assets.clone().into()),
		BuyExecution {
			fees: assets.clone(),
			weight_limit: Limited(1),
		},
		Transact {
			origin_type: OriginKind::SovereignAccount,
			require_weight_at_most: expect_weight,
			call: call.encode().into(),
		},
	];
	let xcm_weight = <XcmConfig as xcm_executor::Config>::Weigher::weight(&mut Xcm(instructions));
	assert_eq!(xcm_weight.unwrap(), expect_weight + 30);

	let instructions = vec![
		DescendOrigin(Junctions::X1(Junction::AccountId32 {
			network: NetworkId::Any,
			id: [0; 32],
		})),
		WithdrawAsset(assets.clone().into()),
		BuyExecution {
			fees: assets,
			weight_limit: Limited(1),
		},
		Transact {
			origin_type: OriginKind::SovereignAccount,
			require_weight_at_most: expect_weight,
			call: call.encode().into(),
		},
	];
	let xcm_weight = <XcmConfig as xcm_executor::Config>::Weigher::weight(&mut Xcm(instructions));
	assert_eq!(xcm_weight.unwrap(), expect_weight + 40);
}

#[test]
fn test_trader() {
	use para::XcmConfig;

	let asset: MultiAsset = (Parent, 1000).into();

	let mut holding = Assets::new();
	holding.subsume(asset.clone());

	let backup = holding.clone();

	let fees: MultiAsset = (Parent, 1000).into();
	let max_fee = holding.try_take(fees.into()).unwrap();

	assert_eq!(holding.is_empty(), true);
	assert_eq!(max_fee, backup);

	let mut trader = para::AllTokensAreCreatedEqualToWeight::new();
	let result = <XcmConfig as xcm_executor::Config>::Trader::buy_weight(&mut trader, 1000, max_fee.clone());
	assert_eq!(result.is_ok(), true);
	assert_eq!(result.unwrap().is_empty(), true);

	let result = <XcmConfig as xcm_executor::Config>::Trader::buy_weight(&mut trader, 2000, max_fee);
	assert_eq!(result.is_err(), true);
}
