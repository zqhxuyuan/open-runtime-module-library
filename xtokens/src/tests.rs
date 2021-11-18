#![cfg(test)]

use super::*;
use frame_support::{assert_err, assert_noop, assert_ok, traits::Currency};
use mock::*;
use orml_traits::{ConcreteFungibleAsset, MultiCurrency};
use xcm_executor::XcmExecutor;
use xcm_simulator::TestExt;

// Not used in any unit tests, but it's super helpful for debugging. Let's
// keep it here.
#[allow(dead_code)]
fn print_events<Runtime: frame_system::Config>(name: &'static str) {
	println!("------ {:?} events -------", name);
	frame_system::Pallet::<Runtime>::events()
		.iter()
		.for_each(|r| println!("> {:?}", r.event));
}

#[test]
fn send_relay_chain_asset_to_relay_chain() {
	TestNet::reset();

	Relay::execute_with(|| {
		let _ = RelayBalances::deposit_creating(&para_a_account(), 1_000);
	});

	ParaA::execute_with(|| {
		assert_ok!(ParaXTokens::transfer(
			Some(ALICE).into(),
			CurrencyId::R,
			500,
			Box::new(
				MultiLocation::new(
					1,
					X1(Junction::AccountId32 {
						network: NetworkId::Any,
						id: BOB.into(),
					})
				)
				.into()
			),
			40,
		));
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), 500);
	});

	Relay::execute_with(|| {
		assert_eq!(RelayBalances::free_balance(&para_a_account()), 500);
		assert_eq!(RelayBalances::free_balance(&BOB), 460);
	});
}

#[test]
fn cannot_lost_fund_on_send_failed() {
	TestNet::reset();

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::A, &ALICE, 1_000));
		assert_noop!(
			ParaXTokens::transfer(
				Some(ALICE).into(),
				CurrencyId::A,
				500,
				Box::new(
					(
						Parent,
						Parachain(100),
						Junction::AccountId32 {
							network: NetworkId::Kusama,
							id: BOB.into(),
						},
					)
						.into()
				),
				40,
			),
			Error::<para::Runtime>::XcmExecutionFailed
		);

		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), 1_000);
	});
}

#[test]
fn send_relay_chain_asset_to_sibling() {
	TestNet::reset();

	Relay::execute_with(|| {
		let _ = RelayBalances::deposit_creating(&para_a_account(), 1000);
	});

	ParaA::execute_with(|| {
		assert_ok!(ParaXTokens::transfer(
			Some(ALICE).into(),
			CurrencyId::R,
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
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), 500);
	});

	Relay::execute_with(|| {
		assert_eq!(RelayBalances::free_balance(&para_a_account()), 500);
		assert_eq!(RelayBalances::free_balance(&para_b_account()), 460);
	});

	ParaB::execute_with(|| {
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &BOB), 420);
	});
}

#[test]
fn send_sibling_asset_to_reserve_sibling() {
	TestNet::reset();

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::B, &ALICE, 1_000));
	});

	ParaB::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::B, &sibling_a_account(), 1_000));
	});

	ParaA::execute_with(|| {
		assert_ok!(ParaXTokens::transfer(
			Some(ALICE).into(),
			CurrencyId::B,
			500,
			Box::new(
				(
					Parent,
					Parachain(2),
					Junction::AccountId32 {
						network: NetworkId::Any,
						id: BOB.into(),
					},
				)
					.into()
			),
			40,
		));

		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &ALICE), 500);
	});

	ParaB::execute_with(|| {
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &sibling_a_account()), 500);
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &BOB), 460);
	});
}

#[test]
fn send_sibling_asset_to_non_reserve_sibling() {
	TestNet::reset();

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::B, &ALICE, 1_000));
	});

	ParaB::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::B, &sibling_a_account(), 1_000));
	});

	ParaA::execute_with(|| {
		assert_ok!(ParaXTokens::transfer(
			Some(ALICE).into(),
			CurrencyId::B,
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
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &ALICE), 500);
	});

	// check reserve accounts
	ParaB::execute_with(|| {
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &sibling_a_account()), 500);
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &sibling_c_account()), 460);
	});

	ParaC::execute_with(|| {
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &BOB), 420);
	});
}

#[test]
fn send_self_parachain_asset_to_sibling() {
	TestNet::reset();

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::A, &ALICE, 1_000));

		assert_ok!(ParaXTokens::transfer(
			Some(ALICE).into(),
			CurrencyId::A,
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

		assert_eq!(ParaTokens::free_balance(CurrencyId::A, &ALICE), 500);
		assert_eq!(ParaTokens::free_balance(CurrencyId::A, &sibling_b_account()), 500);
	});

	ParaB::execute_with(|| {
		assert_eq!(ParaTokens::free_balance(CurrencyId::A, &BOB), 460);
	});
}

#[test]
fn transfer_no_reserve_assets_fails() {
	TestNet::reset();

	ParaA::execute_with(|| {
		assert_noop!(
			ParaXTokens::transfer_multiasset(
				Some(ALICE).into(),
				Box::new((X1(GeneralKey("B".into())).into(), 100).into()),
				Box::new(
					(
						Parent,
						Parachain(2),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into()
						}
					)
						.into()
				),
				50,
			),
			Error::<para::Runtime>::AssetHasNoReserve
		);
	});
}

#[test]
fn transfer_to_self_chain_fails() {
	TestNet::reset();

	ParaA::execute_with(|| {
		assert_noop!(
			ParaXTokens::transfer_multiasset(
				Some(ALICE).into(),
				Box::new(MultiAsset::sibling_parachain_asset(1, "A".into(), 100).into()),
				Box::new(
					MultiLocation::new(
						1,
						X2(
							Parachain(1),
							Junction::AccountId32 {
								network: NetworkId::Any,
								id: BOB.into()
							}
						)
					)
					.into()
				),
				50,
			),
			Error::<para::Runtime>::NotCrossChainTransfer
		);
	});
}

