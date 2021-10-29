#![cfg(test)]

use super::*;
use codec::Encode;
use frame_support::{assert_err, assert_noop, assert_ok, traits::Currency};
use orml_traits::{ConcreteFungibleAsset, MultiCurrency};
use polkadot_parachain::primitives::{AccountIdConversion, Sibling};
use sp_runtime::AccountId32;
use xcm_simulator::TestExt;
use xcm_builder::LocationInverter;
use polkadot_runtime_parachains::hrmp::Error::AcceptHrmpChannelLimitExceeded;

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

#[test]
fn send_relay_reserve_to_para_latest1() {
    TestNet::reset();
    let withdraw_amount = 100;
    let fee: u128 = 40;

    Relay::execute_with(|| {
        assert_ok!(RelayChainPalletXcm::reserve_transfer_assets(
				relay::Origin::signed(ALICE),
				Box::new(X1(Parachain(1)).into().into()),
				Box::new(X1(Junction::AccountId32 { network: Any, id: BOB.into() }).into().into()),
				Box::new((Here, withdraw_amount).into()),
				0,
			));

        assert_eq!(RelayBalances::free_balance(&ALICE), INITIAL_BALANCE - withdraw_amount);
        assert_eq!(RelayBalances::free_balance(&para_a_account()), withdraw_amount);
    });

    ParaA::execute_with(|| {
        assert_eq!(ParaTokens::free_balance(CurrencyId::R, &BOB), withdraw_amount - fee);
    });
}

#[test]
fn send_relay_reserve_to_para_latest2() {
    TestNet::reset();
    let withdraw_amount = 500;
    let fee: u128 = 40;

    Relay::execute_with(|| {
        let _ = RelayBalances::deposit_creating(&para_a_account(), INITIAL_BALANCE);

        // Alice on Relaychain reserve transfer token:R to Bob on Parachain(1)
        assert_ok!(RelayChainPalletXcm::reserve_transfer_assets(
				relay::Origin::signed(ALICE),
				Box::new(X1(Parachain(1)).into().into()),
				Box::new(X1(
				    Junction::AccountId32 {
				        network: Any, id: BOB.into()
				    }).into().into()),
				Box::new((Here, withdraw_amount).into()),
				0,
			));

        assert_eq!(RelayBalances::free_balance(&ALICE), INITIAL_BALANCE - withdraw_amount);
        // the balance of Parachain(1) sovereign account is deposited withdraw_amount
        assert_eq!(RelayBalances::free_balance(&para_a_account()), INITIAL_BALANCE + withdraw_amount);
    });

    ParaA::execute_with(|| {
        // the dest account on Parachain(1) is deposited withdraw_amount, and minus fee
        assert_eq!(ParaTokens::free_balance(CurrencyId::R, &BOB), withdraw_amount - fee);
    });
}

#[test]
fn send_relay_reserve_to_para_v0() {
    TestNet::reset();
    // env_logger::init();
    let withdraw_amount = 100;
    let fee: u128 = 40;

    use xcm::v0::*;
    use xcm::v0::Junction::*;
    use xcm::v0::Order::*;
    use xcm::v0::MultiAsset::*;

    Relay::execute_with(|| {
        assert_ok!(RelayChainPalletXcm::reserve_transfer_assets(
				relay::Origin::signed(ALICE),
				Box::new(VersionedMultiLocation::V0(X1(Parachain(1)))),
				Box::new(VersionedMultiLocation::V0(X1(AccountId32 { network: Any, id: BOB.into()}))),
				Box::new(VersionedMultiAssets::V0(vec![ConcreteFungible {
					id: MultiLocation::Null,
					amount: withdraw_amount,
				}])),
				0,
			));

        assert_eq!(RelayBalances::free_balance(&ALICE), INITIAL_BALANCE - withdraw_amount);
        assert_eq!(RelayBalances::free_balance(&para_a_account()), withdraw_amount);
    });

    ParaA::execute_with(|| {
        assert_eq!(ParaTokens::free_balance(CurrencyId::R, &BOB), withdraw_amount - fee);
        assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), INITIAL_BALANCE);
    });
}

#[test]
fn send_para_reserve_to_relay_v0_failed() {
    TestNet::reset();
    // env_logger::init();

    let withdraw_amount = 100;
    let fee: u128 = 40;

    use xcm::v0::*;
    use xcm::v0::Junction::*;
    use xcm::v0::Order::*;
    use xcm::v0::MultiAsset::*;

    ParaA::execute_with(|| {
        assert_ok!(ParachainPalletXcm::reserve_transfer_assets(
				para::Origin::signed(ALICE),
				Box::new(VersionedMultiLocation::V0(X1(Parent))),
				Box::new(VersionedMultiLocation::V0(X1(AccountId32 { network: Any, id: BOB.into()}))),
				Box::new(VersionedMultiAssets::V0(vec![ConcreteFungible {
					id: MultiLocation::X1(Parent),
					amount: withdraw_amount,
				}])),
				0,
			));

        assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), INITIAL_BALANCE - withdraw_amount);
        // assert_eq!(ParaTokens::free_balance(CurrencyId::R, &GOD), withdraw_amount);
    });

    Relay::execute_with(|| {
        assert_eq!(RelayBalances::free_balance(&BOB), 0);
    });
}

