#![cfg(test)]

use super::*;
use crate as orml_asset_registry;
use crate::tests::para::AssetRegistry;
use crate::tests::para::CustomMetadata;
use crate::tests::para::Origin;
use crate::tests::para::TreasuryAccount;

use frame_support::{assert_noop, assert_ok};
use mock::*;
use orml_traits::MultiCurrency;
use polkadot_parachain::primitives::Sibling;
use sp_runtime::AccountId32;
use xcm_simulator::TestExt;

fn treasury_account() -> AccountId32 {
	TreasuryAccount::get()
}

fn sibling_a_account() -> AccountId32 {
	use sp_runtime::traits::AccountIdConversion;
	Sibling::from(1).into_account()
}

fn sibling_b_account() -> AccountId32 {
	use sp_runtime::traits::AccountIdConversion;
	Sibling::from(2).into_account()
}

fn sibling_c_account() -> AccountId32 {
	use sp_runtime::traits::AccountIdConversion;
	Sibling::from(3).into_account()
}

// Not used in any unit tests, but it's super helpful for debugging. Let's
// keep it here.
#[allow(dead_code)]
fn print_events<Runtime: frame_system::Config>(name: &'static str) {
	println!("------ {:?} events -------", name);
	frame_system::Pallet::<Runtime>::events()
		.iter()
		.for_each(|r| println!("> {:?}", r.event));
}

fn dummy_metadata() -> AssetMetadata<<para::Runtime as orml_asset_registry::Config>::Balance, CustomMetadata> {
	AssetMetadata {
		decimals: 12,
		name: "para A native token".as_bytes().to_vec(),
		symbol: "paraA".as_bytes().to_vec(),
		existential_deposit: 0,
		location: Some(MultiLocation::new(1, X2(Parachain(1), GeneralKey(vec![0]))).into()),
		additional: CustomMetadata {
			fee_per_second: 1_000_000_000_000,
		},
	}
}

#[test]
/// test that the asset registry can be used in xcm transfers
fn send_self_parachain_asset_to_sibling() {
	TestNet::reset();

	let mut metadata = dummy_metadata();

	ParaB::execute_with(|| {
		AssetRegistry::register_asset(Origin::root(), metadata.clone(), None).unwrap();
	});

	ParaA::execute_with(|| {
		metadata.location = Some(MultiLocation::new(0, X1(GeneralKey(vec![0]))).into());
		AssetRegistry::register_asset(Origin::root(), metadata, None).unwrap();

		assert_ok!(ParaTokens::deposit(CurrencyId::RegisteredAsset(0), &ALICE, 1_000));

		assert_ok!(ParaXTokens::transfer(
			Some(ALICE).into(),
			CurrencyId::RegisteredAsset(0),
			500,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(2),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into(),
						}
					)
				)
				.into()
			),
			40,
		));

		assert_eq!(ParaTokens::free_balance(CurrencyId::RegisteredAsset(0), &ALICE), 500);
		assert_eq!(
			ParaTokens::free_balance(CurrencyId::RegisteredAsset(0), &sibling_b_account()),
			500
		);
	});

	ParaB::execute_with(|| {
		assert_eq!(ParaTokens::free_balance(CurrencyId::RegisteredAsset(0), &BOB), 460);
		assert_eq!(
			ParaTokens::free_balance(CurrencyId::RegisteredAsset(0), &treasury_account()),
			40
		);
	});
}

#[test]
/// test that the asset registry can be used in xcm transfers
fn send_sibling_asset_to_non_reserve_sibling() {
	TestNet::reset();

	// send from paraA send paraB's token to paraC

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::RegisteredAsset(0), &ALICE, 1_000));
		AssetRegistry::register_asset(
			Origin::root(),
			AssetMetadata {
				location: Some(MultiLocation::new(1, X2(Parachain(2), GeneralKey(vec![0]))).into()),
				..dummy_metadata()
			},
			None,
		)
		.unwrap();
	});

	ParaB::execute_with(|| {
		assert_ok!(ParaTokens::deposit(
			CurrencyId::RegisteredAsset(0),
			&sibling_a_account(),
			1_000
		));
		AssetRegistry::register_asset(
			Origin::root(),
			AssetMetadata {
				location: Some(MultiLocation::new(0, X1(GeneralKey(vec![0]))).into()),
				..dummy_metadata()
			},
			None,
		)
		.unwrap();
	});

	ParaC::execute_with(|| {
		AssetRegistry::register_asset(
			Origin::root(),
			AssetMetadata {
				location: Some(MultiLocation::new(1, X2(Parachain(2), GeneralKey(vec![0]))).into()),
				..dummy_metadata()
			},
			None,
		)
		.unwrap();
	});

	ParaA::execute_with(|| {
		assert_ok!(ParaXTokens::transfer(
			Some(ALICE).into(),
			CurrencyId::RegisteredAsset(0),
			500,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(3),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into(),
						}
					)
				)
				.into()
			),
			40
		));
		assert_eq!(ParaTokens::free_balance(CurrencyId::RegisteredAsset(0), &ALICE), 500);
	});

	// check reserve accounts
	ParaB::execute_with(|| {
		assert_eq!(
			ParaTokens::free_balance(CurrencyId::RegisteredAsset(0), &sibling_a_account()),
			500
		);
		assert_eq!(
			ParaTokens::free_balance(CurrencyId::RegisteredAsset(0), &sibling_c_account()),
			460
		);
	});

	ParaC::execute_with(|| {
		assert_eq!(ParaTokens::free_balance(CurrencyId::RegisteredAsset(0), &BOB), 420);
	});
}