#[test]
fn transfer_to_invalid_dest_fails() {
	TestNet::reset();

	ParaA::execute_with(|| {
		assert_noop!(
			ParaXTokens::transfer_multiasset(
				Some(ALICE).into(),
				Box::new(MultiAsset::sibling_parachain_asset(1, "A".into(), 100).into()),
				Box::new(
					MultiLocation::new(
						0,
						X1(Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into()
						})
					)
					.into()
				),
				50,
			),
			Error::<para::Runtime>::InvalidDest
		);
	});
}

#[test]
fn send_as_sovereign() {
	TestNet::reset();

	Relay::execute_with(|| {
		let _ = RelayBalances::deposit_creating(&para_a_account(), 1_000_000_000_000);
	});

	ParaA::execute_with(|| {
		use xcm::latest::OriginKind::SovereignAccount;

		let call =
			relay::Call::System(frame_system::Call::<relay::Runtime>::remark_with_event { remark: vec![1, 1, 1] });
		let assets: MultiAsset = (Here, 1_000_000_000_000).into();
		assert_ok!(para::OrmlXcm::send_as_sovereign(
			para::Origin::root(),
			Box::new(MultiLocation::parent()),
			Box::new(Xcm(vec![
				WithdrawAsset(assets.clone().into()),
				BuyExecution {
					fees: assets,
					weight_limit: Limited(2_000_000_000)
				},
				Instruction::Transact {
					origin_type: SovereignAccount,
					require_weight_at_most: 1_000_000_000,
					call: call.encode().into(),
				}
			]))
		));
	});

	Relay::execute_with(|| {
		assert!(relay::System::events().iter().any(|r| {
			matches!(
				r.event,
				relay::Event::System(frame_system::Event::<relay::Runtime>::Remarked(_, _))
			)
		}));
	})
}

#[test]
fn send_as_sovereign_fails_if_bad_origin() {
	TestNet::reset();

	Relay::execute_with(|| {
		let _ = RelayBalances::deposit_creating(&para_a_account(), 1_000_000_000_000);
	});

	ParaA::execute_with(|| {
		use xcm::latest::OriginKind::SovereignAccount;

		let call =
			relay::Call::System(frame_system::Call::<relay::Runtime>::remark_with_event { remark: vec![1, 1, 1] });
		let assets: MultiAsset = (Here, 1_000_000_000_000).into();
		assert_err!(
			para::OrmlXcm::send_as_sovereign(
				para::Origin::signed(ALICE),
				Box::new(MultiLocation::parent()),
				Box::new(Xcm(vec![
					WithdrawAsset(assets.clone().into()),
					BuyExecution {
						fees: assets,
						weight_limit: Limited(10_000_000)
					},
					Instruction::Transact {
						origin_type: SovereignAccount,
						require_weight_at_most: 1_000_000_000,
						call: call.encode().into(),
					}
				]))
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn para_transact_to_relay_remark_use_sovereign_account() {
	Relay::execute_with(|| {
		let _ = RelayBalances::deposit_creating(&para_a_account(), 6030);
	});

	ParaA::execute_with(|| {
		parachain_transact_to_relaychian_remark();
	});

	Relay::execute_with(|| {
		use relay::{Event, System};
		assert!(System::events()
			.iter()
			.any(|r| matches!(r.event, Event::System(frame_system::Event::Remarked(_, _)))));
	});
}

#[test]
fn relay_transact_to_para_remark_use_default_sovereign_account() {
	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::R, &DEFAULT, 6030));
	});

	relaychain_transact_to_parachain_remark(Here, 6030);

	ParaA::execute_with(|| {
		use para::{Event, System};
		assert!(System::events()
			.iter()
			.any(|r| matches!(r.event, Event::System(frame_system::Event::Remarked(_, _)))));
	});
}

// (Relay) reserve_transfer_assets
#[test]
fn send_relay_chain_asset_to_para_chain_reserve_transfer_assets_works() {
	// env_logger::init();
	let withdraw_amount = 123;

	// 中继链转平行，通过reserve transfer asset，不需要通过xtokens
	Relay::execute_with(|| {
		let _ = RelayBalances::deposit_creating(&para_a_account(), INITIAL_BALANCE);

		assert_ok!(RelayChainPalletXcm::reserve_transfer_assets(
			relay::Origin::signed(ALICE),
			Box::new(X1(Parachain(1)).into().into()),
			Box::new(
				X1(Junction::AccountId32 {
					network: Any,
					id: BOB.into()
				})
				.into()
				.into()
			),
			Box::new((Here, withdraw_amount).into()),
			0,
		));

		assert_eq!(RelayBalances::free_balance(&ALICE), INITIAL_BALANCE - withdraw_amount);
		assert_eq!(
			RelayBalances::free_balance(&para_a_account()),
			INITIAL_BALANCE + withdraw_amount
		);
	});

	// parachain receiver message format:
	// origin = (1, Here)
	// ReserveAssetDeposited((1, Here), 123) | ClearOrigin | BuyExecution |
	// DepositAsset
	ParaA::execute_with(|| {
		// 平行链目标账户的余额增加了
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &BOB), withdraw_amount - 40);
		// 默认初始化时，平行链的Alice账户R币种就有余额了
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), INITIAL_BALANCE);
	});
}

// (Relay) TransferReserveAsset + { xcm: Transact }
#[test]
fn relay_transact_to_para_remark_use_normal_account_transfer_reserve_asset_bad_origin() {
	// env_logger::init();
	use para::{Call, Runtime};

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::R, &ALICE, 20_000));
		assert_ok!(ParaTokens::deposit(CurrencyId::A, &ALICE, 1000));
		assert_eq!(21_000, ParaTokens::free_balance(CurrencyId::R, &ALICE));
	});

	let alice = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	});

	// transfer asset from alice to bob on parachain(all in same parachain)
	let call = Call::Balances(pallet_balances::Call::<Runtime>::transfer {
		dest: BOB.into(),
		value: 500,
	});
	Relay::execute_with(|| {
		let _ = RelayBalances::deposit_creating(&ALICE, 20_000);

		let xcm = vec![TransferReserveAsset {
			assets: (Here, 10_000).into(),
			dest: Parachain(1).into(),
			xcm: Xcm::<()>(vec![
				BuyExecution {
					fees: (Parent, 10_000).into(), // buy fee using relay chain asset?
					weight_limit: Limited(6050 as u64),
				},
				Transact {
					origin_type: OriginKind::SovereignAccount,
					require_weight_at_most: 6_000 as u64,
					call: call.encode().into(),
				},
				DepositAsset {
					assets: All.into(),
					max_assets: 1,
					beneficiary: (0, alice.clone()).into(),
				},
			]),
		}];
		// RelayChainPalletXcm::send_xcm(alice, Parachain(1).into(), Xcm(xcm),));
		// RelayChainPalletXcm::execute(relay::Origin::signed(ALICE),
		//   Box::new(VersionedXcm::from(Xcm(xcm))), 10000);
		XcmExecutor::<relay::XcmConfig>::execute_xcm_in_credit(alice, Xcm(xcm), 10, 10);
	});

	ParaA::execute_with(|| {
		use para::{Event, System};
		for ev in System::events() {
			println!("{:?}", ev.event);
		}
		// assert_eq!(24950, ParaTokens::free_balance(CurrencyId::R, &ALICE));
	});
}