#[test]
fn send_para_reserve_to_relay_v1_failed() {
    TestNet::reset();
    // env_logger::init();

    let withdraw_amount = 100;
    let fee: u128 = 40;

    use xcm::v1::*;
    use xcm::v1::Junction::*;
    use xcm::v1::Order::*;

    let asset = MultiAsset {
        id: AssetId::Concrete(xcm::v1::Parent.into()),
        fun: Fungibility::Fungible(withdraw_amount)
    };
    let assets = vec![asset];

    let account = Junctions::X1(AccountId32 { network: Any, id: BOB.into()});
    let location = MultiLocation {
        parents: 0,
        interior: account,
    };

    ParaA::execute_with(|| {
        assert_ok!(ParachainPalletXcm::reserve_transfer_assets(
				para::Origin::signed(ALICE),
				Box::new(VersionedMultiLocation::V1(xcm::v1::Parent.into())),
				Box::new(VersionedMultiLocation::V1(location)),
				Box::new(VersionedMultiAssets::V1(assets.into())),
				0,
			));

        assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), INITIAL_BALANCE - withdraw_amount);
        // assert_eq!(ParaTokens::free_balance(CurrencyId::R, &GOD), withdraw_amount);
    });

    Relay::execute_with(|| {
        assert_eq!(RelayBalances::free_balance(&BOB), 0);
    });
}

#[test]
fn send_para_reserve_to_relay_v1_failed2() {
    TestNet::reset();
    // env_logger::init();

    let withdraw_amount = 100;
    let fee: u128 = 40;

    use xcm::v1::*;
    use xcm::v1::Junction::*;
    use xcm::v1::Order::*;

    let asset = MultiAsset {
        id: AssetId::Concrete(xcm::v1::Parent.into()),
        fun: Fungibility::Fungible(withdraw_amount)
    };
    let assets = vec![asset];

    let account = Junctions::X1(AccountId32 { network: Any, id: CHARLIE.into()});
    let location = MultiLocation {
        parents: 0,
        interior: account,
    };

    ParaA::execute_with(|| {
        assert_ok!(ParaTokens::deposit(CurrencyId::R, &BOB, INITIAL_BALANCE));

        assert_ok!(ParachainPalletXcm::reserve_transfer_assets(
				para::Origin::signed(BOB),
				Box::new(VersionedMultiLocation::V1(xcm::v1::Parent.into())),
				Box::new(VersionedMultiLocation::V1(location)),
				Box::new(VersionedMultiAssets::V1(assets.into())),
				0,
			));

        assert_eq!(ParaTokens::free_balance(CurrencyId::R, &BOB), INITIAL_BALANCE - withdraw_amount);
        assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), INITIAL_BALANCE);
        // assert_eq!(ParaTokens::free_balance(CurrencyId::R, &GOD), withdraw_amount);
        assert_eq!(ParaTokens::free_balance(CurrencyId::R, &CHARLIE), 0);
    });

    Relay::execute_with(|| {
        assert_eq!(RelayBalances::free_balance(&CHARLIE), 0);
    });
}

fn transform_by_currency(currency: CurrencyId, dest: MultiLocation) -> (MultiAsset, MultiLocation, MultiLocation, MultiLocation) {
    let amount = 500u128;
    let asset_fun: Fungibility = Fungibility::Fungible(amount);

    let convert_location = CurrencyIdConvert::convert(currency).unwrap();
    let asset_id: AssetId = AssetId::Concrete(convert_location.clone());

    let asset: MultiAsset = MultiAsset {
        id: asset_id.clone(),
        fun: asset_fun
    };

    let reserve = asset.reserve().unwrap();

    let recipient = dest.non_chain_part().unwrap();
    let dest = dest.chain_part().unwrap();

    // let inv_at = LocationInverter::<mock::para::Ancestry>::invert_location(&dest.clone()).unwrap();
    // let fees = asset.clone().reanchored(&inv_at).unwrap();

    (asset, dest, recipient, reserve)
}