#[test]
/// tests the SequentialId AssetProcessor
fn test_sequential_id() {
	TestNet::reset();

	ParaA::execute_with(|| {
		let metadata1 = dummy_metadata();

		let metadata2 = AssetMetadata {
			name: "para A native token 2".as_bytes().to_vec(),
			symbol: "paraA2".as_bytes().to_vec(),
			location: Some(MultiLocation::new(1, X2(Parachain(1), GeneralKey(vec![1]))).into()),
			..dummy_metadata()
		};
		AssetRegistry::register_asset(Origin::root(), metadata1.clone(), None).unwrap();
		AssetRegistry::register_asset(Origin::root(), metadata2.clone(), None).unwrap();

		assert_eq!(AssetRegistry::get_metadata(0).unwrap(), metadata1);
		assert_eq!(AssetRegistry::get_metadata(1).unwrap(), metadata2);
	});
}

#[test]
/// tests FixedRateAssetRegistryTrader
fn test_fixed_rate_asset_trader() {
	TestNet::reset();

	let mut metadata = dummy_metadata();

	ParaB::execute_with(|| {
		AssetRegistry::register_asset(Origin::root(), metadata.clone(), None).unwrap();
	});

	ParaA::execute_with(|| {
		let para_a_metadata = AssetMetadata {
			location: Some(MultiLocation::new(0, X1(GeneralKey(vec![0]))).into()),
			..metadata.clone()
		};
		AssetRegistry::register_asset(Origin::root(), para_a_metadata, None).unwrap();

		assert_ok!(ParaTokens::deposit(CurrencyId::RegisteredAsset(0), &ALICE, 1_000));

		assert_ok!(ParaXTokens::transfer(
			Some(ALICE).into(),
			CurrencyId::RegisteredAsset(0),
			500,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(2),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into(),
						}
					)
				)
				.into()
			),
			40,
		));
	});

	let expected_fee = 40;
	let expected_transfer_1_amount = 500 - expected_fee;
	ParaB::execute_with(|| {
		assert_eq!(
			ParaTokens::free_balance(CurrencyId::RegisteredAsset(0), &BOB),
			expected_transfer_1_amount
		);

		assert_eq!(
			ParaTokens::free_balance(CurrencyId::RegisteredAsset(0), &treasury_account()),
			expected_fee
		);

		// now double the fee rate
		metadata.additional.fee_per_second *= 2;
		AssetRegistry::update_asset(Origin::root(), metadata, 0).unwrap();
	});

	ParaA::execute_with(|| {
		assert_ok!(ParaXTokens::transfer(
			Some(ALICE).into(),
			CurrencyId::RegisteredAsset(0),
			500,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(2),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into(),
						}
					)
				)
				.into()
			),
			40,
		));
	});

	let expected_transfer_2_amount = 500 - 2 * expected_fee;

	ParaB::execute_with(|| {
		assert_eq!(
			ParaTokens::free_balance(CurrencyId::RegisteredAsset(0), &BOB),
			expected_transfer_1_amount + expected_transfer_2_amount
		);

		assert_eq!(
			ParaTokens::free_balance(CurrencyId::RegisteredAsset(0), &treasury_account()),
			expected_fee * 3 // 1 for the first transfer, then twice for the second one
		);
	});
}

#[test]
fn test_register_duplicate_location_returns_error() {
	TestNet::reset();

	ParaA::execute_with(|| {
		let metadata = dummy_metadata();

		assert_ok!(AssetRegistry::register_asset(Origin::root(), metadata.clone(), None));
		assert_noop!(
			AssetRegistry::register_asset(Origin::root(), metadata.clone(), None),
			Error::<para::Runtime>::ConflictingLocation
		);
	});
}

#[test]
fn test_update_metadata_works() {
	TestNet::reset();

	ParaA::execute_with(|| {
		let old_metadata = dummy_metadata();
		assert_ok!(AssetRegistry::register_asset(
			Origin::root(),
			old_metadata.clone(),
			None
		));

		let new_metadata = AssetMetadata {
			decimals: 11,
			name: "para A native token2".as_bytes().to_vec(),
			symbol: "paraA2".as_bytes().to_vec(),
			existential_deposit: 1,
			location: Some(MultiLocation::new(1, X2(Parachain(1), GeneralKey(vec![1]))).into()),
			additional: CustomMetadata {
				fee_per_second: 2_000_000_000_000,
			},
		};
		assert_ok!(AssetRegistry::update_asset(Origin::root(), new_metadata.clone(), 0));

		let old_location: MultiLocation = old_metadata.location.clone().unwrap().try_into().unwrap();
		let new_location: MultiLocation = new_metadata.location.clone().unwrap().try_into().unwrap();

		// check that the old location was removed and the new one added
		assert_eq!(AssetRegistry::get_asset_id(old_location), None);
		assert_eq!(AssetRegistry::get_asset_id(new_location), Some(0));

		assert_eq!(AssetRegistry::get_metadata(0).unwrap(), new_metadata);
	});
}