// (Relay) WithdrawAsset + DepositReserveAsset + { xcm: Transact }
#[test]
fn relay_transact_to_para_remark_use_normal_account_deposit_reserve_asset_bad_origin() {
	// env_logger::init();
	use para::{Call, Runtime};

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::R, &ALICE, 20_000));
		assert_eq!(21_000, ParaTokens::free_balance(CurrencyId::R, &ALICE));
	});

	let alice = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	});

	let call = Call::System(frame_system::Call::<Runtime>::remark_with_event { remark: vec![1, 2, 3] });
	Relay::execute_with(|| {
		let _ = RelayBalances::deposit_creating(&ALICE, 20_000);

		let xcm = vec![
			WithdrawAsset((Here, 10_000).into()),
			DepositReserveAsset {
				assets: All.into(),
				max_assets: 1,
				dest: Parachain(1).into(),
				xcm: Xcm::<()>(vec![
					BuyExecution {
						fees: (Parent, 10_000).into(),
						weight_limit: Limited(6050 as u64),
					},
					Transact {
						origin_type: OriginKind::SovereignAccount,
						require_weight_at_most: 6_000 as u64,
						call: call.encode().into(),
					},
					DepositAsset {
						assets: All.into(),
						max_assets: 1,
						beneficiary: (0, alice.clone()).into(),
					},
				]),
			},
		];
		XcmExecutor::<relay::XcmConfig>::execute_xcm_in_credit(alice, Xcm(xcm), 20, 20);
	});

	ParaA::execute_with(|| {
		use para::{Event, System};
		for ev in System::events() {
			println!("{:?}", ev.event);
		}
		// assert_eq!(24950, ParaTokens::free_balance(CurrencyId::R, &ALICE));
	});
}

// (Relay) TransferReserveAsset + { xcm: WithdrawAsset }
#[test]
fn relay_to_para_buy_withdraw_deposit_bad_origin() {
	// env_logger::init();
	use para::{Call, Runtime};

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::R, &ALICE, 20_000));
		assert_ok!(ParaTokens::deposit(CurrencyId::A, &ALICE, 1000));
		assert_eq!(21_000, ParaTokens::free_balance(CurrencyId::R, &ALICE));
		let _ = ParaBalances::deposit_creating(&ALICE, 1000);
	});

	let alice = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	});
	let bob = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: BOB.into(),
	});

	Relay::execute_with(|| {
		let _ = RelayBalances::deposit_creating(&ALICE, 20_000);
		assert_eq!(21_000, RelayBalances::free_balance(&ALICE));

		let xcm = vec![TransferReserveAsset {
			assets: (Here, 10_000).into(),
			dest: Parachain(1).into(),
			xcm: Xcm::<()>(vec![
				BuyExecution {
					fees: (Parent, 10_000).into(), // use relaychain asset as fee payment
					weight_limit: Limited(6050 as u64),
				},
				// Transact {call: transfer Alice on parachain to Bob on parachain}
				WithdrawAsset((Here, 500).into()), // withdraw Alice on parachain
				// BuyExecution may add here too
				DepositAsset {
					assets: All.into(),
					max_assets: 1,
					beneficiary: (0, bob).into(), // deposit to Bob on parachain
				},
			]),
		}];
		// RelayChainPalletXcm::send_xcm(alice, Parachain(1).into(), Xcm(xcm),));
		// RelayChainPalletXcm::execute(relay::Origin::signed(ALICE),
		//   Box::new(VersionedXcm::from(Xcm(xcm))), 10000);
		XcmExecutor::<relay::XcmConfig>::execute_xcm_in_credit(alice, Xcm(xcm), 10, 10);

		println!("Alice_Relay:{}", RelayBalances::free_balance(&ALICE));
		println!("ParaA_Relay:{}", RelayBalances::free_balance(&para_a_account()));
	});

	ParaA::execute_with(|| {
		use para::{Event, System};
		for ev in System::events() {
			println!("{:?}", ev.event);
		}
		// assert_eq!(24950, ParaTokens::free_balance(CurrencyId::R, &ALICE));
		println!("Alice_Para:{}", ParaBalances::free_balance(&ALICE));
		println!("Bob_Para_A:{}", ParaBalances::free_balance(&BOB));
	});
}

// (Para) ReserveAssetDeposited + Transact + DepositAsset
#[test]
fn relay_transact_to_para_remark_use_normal_account_deposit_reserve_unexpected() {
	// env_logger::init();
	use para::{Call, Runtime};

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::R, &ALICE, 20_000));
		assert_eq!(21_000, ParaTokens::free_balance(CurrencyId::R, &ALICE));
	});

	let alice = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	});

	let call = Call::System(frame_system::Call::<Runtime>::remark_with_event { remark: vec![1, 2, 3] });
	let assets: MultiAsset = (Parent, 10_000).into();
	Relay::execute_with(|| {
		let xcm = vec![
			// WithdrawAsset(assets.clone().into()),
			ReserveAssetDeposited(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Limited(6050 as u64),
			},
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: 6_000 as u64,
				call: call.encode().into(),
			},
			DepositAsset {
				assets: All.into(),
				max_assets: 1,
				beneficiary: {
					// (1, alice.clone()).into()
					(0, alice.clone()).into()
				},
			},
		];
		assert_ok!(RelayChainPalletXcm::send_xcm(alice, Parachain(1).into(), Xcm(xcm),));
	});

	ParaA::execute_with(|| {
		use para::{Event, System};
		for ev in System::events() {
			println!("{:?}", ev.event);
		}
		assert_eq!(24950, ParaTokens::free_balance(CurrencyId::R, &ALICE));
	});
}