#[test]
fn raw_sibling_asset_to_reserve_sibling_v1() {
    use xcm::v1::Xcm::*;
    use xcm::v1::Order::*;

    // env_logger::init();

    ParaA::execute_with(|| {
        assert_ok!(ParaTokens::deposit(CurrencyId::B, &ALICE, INITIAL_BALANCE));
    });
    ParaB::execute_with(|| {
        assert_ok!(ParaTokens::deposit(CurrencyId::B, &sibling_a_account(), INITIAL_BALANCE));
    });

    // account on dest parachain: ../Parachain(2)/AccountId(Bob)
    let dest: MultiLocation = (
        Parent,
        Parachain(2),
        Junction::AccountId32 {
            network: NetworkId::Any,
            id: BOB.into(),
        },
    ).into();

    let (asset, dest, recipient, reserve) =
        transform_by_currency(CurrencyId::B, dest);
    println!("asset:{:?}", asset);
    println!("dest:{:?}", dest);
    println!("recipient:{:?}", recipient);
    println!("reserve:{:?}", reserve);

    let location = MultiLocation {
        parents: 1,
        interior: Junctions::X2(Parachain(2),  GeneralKey("B".into()))
    };
    let fees = MultiAsset {
        id: AssetId::Concrete(location),
        fun: Fungibility::Fungible(500)
    };

    let buy_execution = BuyExecution {
        fees,
        weight: 40,
        debt: 0,
        halt_on_error: false,
        instructions: vec![]
    };

    ParaA::execute_with(|| {
        let msg = Box::new(VersionedXcm::V1(
            WithdrawAsset {
                assets: asset.clone().into(),
                effects: vec![InitiateReserveWithdraw {
                    assets: All.into(),
                    reserve: dest.clone(),
                    effects: vec![
                        buy_execution,
                        DepositAsset {
                            assets: All.into(),
                            max_assets: 1,
                            beneficiary: recipient,
                        }
                    ],
                }],
            }
        ));
        // assert_ok!(ParachainPalletXcm::execute(para::Origin::signed(ALICE), msg, 0));
        assert_ok!(ParachainPalletXcm::execute(Some(ALICE).into(), msg, 100));

        // assert_eq!(ParaTokens::free_balance(CurrencyId::B, &ALICE), 500)
        println!("alice of B:{}", ParaTokens::free_balance(CurrencyId::B, &ALICE));
    });
}

#[test]
fn send_manual_craft_sibling_asset_to_reserve_sibling_v2() {
    // TestNet::reset();
    // env_logger::init();

    ParaA::execute_with(|| {
        // in the ParaA, Alice should have enough $B
        assert_ok!(ParaTokens::deposit(CurrencyId::B, &ALICE, INITIAL_BALANCE));
        assert_eq!(ParaTokens::free_balance(CurrencyId::B, &ALICE), INITIAL_BALANCE)
    });
    ParaB::execute_with(|| {
        // in the ParaB, the reserve account of ParaA should also have enough $B
        assert_ok!(ParaTokens::deposit(CurrencyId::B, &sibling_a_account(), INITIAL_BALANCE));
        assert_eq!(ParaTokens::free_balance(CurrencyId::B, &sibling_a_account()), INITIAL_BALANCE);
    });

    let dest: MultiLocation = (
        Parent,
        Parachain(2),
        Junction::AccountId32 {
            network: NetworkId::Any,
            id: BOB.into(),
        },
    ).into();

    let (asset, dest, recipient, reserve) =
        transform_by_currency(CurrencyId::B, dest);

    ParaA::execute_with(|| {
        // let inv_at = LocationInverter::<mock::para::Ancestry>::invert_location(&dest.clone()).unwrap();
        // let fees = asset.clone().reanchored(&inv_at).unwrap();
        let msg = Box::new(VersionedXcm::V2(
            Xcm(vec![
                WithdrawAsset(asset.clone().into()),
                InitiateReserveWithdraw {
                    assets: All.into(),
                    reserve: reserve.clone(),
                    xcm: Xcm(vec![
                        // Self::buy_execution(asset, &reserve, dest_weight)?,
                        // Self::deposit_asset(recipient),
                        // BuyExecution {
                        // 	fees,
                        // 	weight_limit: WeightLimit::Limited(40),
                        // },
                        DepositAsset {
                            assets: All.into(),
                            max_assets: 1,
                            beneficiary: recipient,
                        }
                    ]),
                },
            ])
        ));
        // assert_ok!(ParachainPalletXcm::execute(para::Origin::signed(ALICE), msg, 0));
        assert_ok!(ParachainPalletXcm::execute(Some(ALICE).into(), msg, 100));

        // assert_eq!(ParaTokens::free_balance(CurrencyId::B, &ALICE), 500)
        println!("alice of B:{}", ParaTokens::free_balance(CurrencyId::B, &ALICE));
    });

    ParaB::execute_with(|| {
        println!("alice of B:{}", ParaTokens::free_balance(CurrencyId::B, &BOB));
        // assert_eq!(ParaTokens::free_balance(CurrencyId::B, &BOB), 500);
        // assert_eq!(ParaTokens::free_balance(CurrencyId::B, &sibling_a_account()), 500);
    });
}