// 用平行链上的代币支付手续费
// (Para) WithdrawAsset + Transact + DepositAsset
#[test]
fn relay_transact_to_para_remark_use_normal_account_deposit_A_works() {
	// env_logger::init();
	use para::{Call, Runtime};

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::A, &ALICE, 21_000));
		assert_eq!(21_000, ParaTokens::free_balance(CurrencyId::A, &ALICE));
	});

	let alice = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	});

	let call = Call::System(frame_system::Call::<Runtime>::remark_with_event { remark: vec![1, 2, 3] });

	let assets: MultiAsset = ((Parent, X2(Parachain(1), GeneralKey("A".as_bytes().to_vec()))), 10_000).into();

	Relay::execute_with(|| {
		let xcm = vec![
			WithdrawAsset(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Limited(6050 as u64),
			},
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: 6_000 as u64,
				call: call.encode().into(),
			},
			DepositAsset {
				assets: All.into(),
				max_assets: 1,
				beneficiary: {
					// 因为 WithdrawAsset 被扣除的是平行链上的Alice账户的代币A余额，
					// 所以这里DepositAsset要还回去的也是平行链上的Alice账户的代币A余额
					(0, alice.clone()).into()
				},
			},
		];
		assert_ok!(RelayChainPalletXcm::send_xcm(alice, Parachain(1).into(), Xcm(xcm),));
	});

	ParaA::execute_with(|| {
		use para::{Event, System};
		for ev in System::events() {
			println!("{:?}", ev.event);
		}
		assert!(System::events()
			.iter()
			.any(|r| matches!(r.event, Event::System(frame_system::Event::Remarked(_, _)))));
		assert_eq!(14950, ParaTokens::free_balance(CurrencyId::A, &ALICE));
	});
}

// 用平行链上的中继链代币支付手续费
// (Para) WithdrawAsset + Transact + DepositAsset
#[test]
fn relay_transact_to_para_remark_use_normal_account_deposit_R_works() {
	// env_logger::init();
	use para::{Call, Runtime};

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::R, &ALICE, 20_000));
		assert_eq!(21_000, ParaTokens::free_balance(CurrencyId::R, &ALICE));
	});

	let alice = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	});
	let call = Call::System(frame_system::Call::<Runtime>::remark_with_event { remark: vec![1, 2, 3] });

	// assets = ../ 对应的是 平行链上的 R 代币资产
	let assets: MultiAsset = (Parent, 10_000).into();

	let deposit = true;

	Relay::execute_with(|| {
		let mut xcm = vec![
			WithdrawAsset(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Limited(6050 as u64),
			},
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: 6_000 as u64,
				call: call.encode().into(),
			},
			DepositAsset {
				assets: All.into(),
				max_assets: 1,
				beneficiary: {
					// 因为 WithdrawAsset 被扣除的是平行链上的Alice账户的代币R的余额，
					// 所以这里DepositAsset要还回去的也是平行链上的Alice账户的代币R的余额
					// 注意，在中继链上的Alice账户，并没有增加代币R的余额
					(1, alice.clone()).into()
				},
			},
		];
		assert_ok!(RelayChainPalletXcm::send_xcm(alice, Parachain(1).into(), Xcm(xcm),));
	});

	ParaA::execute_with(|| {
		use para::{Event, System};
		for ev in System::events() {
			println!("{:?}", ev.event);
		}
		assert!(System::events()
			.iter()
			.any(|r| matches!(r.event, Event::System(frame_system::Event::Remarked(_, _)))));
		// 21_000 - 10_000 = 11_000
		// 10_000 - 6050 = 3950
		// 11_000 + 3950 = 14950
		assert_eq!(14950, ParaTokens::free_balance(CurrencyId::R, &ALICE));
	});
}

// (Para) WithdrawAsset + Transact(remark) + DepositAsset
#[test]
fn relay_transact_to_para_remark_use_normal_account_deposit_RA_works() {
	// env_logger::init();
	use para::{Call, Runtime};

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::R, &ALICE, 20_000));
		assert_ok!(ParaTokens::deposit(CurrencyId::A, &ALICE, 21_000));
		assert_eq!(21_000, ParaTokens::free_balance(CurrencyId::R, &ALICE));
		assert_eq!(21_000, ParaTokens::free_balance(CurrencyId::A, &ALICE));
	});

	let alice = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	});

	let call = Call::System(frame_system::Call::<Runtime>::remark_with_event { remark: vec![1, 2, 3] });
	// let call = Call::Balances(pallet_balances::Call::<Runtime>::transfer {
	// 	dest: BOB.into(),
	// 	value: 500,
	// });

	// change this flag to test different case
	// if use relay token as fee payment, that's Alice R account
	// if use parachain token as fee payment, that's Alice A account
	let use_relay_token = false;
	let issue_method_pallet = false;

	let assets: MultiAsset = match use_relay_token {
		true => (Parent, 10_000).into(),
		_ => ((Parent, X2(Parachain(1), GeneralKey("A".as_bytes().to_vec()))), 10_000).into(),
	};
	let dest: MultiLocation = match use_relay_token {
		true =>
		// 因为 WithdrawAsset 被扣除的是平行链上的Alice账户的代币A余额，
		// 所以这里DepositAsset要还回去的也是平行链上的Alice账户的代币A余额
		{
			(0, alice.clone()).into()
		}
		_ =>
		// 因为 WithdrawAsset 被扣除的是平行链上的Alice账户的代币R的余额，
		// 所以这里DepositAsset要还回去的也是平行链上的Alice账户的代币R的余额
		// 注意，在中继链上的Alice账户，并没有增加代币R的余额
		{
			(1, alice.clone()).into()
		}
	};

	Relay::execute_with(|| {
		let xcm = vec![
			WithdrawAsset(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Limited(6050 as u64),
			},
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: 6_000 as u64,
				call: call.encode().into(),
			},
			DepositAsset {
				assets: All.into(),
				max_assets: 1,
				beneficiary: dest,
			},
		];
		/// two way to send xcm to destination
		/// first one is use util method, testcase can use this one for
		/// convenient second one is use pallet method, the user will use this
		/// one in front page
		match issue_method_pallet {
			true => {
				assert_ok!(RelayChainPalletXcm::send(
					relay::Origin::signed(ALICE),
					Box::new(Parachain(1).into().into()),
					Box::new(VersionedXcm::from(Xcm(xcm)))
				));
			}
			_ => {
				assert_ok!(RelayChainPalletXcm::send_xcm(alice, Parachain(1).into(), Xcm(xcm),));
			}
		}
	});

	ParaA::execute_with(|| {
		use para::{Event, System};
		for ev in System::events() {
			println!("{:?}", ev.event);
		}
		assert!(System::events()
			.iter()
			.any(|r| matches!(r.event, Event::System(frame_system::Event::Remarked(_, _)))));
		match use_relay_token {
			true => {
				assert_eq!(14950, ParaTokens::free_balance(CurrencyId::R, &ALICE));
			}
			_ => {
				assert_eq!(14950, ParaTokens::free_balance(CurrencyId::A, &ALICE));
			}
		}
	});
}

// (Para) WithdrawAsset + Transact(transfer) + DepositAsset
#[test]
fn relay_transact_to_para_transfer_use_normal_account_deposit_RA_works() {
	// env_logger::init();
	use para::{Call, Runtime};

	// transfer call
	let tokens_balance_deposit = 1_000_000_000;
	let transact_weight = 195_952_000;
	let fee_payment = 195_952_050;
	let init_para_balance = 1000;
	let transfer_balance = 500;
	let is_transfer_call = true;

	// remark call
	let tokens_balance_deposit = 20_000;
	let transact_weight = 6_000;
	let fee_payment = 6_050;
	let is_transfer_call = false;

	let tokens_balance_total = tokens_balance_deposit + 1000;
	let left_balance_tokens = tokens_balance_total - fee_payment;
	let left_balance_para = init_para_balance - transfer_balance;

	let use_relay_token = true;
	let issue_method_pallet = false;

	let call = if !is_transfer_call {
		Call::System(frame_system::Call::<Runtime>::remark_with_event { remark: vec![1, 2, 3] })
	} else {
		Call::Balances(pallet_balances::Call::<Runtime>::transfer {
			dest: BOB.into(),
			value: transfer_balance,
		})
	};

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::R, &ALICE, tokens_balance_deposit));
		assert_ok!(ParaTokens::deposit(CurrencyId::A, &ALICE, tokens_balance_total));
		let _ = ParaBalances::deposit_creating(&ALICE, init_para_balance);
	});

	let alice = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	});
	let assets: MultiAsset = if use_relay_token {
		(Parent, fee_payment).into()
	} else {
		(
			(Parent, X2(Parachain(1), GeneralKey("A".as_bytes().to_vec()))),
			fee_payment,
		)
			.into()
	};
	let beneficiary: MultiLocation = if use_relay_token {
		(0, alice.clone()).into()
	} else {
		(1, alice.clone()).into()
	};

	Relay::execute_with(|| {
		let xcm = vec![
			WithdrawAsset(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Limited(fee_payment as u64),
			},
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: transact_weight as u64,
				call: call.encode().into(),
			},
			DepositAsset {
				assets: All.into(),
				max_assets: 1,
				beneficiary,
			},
		];
		match issue_method_pallet {
			true => {
				assert_ok!(RelayChainPalletXcm::send(
					relay::Origin::signed(ALICE),
					Box::new(Parachain(1).into().into()),
					Box::new(VersionedXcm::from(Xcm(xcm)))
				));
			}
			_ => {
				assert_ok!(RelayChainPalletXcm::send_xcm(alice, Parachain(1).into(), Xcm(xcm),));
			}
		}
	});

	ParaA::execute_with(|| {
		use para::{Event, System};
		for ev in System::events() {
			println!("{:?}", ev.event);
		}
		if is_transfer_call {
			assert!(System::events()
				.iter()
				.any(|r| matches!(r.event, Event::Balances(pallet_balances::Event::Transfer(_, _, _)))));
		} else {
			assert!(System::events()
				.iter()
				.any(|r| matches!(r.event, Event::System(frame_system::Event::Remarked(_, _)))));
		}
		if use_relay_token {
			assert_eq!(left_balance_tokens, ParaTokens::free_balance(CurrencyId::R, &ALICE));
		} else {
			assert_eq!(left_balance_tokens, ParaTokens::free_balance(CurrencyId::A, &ALICE));
		}
		if is_transfer_call {
			assert_eq!(left_balance_para, ParaBalances::free_balance(&ALICE));
			assert_eq!(left_balance_para, ParaBalances::free_balance(&BOB));
		}
	});
}

// (Para) WithdrawAsset + Transact(remark)
#[test]
fn relay_transact_to_para_remark_use_normal_account_R_works() {
	// env_logger::init();
	use para::{Call, Runtime};

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::R, &ALICE, 20_000));
		assert_eq!(21_000, ParaTokens::free_balance(CurrencyId::R, &ALICE));
	});

	let alice = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	});
	let call = Call::System(frame_system::Call::<Runtime>::remark_with_event { remark: vec![1, 2, 3] });

	// assets = ../ 对应的是 平行链上的 R 代币资产
	let assets: MultiAsset = (Parent, 10_000).into();

	let deposit = true;

	Relay::execute_with(|| {
		let mut xcm = vec![
			WithdrawAsset(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Limited(6050 as u64),
			},
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: 6_000 as u64,
				call: call.encode().into(),
			},
		];
		assert_ok!(RelayChainPalletXcm::send_xcm(alice, Parachain(1).into(), Xcm(xcm),));
	});

	ParaA::execute_with(|| {
		use para::{Event, System};
		for ev in System::events() {
			println!("{:?}", ev.event);
		}
		assert!(System::events()
			.iter()
			.any(|r| matches!(r.event, Event::System(frame_system::Event::Remarked(_, _)))));
		// origin: (1, Here), versioned: ((1, Here), 3960)
		assert!(System::events()
			.iter()
			.any(|r| matches!(r.event, Event::PolkadotXcm(pallet_xcm::Event::AssetsTrapped(_, _, _)))));
		// 21_000 - 10_000 = 11_000
		// 10_000 - 6040 = 3960  交易费用：6040，剩余：3960
		// __11_000 + 3960 = 14960__  实际余额只有 11_000，剩余的 3960 在 AssetTraps 中
		assert_eq!(11_000, ParaTokens::free_balance(CurrencyId::R, &ALICE));
	});
}

// WithdrawAsset(Parent) + WithdrawAsset(Here) + Deposit(1, Here) -> AssetTraps
#[test]
fn relay_to_para_withdraw_two_times_with_different_token_trap1_works() {
	// env_logger::init();
	use para::{Call, Runtime};

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::R, &ALICE, 20_000));
		assert_eq!(21_000, ParaTokens::free_balance(CurrencyId::R, &ALICE));

		let _ = ParaBalances::deposit_creating(&ALICE, 1000);
		assert_ok!(ParaTokens::deposit(CurrencyId::A, &ALICE, 21_000));
	});

	let alice = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	});
	let bob = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: BOB.into(),
	});
	let assets: MultiAsset = (Parent, 10_000).into();
	let assets_para: MultiAsset = ((Parent, X2(Parachain(1), GeneralKey("A".as_bytes().to_vec()))), 500).into();

	Relay::execute_with(|| {
		let mut xcm = vec![
			WithdrawAsset(assets.clone().into()), // withdraw 10_000 R, left 11_000 R
			BuyExecution {
				fees: assets, // 10_000 R - fee: 50 = 9950 R
				weight_limit: Limited(50 as u64),
			},
			WithdrawAsset(assets_para.into()), // withdraw 500 A, left 20500 A on parachain
			DepositAsset {
				assets: All.into(),
				// there're two asset in holding. if using 1, another asset left in AssetTraps
				max_assets: 1,
				beneficiary: {
					(0, bob).into() // 9950 R deposit to Bob
				},
			},
		];
		assert_ok!(RelayChainPalletXcm::send_xcm(alice, Parachain(1).into(), Xcm(xcm),));
	});

	ParaA::execute_with(|| {
		use para::{Event, System};
		for ev in System::events() {
			println!("{:?}", ev.event);
		}
		// origin: (1, Here), versioned: ((1, (Parachain(1), "A")), 500)
		assert!(System::events()
			.iter()
			.any(|r| matches!(r.event, Event::PolkadotXcm(pallet_xcm::Event::AssetsTrapped(_, _, _)))));

		assert_eq!(1000, ParaBalances::free_balance(&ALICE));
		assert_eq!(0, ParaBalances::free_balance(&BOB));
		assert_eq!(20_500, ParaTokens::free_balance(CurrencyId::A, &ALICE));
		assert_eq!(11_000, ParaTokens::free_balance(CurrencyId::R, &ALICE));
		assert_eq!(9950, ParaTokens::free_balance(CurrencyId::R, &BOB));
	});
}

// 用 Transact 时，BuyExecution 的 fees 必须要比 require_weight 多。
// 假设 Transact 的 require_weight = 6050，那么传给 BuyExecution fees:assets
// 也至少需要 6050 而去掉 Transact 之后，weight 大大减少。并且又因为没有执行
// Transact 时的 weight 限制条件， BuyExecution 中指定的 weight limit
// 甚至可以很少。比如 xcm weight = 50, limit 指定 10 也是可以的。
// WithdrawAsset(Parent) + WithdrawAsset(Here) + Deposit(2, Here) -> NoTrap
#[test]
fn relay_to_para_withdraw_two_times_with_different_token_notrap_works() {
	// env_logger::init();
	use para::{Call, Runtime};

	// Alice has 1000 R and 1000 A
	ParaA::execute_with(|| {
		assert_eq!(1_000, ParaTokens::free_balance(CurrencyId::R, &ALICE));

		assert_ok!(ParaTokens::deposit(CurrencyId::A, &ALICE, 1000));
	});

	let alice = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	});
	let bob = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: BOB.into(),
	});
	let assets: MultiAsset = (Parent, 500).into();
	let assets_para: MultiAsset = ((Parent, X2(Parachain(1), GeneralKey("A".as_bytes().to_vec()))), 500).into();

	Relay::execute_with(|| {
		let mut xcm = vec![
			WithdrawAsset(assets.clone().into()), // withdraw 500 R to holding ==> left 500 R -> Alice
			BuyExecution {
				fees: assets, // [holding: 500 R] - [fee: 50] = [holding: 450 R]
				weight_limit: Limited(50 as u64),
			},
			WithdrawAsset(assets_para.into()), // withdraw 500 A to holding ==> left 500 A -> Alice
			DepositAsset {
				assets: All.into(), // all the holding asset will deposit to beneficiary
				max_assets: 2,
				beneficiary: {
					(0, bob).into() // 450 R and 500 A all deposit to Bob
				},
			},
		];
		assert_ok!(RelayChainPalletXcm::send_xcm(alice, Parachain(1).into(), Xcm(xcm),));
	});

	ParaA::execute_with(|| {
		use para::{Event, System};
		for ev in System::events() {
			println!("{:?}", ev.event);
		}

		assert_eq!(500, ParaTokens::free_balance(CurrencyId::A, &ALICE));
		assert_eq!(500, ParaTokens::free_balance(CurrencyId::A, &BOB));
		assert_eq!(500, ParaTokens::free_balance(CurrencyId::R, &ALICE));
		assert_eq!(450, ParaTokens::free_balance(CurrencyId::R, &BOB));
	});
}

// (Para) WithdrawAsset(Here) + BuyExecution(Here) + DepositAsset
#[test]
fn relay_to_para_withdraw_here_buy_deposit_works() {
	// env_logger::init();
	use para::{Call, Runtime};

	// Alice has 1000 A on parachain
	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::A, &ALICE, 1000));
	});

	let alice = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	});
	let bob = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: BOB.into(),
	});
	let assets_para: MultiAsset = ((Parent, X2(Parachain(1), GeneralKey("A".as_bytes().to_vec()))), 500).into();
	Relay::execute_with(|| {
		let mut xcm = vec![
			// withdraw 500 A to holding ==> left 500 A -> Alice on parachain
			WithdrawAsset(assets_para.clone().into()),
			BuyExecution {
				fees: assets_para, // [holding: 500 A] - [fee: 40] = [holding: 460 A]
				weight_limit: Limited(40 as u64),
			},
			DepositAsset {
				assets: All.into(),
				max_assets: 1,
				beneficiary: {
					(0, bob).into() // 490 A on the holding all deposit to Bob on parachain
				},
			},
		];
		assert_ok!(RelayChainPalletXcm::send_xcm(alice, Parachain(1).into(), Xcm(xcm),));
	});

	ParaA::execute_with(|| {
		assert_eq!(500, ParaTokens::free_balance(CurrencyId::A, &ALICE));
		assert_eq!(460, ParaTokens::free_balance(CurrencyId::A, &BOB));
	});
}

#[test]
fn batch_all_relay_withdraw_para_transfer_works() {
	// env_logger::init();
	use para::{Call, Runtime};

	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::A, &ALICE, 1000));
	});

	let alice = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	});
	let bob = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: BOB.into(),
	});

	Relay::execute_with(|| {
		let xcm_relay_call = relay::Call::XcmPallet(pallet_xcm::Call::<relay::Runtime>::execute {
			message: Box::new(VersionedXcm::from(Xcm(vec![
				WithdrawAsset((Here, 500).into()),
				BuyExecution {
					// [holding: 500] - [fee: 30] = [holding: 430]
					fees: (Here, 500).into(),
					weight_limit: Limited(30 as u64),
				},
				DepositAsset {
					assets: All.into(),
					max_assets: 1,
					beneficiary: {
						// 470 on the holding all deposit to Alice(sender) on relaychain
						(0, alice.clone()).into()
					},
				},
			]))),
			max_weight: 1000,
		});

		let xcm_para_call = relay::Call::XcmPallet(pallet_xcm::Call::<relay::Runtime>::send {
			dest: Box::new(VersionedMultiLocation::from(Parachain(1).into())),
			message: Box::new(VersionedXcm::from(Xcm(vec![
				// withdraw 500 A to holding ==> left 500 A -> Alice on parachain
				WithdrawAsset(((Parent, X2(Parachain(1), GeneralKey("A".as_bytes().to_vec()))), 500).into()),
				BuyExecution {
					// [holding: 500 A] - [fee: 40] = [holding: 460 A]
					fees: ((Parent, X2(Parachain(1), GeneralKey("A".as_bytes().to_vec()))), 500).into(),
					weight_limit: Limited(40 as u64),
				},
				DepositAsset {
					assets: All.into(),
					max_assets: 1,
					beneficiary: {
						// 460 A on the holding all deposit to beneficiary on parachain
						(0, bob).into()
					},
				},
			]))),
		});

		assert_ok!(RelayChainUtility::batch_all(
			relay::Origin::signed(ALICE),
			vec![xcm_relay_call, xcm_para_call]
		));
		assert_eq!(970, RelayBalances::free_balance(&ALICE));
	});

	ParaA::execute_with(|| {
		assert_eq!(500, ParaTokens::free_balance(CurrencyId::A, &ALICE));
		assert_eq!(460, ParaTokens::free_balance(CurrencyId::A, &BOB));
	});
}

// (Para) WithdrawAsset + Transact(remark) multi-times test
#[test]
fn relay_transact_to_para_remark_use_normal_account() {
	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::R, &ALICE, 6040));
		assert_eq!(7040, ParaTokens::free_balance(CurrencyId::R, &ALICE));
	});

	let alice = Junctions::X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	});
	relaychain_transact_to_parachain_remark(alice.clone(), 6040);

	ParaA::execute_with(|| {
		use para::{Event, System};
		assert_eq!(1000, ParaTokens::free_balance(CurrencyId::R, &ALICE));
		assert!(System::events()
			.iter()
			.any(|r| matches!(r.event, Event::System(frame_system::Event::Remarked(_, _)))));
		System::reset_events();
	});
	relaychain_transact_to_parachain_remark(alice.clone(), 100);

	ParaA::execute_with(|| {
		use para::{Event, System};
		assert_eq!(900, ParaTokens::free_balance(CurrencyId::R, &ALICE));
		assert_eq!(
			System::events()
				.iter()
				.find(|r| matches!(r.event, Event::System(frame_system::Event::Remarked(_, _)))),
			None
		);
	});
}

// (Para) WithdrawAsset + Transact(transfer) multi-times test
#[test]
fn relay_transact_to_para_transfer_use_normal_account() {
	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::R, &ALICE, 195952040));
		assert_eq!(195953040, ParaTokens::free_balance(CurrencyId::R, &ALICE));
		let _ = ParaBalances::deposit_creating(&ALICE, 1_000);
		assert_eq!(1000, ParaBalances::free_balance(&ALICE));
	});

	let alice = Junctions::X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE.into(),
	});
	relaychain_transact_to_parachain_transfer(alice.clone(), 195952040, 500);

	ParaA::execute_with(|| {
		use para::{Event, System};
		assert_eq!(1000, ParaTokens::free_balance(CurrencyId::R, &ALICE));
		assert_eq!(500, ParaBalances::free_balance(&ALICE));
		assert_eq!(500, ParaBalances::free_balance(&BOB));
		assert!(System::events()
			.iter()
			.any(|r| matches!(r.event, Event::Balances(pallet_balances::Event::Transfer(_, _, _)))));
		System::reset_events();
	});

	relaychain_transact_to_parachain_transfer(alice.clone(), 100, 100);

	ParaA::execute_with(|| {
		use para::{Event, System};
		assert_eq!(900, ParaTokens::free_balance(CurrencyId::R, &ALICE));
		assert_eq!(500, ParaBalances::free_balance(&ALICE));
		assert_eq!(500, ParaBalances::free_balance(&BOB));
		assert_eq!(
			System::events()
				.iter()
				.find(|r| matches!(r.event, Event::Balances(pallet_balances::Event::Transfer(_, _, _)))),
			None
		);
	});
}

#[test]
fn para_transact_to_sibling_remark_use_sovereign_account() {
	ParaB::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::R, &sibling_a_account(), 6030));
	});

	parachain_transact_to_sibling_remark(Here, 6030);

	ParaB::execute_with(|| {
		use para::{Event, System};
		assert_eq!(0, ParaTokens::free_balance(CurrencyId::R, &sibling_a_account()));
		assert!(System::events()
			.iter()
			.any(|r| matches!(r.event, Event::System(frame_system::Event::Remarked(_, _)))));
	});
}

#[test]
fn para_transact_to_sibling_remark_use_account_failed() {
	let alice = Junctions::X1(Junction::AccountId32 {
		network: NetworkId::Any,
		id: ALICE.into(),
	});

	// the origin of `WithdrawAsset` in the context of destination parachain is
	// `(Parent, Parachain(1), Alice)` and it get error when convert by
	// `LocationToAccountId`.
	parachain_transact_to_sibling_remark(alice, 6040);

	ParaB::execute_with(|| {
		use para::{Event, System};
		assert_eq!(
			System::events()
				.iter()
				.find(|r| matches!(r.event, Event::System(frame_system::Event::Remarked(_, _)))),
			None
		);
	});
}

#[test]
fn relay_transact_to_para_unsupport_kind_failed() {
	ParaA::execute_with(|| {
		assert_ok!(ParaTokens::deposit(CurrencyId::R, &DEFAULT, 6040));
	});

	use para::{Call, Runtime};
	let call = Call::System(frame_system::Call::<Runtime>::remark_with_event { remark: vec![1, 2, 3] });
	let assets: MultiAsset = (Parent, 6040).into();
	let alice = Junctions::X1(Junction::AccountId32 {
		network: NetworkId::Any,
		id: ALICE.into(),
	});

	Relay::execute_with(|| {
		let xcm = vec![
			WithdrawAsset(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Limited(6040),
			},
			Transact {
				origin_type: OriginKind::Native,
				require_weight_at_most: 6000 as u64,
				call: call.encode().into(),
			},
		];
		assert_ok!(RelayChainPalletXcm::send_xcm(alice, Parachain(1).into(), Xcm(xcm),));
	});

	ParaA::execute_with(|| {
		use para::{Event, System};
		assert_eq!(
			System::events()
				.iter()
				.find(|r| matches!(r.event, Event::System(frame_system::Event::Remarked(_, _)))),
			None
		);
	});
}

fn relaychain_transact_to_parachain_remark(junctions: Junctions, amount: u128) {
	use para::{Call, Runtime};
	let call = Call::System(frame_system::Call::<Runtime>::remark_with_event { remark: vec![1, 2, 3] });
	let assets: MultiAsset = (Parent, amount).into();

	let limit: u64 = match junctions {
		Here => 6030,
		_ => 6040,
	};

	Relay::execute_with(|| {
		let xcm = vec![
			WithdrawAsset(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Limited(limit),
			},
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: 6000 as u64,
				call: call.encode().into(),
			},
			/* DepositAsset {
			 * 	assets: All.into(),
			 * 	max_assets: 1,
			 * 	beneficiary: {
			 * 		(1).into()
			 * 	}
			 * } */
		];
		assert_ok!(RelayChainPalletXcm::send_xcm(junctions, Parachain(1).into(), Xcm(xcm),));
	});
}

fn relaychain_transact_to_parachain_transfer(junctions: Junctions, amount: u128, transfer_amount: u128) {
	use para::{Call, Runtime};
	let call = Call::Balances(pallet_balances::Call::<Runtime>::transfer {
		dest: BOB,
		value: transfer_amount,
	});
	let assets: MultiAsset = (Parent, amount).into();

	let limit: u64 = match junctions {
		Here => 195952000 + 30,
		_ => 195952000 + 40,
	};

	Relay::execute_with(|| {
		let xcm = vec![
			WithdrawAsset(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Limited(limit),
			},
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: 195952000 as u64,
				call: call.encode().into(),
			},
		];
		assert_ok!(RelayChainPalletXcm::send_xcm(junctions, Parachain(1).into(), Xcm(xcm),));
	});
}

fn parachain_transact_to_relaychian_remark() {
	use relay::{Call, Runtime};
	let call = Call::System(frame_system::Call::<Runtime>::remark_with_event { remark: vec![1, 2, 3] });
	let assets: MultiAsset = (Here, 6030).into();

	assert_ok!(ParachainPalletXcm::send_xcm(
		Here,
		Parent,
		Xcm(vec![
			WithdrawAsset(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Limited(6030)
			},
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: 6000 as u64,
				call: call.encode().into(),
			},
		]),
	));
}

fn parachain_transact_to_sibling_remark(junctions: Junctions, amount: u128) {
	use relay::{Call, Runtime};
	let call = Call::System(frame_system::Call::<Runtime>::remark_with_event { remark: vec![1, 2, 3] });
	let assets: MultiAsset = (Parent, amount).into();
	let limit: u64 = match junctions {
		Here => 6030,
		_ => 6040,
	};

	ParaA::execute_with(|| {
		let xcm = vec![
			WithdrawAsset(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Limited(limit),
			},
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: 6000 as u64,
				call: call.encode().into(),
			},
		];

		assert_ok!(ParachainPalletXcm::send_xcm(
			junctions,
			(Parent, Parachain(2)),
			Xcm(xcm)
		));
	});
}

#[test]
fn call_size_limit() {
	// Ensures Call enum doesn't allocate more than 200 bytes in runtime
	assert!(
		core::mem::size_of::<crate::Call::<crate::tests::para::Runtime>>() <= 200,
		"size of Call is more than 200 bytes: some calls have too big arguments, use Box to \
		reduce the size of Call.
		If the limit is too strong, maybe consider increasing the limit",
	);

	assert!(
		core::mem::size_of::<orml_xcm::Call::<crate::tests::para::Runtime>>() <= 200,
		"size of Call is more than 200 bytes: some calls have too big arguments, use Box to \
		reduce the size of Call.
		If the limit is too strong, maybe consider increasing the limit",
	);
}
