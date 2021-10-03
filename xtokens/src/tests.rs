#![cfg(test)]

use super::*;
use codec::Encode;
use cumulus_primitives_core::ParaId;
use frame_support::{assert_err, assert_noop, assert_ok, traits::Currency};
use mock::*;
use orml_traits::{ConcreteFungibleAsset, MultiCurrency};
use polkadot_parachain::primitives::{AccountIdConversion, Sibling};
use sp_runtime::AccountId32;
use xcm_simulator::TestExt;
use crate::mock::para::{AccountIdToMultiLocation, Ancestry};
use xcm::{VersionedXcm, VersionedMultiLocation, VersionedMultiAssets};
use xcm_builder::{LocationInverter, IsConcrete};
use xcm_executor::XcmExecutor;
use crate::mock::relay::KsmLocation;
use xcm_executor::traits::MatchesFungible;
use orml_xcm_support::IsNativeConcrete;
use sp_std::convert::TryInto;

fn para_a_account() -> AccountId32 {
	ParaId::from(1).into_account()
}

fn relay_account() -> AccountId32 {
	ParaId::from(999).into_account()
}

fn para_b_account() -> AccountId32 {
	ParaId::from(2).into_account()
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

#[test]
fn relay_chain_cant_use_xtokens() {
	TestNet::reset();

	Relay::execute_with(|| {
		// 如果这里注释掉，即使后面的 ParaA: ParaXTokens::transfer 能够成功，并且 Alice 也扣钱了
		// 但是在中继链侧， BOB 和 para_a_account() 都没有更新余额
		let _ = RelayBalances::deposit_creating(&para_a_account(), 1_000);

		// // invalid method
		// ParaXTokens::transfer(
		// 	Some(ALICE).into(),
		// 	CurrencyId::R,
		// 	500,
		// 	Box::new(MultiLocation::new(
		// 		1,
		// 		X1(Junction::AccountId32 {
		// 			network: NetworkId::Any,
		// 			id: BOB.into(),
		// 		})
		// 	)),
		// 	30,
		// );
		//
		// // no effect
		// assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), 0);
	});

	// valid method on parachain
	ParaA::execute_with(|| {
		assert_ok!(ParaXTokens::transfer(
			Some(ALICE).into(),
			CurrencyId::R,
			500,
			Box::new(MultiLocation::new(
				1,
				X1(Junction::AccountId32 {
					network: NetworkId::Any,
					id: BOB.into(),
				})
			)),
			30,
		));
		// has effect
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), 500);
	});

	Relay::execute_with(|| {
		println!("{}", RelayBalances::free_balance(&para_a_account()));
		println!("{}", RelayBalances::free_balance(&BOB));

		// 由于中继链并没有集成orml_tokens，所以这里查询余额为0，但并不能说明Bob在中继链上没有钱
		// ParaTokens在平行链上调用获取余额才是有效的，同样ParaXTokens::transfer也需要在平行链上调用
		println!("{}", ParaTokens::free_balance(CurrencyId::R, &BOB));
		println!("{}", ParaTokens::free_balance(CurrencyId::R, &ALICE));
	});
}

#[test]
fn send_relay_chain_asset_to_para_chain_reserve_transfer_assets() {
	TestNet::reset();
	let withdraw_amount = 123;

	// 中继链转平行，通过reserve transfer asset，不需要通过xtokens
	Relay::execute_with(|| {
		let _ = RelayBalances::deposit_creating(&para_a_account(), INITIAL_BALANCE);

		assert_ok!(RelayChainPalletXcm::reserve_transfer_assets(
				relay::Origin::signed(ALICE),
				Box::new(X1(Parachain(1)).into().into()),
				// Box::new(X1(Junction::AccountId32 { network: Any, id: ALICE.into() }).into().into()),
				Box::new(X1(Junction::AccountId32 { network: Any, id: BOB.into() }).into().into()),
				Box::new((Here, withdraw_amount).into()),
				0,
				0
			));

		// 在 Relay 里通过 ParaBalances 查询 Alice的余额，并不是说明平行链的Alice有余额
		assert_eq!(
			// pallet_balances::Pallet::<relay::Runtime>::free_balance(&ALICE),
			// pallet_balances::Pallet::<para::Runtime>::free_balance(&ALICE),
			// RelayBalances::free_balance(&ALICE),
			ParaBalances::free_balance(&ALICE),
			INITIAL_BALANCE - withdraw_amount
		);
		assert_eq!(
			pallet_balances::Pallet::<relay::Runtime>::free_balance(&para_a_account()),
			// pallet_balances::Pallet::<para::Runtime>::free_balance(&para_a_account()),
			// RelayBalances::free_balance(&para_a_account()),
			// ParaBalances::free_balance(&para_a_account()),
			INITIAL_BALANCE + withdraw_amount
		);

		// 由于中继链直接调用pallet_xcm的reserve_transfer_assets()，没有用到xtokens，所以通过ParaTokens查询余额，Alice的R tokens=0
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), 0);

		// 结论：在中继链里无法使用ParaTokens查询余额，但是可以使用 pallet_balances 查询
	});

	ParaA::execute_with(|| {
		// 平行链目标账户的余额增加了
		// assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), INITIAL_BALANCE + withdraw_amount);
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &BOB), withdraw_amount);
		// 默认初始化时，平行链的Alice账户R币种就有余额了
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), INITIAL_BALANCE);

		// para_a_account 在中继链上查询无效。自己的平行链，不需要记录自己
		// assert_eq!(ParaTokens::free_balance(CurrencyId::R, &para_a_account()), 0);
		// println!("{}", ParaBalances::free_balance(&para_a_account()));

		// println!("{}", ParaBalances::free_balance(&ALICE));
		// println!("{}", pallet_balances::Pallet::<para::Runtime>::free_balance(&ALICE));
		// println!("{}", para::Balances::free_balance(&ALICE));

		// ParaTokens里有钱（币种R，因为Relay转了R给Bob），但是 balances模块(平行链的balance记录的是自己native token的币种：A)中Bob没有钱
		println!("{}", ParaBalances::free_balance(&BOB));
	});
}

// 平行链转账给中继链，转账token为中继链的token
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
			Box::new(MultiLocation::new(
				1,
				X1(Junction::AccountId32 {
					network: NetworkId::Any,
					id: BOB.into(),
				})
			)),
			30,
		));
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), 500);
	});

	Relay::execute_with(|| {
		assert_eq!(RelayBalances::free_balance(&para_a_account()), 500);
		assert_eq!(RelayBalances::free_balance(&BOB), 470);

		// 由于中继链并没有集成orml_tokens，所以这里查询余额为0，但并不能说明Bob在中继链上没有钱
		// ParaTokens在平行链上调用获取余额才是有效的，同样ParaXTokens::transfer也需要在平行链上调用
		println!("{}", ParaTokens::free_balance(CurrencyId::R, &BOB));
		println!("{}", ParaTokens::free_balance(CurrencyId::R, &ALICE));
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
				30,
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
			Box::new(MultiLocation::new(
				1,
				X2(
					Parachain(2),
					Junction::AccountId32 {
						network: NetworkId::Any,
						id: BOB.into(),
					}
				)
			)),
			30,
		));
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &ALICE), 500);
	});

	Relay::execute_with(|| {
		assert_eq!(RelayBalances::free_balance(&para_a_account()), 500);
		assert_eq!(RelayBalances::free_balance(&para_b_account()), 470);
	});

	ParaB::execute_with(|| {
		assert_eq!(ParaTokens::free_balance(CurrencyId::R, &BOB), 440);
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
			30,
		));

		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &ALICE), 500);
	});

	ParaB::execute_with(|| {
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &sibling_a_account()), 500);
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &BOB), 470);
	});
}

// Alice on ParaA transfer 500 $B to Bob on ParaB
#[test]
fn test_A_B_B() {
	ParaA::execute_with(|| {
		// in the ParaA, Alice should have enough $B
		assert_ok!(ParaTokens::deposit(CurrencyId::B, &ALICE, 1_000));
	});
	ParaB::execute_with(|| {
		// in the ParaB, the reserve account of ParaA should also have enough $B
		assert_ok!(ParaTokens::deposit(CurrencyId::B, &sibling_a_account(), 1_000));
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

	// transfer_to_reserve(asset, reserve=dest, recipient, dest_weight)
	// Para(A) - [B] -> Para(B), ToReserve, reserve=dest
	ParaA::execute_with(|| {
		let inv_at = LocationInverter::<Ancestry>::invert_location(&dest.clone());
		let fees = asset.clone().reanchored(&inv_at).unwrap();
		let msg = Box::new(VersionedXcm::V1(
			WithdrawAsset {
				assets: asset.clone().into(),
				effects: vec![InitiateReserveWithdraw {
					assets: All.into(),
					reserve: dest.clone(),
					effects: vec![
						DepositAsset {
							assets: All.into(),
							max_assets: u32::max_value(),
							beneficiary: recipient,
						}
					],
				}],
			}
		));
		// assert_ok!(ParachainPalletXcm::execute(para::Origin::signed(ALICE), msg, 0));
		assert_ok!(ParachainPalletXcm::execute(Some(ALICE).into(), msg, 50));

		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &ALICE), 500)
	});

	ParaB::execute_with(|| {
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &BOB), 500);
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &sibling_a_account()), 500);
	});
}

#[test]
fn test_asset_transform() {
	let dest: MultiLocation = (
		Parent,
		Parachain(2),
		Junction::AccountId32 {
			network: NetworkId::Any,
			id: BOB.into(),
		},
	).into();
	transform_by_currency(CurrencyId::B, dest.clone());
	println!("==================");
	transform_by_currency(CurrencyId::R, dest.clone());
	println!("==================");

	// 上面两个示例，不管用B还是R，通过currency -> MultiLocation -> MultiAsset，在匹配IsConcrete::<KsmLocation>时均无法匹配
	// 因为relay.rs中KsmLocation=Here
}

#[test]
fn test_origin_location_junction() {
	// 在平行链上发起xcmp的话，平行链上的Alice账户，相对于平行链而言，parents=0
	let origin_location = AccountIdToMultiLocation::convert(ALICE);
	// account_loc:MultiLocation { parents: 0, interior: X1(AccountId32 { network: Any, id: [00000000....] }) }
	println!("account_loc:{:?}", origin_location);

	let junction: Junctions = origin_location.try_into().unwrap();
	assert_eq!(junction, X1(Junction::AccountId32 {
		network: NetworkId::Any,
		id: [0u8; 32]
	}));
}

#[test]
fn test_asset_match() {
	// 下面用手动构造MultiAsset的方式，就能够匹配IsConcrete::<KsmLocation>，因为Tuple第一个元素(Here,..)正好等于KsmLocation=Here
	// matches_fungible(asset)的结果是匹配出对应的amout，即Tuple的第二个元素。必须第一个元素和KsmLocation相等，才返回第二个元素
	let asset: MultiAsset = (Here, 100u128).into();
	let assets: u128 = IsConcrete::<KsmLocation>::matches_fungible(&asset.clone()).unwrap_or_default();
	assert_eq!(assets, 100u128);

	// 或者可以通过 VersionedMultiAssets -> MultiAssets -> Vec<MultiAsset> 进行遍历
	let assets: VersionedMultiAssets = (Here, 100u128).into();
	let assets: MultiAssets = assets.try_into().unwrap();
	let assets: Vec<MultiAsset> = assets.drain();
	for asset in assets {
		let assets: u128 = IsConcrete::<KsmLocation>::matches_fungible(&asset.clone()).unwrap_or_default();
		assert_eq!(assets, 100u128);
	}

	// 这里第一个元素不是Here，所以无法匹配到amount
	let asset: MultiAsset = (Junctions::X1(Junction::Parachain(1)), 100u128).into();
	let assets: u128 = IsConcrete::<KsmLocation>::matches_fungible(&asset.clone()).unwrap_or_default();
	assert_eq!(assets, 0);
}

fn transform_by_currency(currency: CurrencyId, dest: MultiLocation) -> (MultiAsset, MultiLocation, MultiLocation, MultiLocation) {
	// let currency = CurrencyId::B;

	// 将金额包装成 Fungibility
	let amount = 500u128;
	let asset_fun: Fungibility = Fungibility::Fungible(amount);

	// 币种转账成 Location，然后 Location 转换为 资产AssetId
	let convert_location = CurrencyIdConvert::convert(currency).unwrap();
	let asset_id: AssetId = AssetId::Concrete(convert_location.clone());

	// 资产Id + 金额 = Asset
	let asset: MultiAsset = MultiAsset {
		id: asset_id.clone(),
		fun: asset_fun
	};
	// loc:MultiLocation { parents: 1, interior: X2(Parachain(2), GeneralKey([66])) }
	// ass:Concrete(MultiLocation { parents: 1, interior: X2(Parachain(2), GeneralKey([66])) })
	println!("convertloc:{:?}", convert_location);
	println!("multiasset:{:?}", asset_id);
	println!("asset0:{:?}", asset.clone());

	// para.rs type AssetTransactor = LocalAssetTransactor, matcher is IsNativeConcrete
	let assets: u128 = IsNativeConcrete::<CurrencyId, CurrencyIdConvert>::matches_fungible(&asset.clone()).unwrap_or_default();
	println!("assets1 match IsNativeConcrete with {:?}: {}", currency, assets);

	// relay.rs: matcher is IsConcrete
	let assets: u128 = IsConcrete::<KsmLocation>::matches_fungible(&asset.clone()).unwrap_or_default();
	println!("assets2 match IsConcrete<KSM>  with {:?}: {}", currency, assets);

	// 资产的保留位置: asset的chain_part
	let reserve = asset.reserve().unwrap();
	// reserve:     MultiLocation { parents: 1, interior: X1(Parachain(2)) }
	println!("reserve:{:?}", reserve);

	// dest -------------------------------------
	//                                                         chain_part | non-chain-part
	// A->B: dst:MultiLocation { parents: 1, interior: X2(Parachain(2), AccountId32 { network: Any, id: [111111....] }) }
	// R->B: dst:MultiLocation { parents: 0, interior: X2(Parachain(2), AccountId32 { network: Any, id: [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1] }) }
	println!("dst:{:?}", dest);

	// 非链的部分，是链下的账户部分；链部分，即到平行链这一层
	let recipient = dest.non_chain_part().unwrap();
	// 注意：dest替换了，不是原来的dest，这里的dest用来做要发送消息到哪条目标链，所以不能携带non-chain-part
	let dest = dest.chain_part().unwrap();
	// chain:       MultiLocation { parents: 1, interior: X1(Parachain(2)) }
	// recip:       MultiLocation { parents: 0, interior: X1(AccountId32 { network: Any, id: [111111....] }) }
	println!("chain_part(dest'):{:?}", dest.clone());
	println!("recip_nochainpart:{:?}", recipient.clone());

	// ----------
	// asset, reserve,dest,recipent 已经足够构造Xcm消息了。
	(asset, dest, recipient, reserve)
}

// Alice on RelayChain transfer 500 $B to Bob on ParaB
// R - [B] -> B
#[test]
fn relaychain_send_parachain_asset_to_parachain() {
	env_logger::init();
	TestNet::reset();

	Relay::execute_with(|| {
		// in the RelayChain, Alice should have enough $B
		// but relay chain didn't have pallet_tokens module
		// assert_ok!(ParaTokens::deposit(CurrencyId::B, &ALICE, 1_000));

		// we can only set Alice balance, but the token is relay chain's native token, not what we want here which is token B
		// let _ = RelayBalances::deposit_creating(&ALICE, 1_000);

	});
	ParaB::execute_with(|| {
		// in the ParaB, the reserve account of ParaA should also have enough $B
		// assert_ok!(ParaTokens::deposit(CurrencyId::B, &sibling_a_account(), 1_000));

		// 在中继链上，无法设置中继链的Alice的B这个币种的余额，所以这里通过在平行链B上设置Alice的B币种的余额
		// 我们姑且认为平行链B的Alice 与 中继链的Alice 是同一个pubkey，在两个不同链上的地址不同
		assert_ok!(ParaTokens::deposit(CurrencyId::B, &ALICE, 1_000));

		assert_ok!(ParaTokens::deposit(CurrencyId::B, &relay_account(), 1_000));

		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &ALICE), 1_000);
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &relay_account()), 1_000);
	});

	// R -> B，从R角度看：
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

	// transfer_to_reserve(asset, reserve=dest, recipient, dest_weight)
	// A - B -> B, ToReserve, reserve=dest
	// R - B -> B, ToReserve, reserve=dest
	Relay::execute_with(|| {
		let inv_at = LocationInverter::<Ancestry>::invert_location(&dest.clone());
		let fees = asset.clone().reanchored(&inv_at).unwrap();

		// A-[B]-B，因为A有B币种的余额（有tokens模块可以设置），所以可以withdraw
		// R-[B]-B，因为R是中继链，无法通过tokens模块设置B币种，所以无需withdraw?
		// AssetNotFound 在中继链withdraw_asset时，会因为Matcher(asset)无法匹配
		let msg = Box::new(VersionedXcm::V1(
			WithdrawAsset {
				assets: asset.clone().into(),
				effects: vec![InitiateReserveWithdraw {
					assets: All.into(),
					reserve: dest.clone(),
					effects: vec![
						DepositAsset {
							assets: All.into(),
							max_assets: u32::max_value(),
							beneficiary: recipient.clone(),
						}
					],
				}],
			}
		));
		// assert_ok!(RelayChainPalletXcm::execute(para::Origin::signed(ALICE), msg, 0));
		// assert_ok!(RelayChainPalletXcm::execute(Some(Here).into(), msg, 50));
		assert_ok!(RelayChainPalletXcm::execute(Some(ALICE).into(), msg, 50));

		// RelayChainPalletXcm::send_xcm(dest.clone().into(), dest.clone(), Xcm::WithdrawAsset {
		// 	assets: All.into(),
		// 	effects: vec![
		// 		DepositAsset {
		// 			assets: All.into(),
		// 			max_assets: u32::max_value(),
		// 			beneficiary: recipient,
		// 		}
		// 	],
		// });

		println!("{}", ParaTokens::free_balance(CurrencyId::B, &ALICE));
	});

	ParaB::execute_with(|| {
		println!("{}", ParaTokens::free_balance(CurrencyId::B, &BOB));
		println!("{}", ParaTokens::free_balance(CurrencyId::B, &relay_account()));
	});
}

#[test]
fn relay_send_transact_remark() {
	Relay::execute_with(|| {
		let call = para::Call::System(frame_system::Call::<para::Runtime>::remark_with_event(
				vec![1, 2, 3]
			));
		// 并不会发生事件！
		// assert_ok!(
		// 	RelayChainPalletXcm::send(
		// 		Some(ALICE).into(),
		// 		Box::new(VersionedMultiLocation::V1(Parachain(1).into())),
		// 		Box::new(VersionedXcm::V1(Transact {
		// 			origin_type: OriginKind::SovereignAccount,
		// 			require_weight_at_most: 100000000000,
		// 			call: call.encode().into()
		// 		})),
		// 	)
		// );
		assert_ok!(
			RelayChainPalletXcm::send_xcm(
				Here,
				Parachain(1).into(),
				Transact {
					origin_type: OriginKind::SovereignAccount,
					require_weight_at_most: 100000000000 as u64,
					call: call.encode().into(),
				},
			)
		);
	});

	ParaA::execute_with(|| {
		use para::{Event, System};
		for ev in System::events() {
			println!("{:?}", ev.event);
		}
		assert!(System::events()
			.iter()
			.any(|r| matches!(r.event, Event::System(frame_system::Event::Remarked(_, _)))));
	});
}

#[test]
fn relay_send_transact_xcm() {
	env_logger::init();

	ParaA::execute_with(|| {
		// assert_ok!(ParaTokens::deposit(CurrencyId::A, &ALICE, 1_000));
		// ParaBalances::deposit_creating(&ALICE, 2_000); // 如果没有[0;32]，则转账失败，因为call的origin=[0;32]
		ParaBalances::deposit_creating(&ALICE1, 2_000); // ALICE1的地址不是[0;32]
		ParaBalances::deposit_creating(&Charlie, 3_000);
		println!("total:{}", ParaBalances::total_issuance());
	});

	Relay::execute_with(|| {
		// 从谁进行扣款？xcm-executor Transact 里：dispatch_origin = Config::OriginConverter::convert_origin(origin, origin_type)
		// 决定了 origin 是什么，以及推导出来的 AccountId，这里是默认的 AccountId，即[0;32]，如果[0;32]没有余额，转账就失败
		let call = para::Call::Balances(
			pallet_balances::Call::<para::Runtime>::transfer(
				BOB,
				500,
			),
		);

		assert_ok!(
			RelayChainPalletXcm::send_xcm(
				Here,
				Parachain(1).into(),
				Transact {
					origin_type: OriginKind::SovereignAccount,
					// origin_type: OriginKind::Xcm,
					require_weight_at_most: 100000000000 as u64,
					call: call.encode().into(),
				},
			)
		);

		println!("alice:{}", RelayBalances::free_balance( &ALICE));
	});

	ParaA::execute_with(|| {
		use para::{Event, System};
		for ev in System::events() {
			println!("{:?}", ev.event);
		}
	});

	ParaA::execute_with(|| {
		// println!("{}", ParaTokens::free_balance(CurrencyId::A, &ALICE));
		// println!("{}", ParaTokens::free_balance(CurrencyId::A, &BOB));

		println!("alice:{}", ParaBalances::free_balance( &ALICE1));
		println!("bob:{}", ParaBalances::free_balance( &BOB));
		println!("charlie:{}", ParaBalances::free_balance( &Charlie));
		println!("total:{}", ParaBalances::total_issuance());
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
			Box::new(MultiLocation::new(
				1,
				X2(
					Parachain(3),
					Junction::AccountId32 {
						network: NetworkId::Any,
						id: BOB.into(),
					}
				)
			),),
			30
		));
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &ALICE), 500);
	});

	// check reserve accounts
	ParaB::execute_with(|| {
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &sibling_a_account()), 500);
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &sibling_c_account()), 470);
	});

	ParaC::execute_with(|| {
		assert_eq!(ParaTokens::free_balance(CurrencyId::B, &BOB), 440);
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
			Box::new(MultiLocation::new(
				1,
				X2(
					Parachain(2),
					Junction::AccountId32 {
						network: NetworkId::Any,
						id: BOB.into(),
					}
				)
			)),
			30,
		));

		assert_eq!(ParaTokens::free_balance(CurrencyId::A, &ALICE), 500);
		assert_eq!(ParaTokens::free_balance(CurrencyId::A, &sibling_b_account()), 500);
	});

	ParaB::execute_with(|| {
		assert_eq!(ParaTokens::free_balance(CurrencyId::A, &BOB), 470);
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
				Box::new(MultiAsset::sibling_parachain_asset(1, "A".into(), 100)),
				Box::new(MultiLocation::new(
					1,
					X2(
						Parachain(1),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into()
						}
					)
				)),
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
				Box::new(MultiAsset::sibling_parachain_asset(1, "A".into(), 100)),
				Box::new(MultiLocation::new(
					0,
					X1(Junction::AccountId32 {
						network: NetworkId::Any,
						id: BOB.into()
					})
				)),
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

		let call = relay::Call::System(frame_system::Call::<relay::Runtime>::remark_with_event(vec![1, 1, 1]));
		let assets: MultiAsset = (Here, 1_000_000_000_000).into();
		assert_ok!(para::OrmlXcm::send_as_sovereign(
			para::Origin::root(),
			Box::new(MultiLocation::parent()),
			Box::new(WithdrawAsset {
				assets: assets.clone().into(),
				effects: vec![Order::BuyExecution {
					fees: assets,
					weight: 10_000_000,
					debt: 10_000_000,
					halt_on_error: true,
					instructions: vec![Transact {
						origin_type: SovereignAccount,
						require_weight_at_most: 1_000_000_000,
						call: call.encode().into(),
					}],
				}]
			})
		));
	});

	Relay::execute_with(|| {
		relay::System::events().iter().any(|r| {
			matches!(
				r.event,
				relay::Event::System(frame_system::Event::<relay::Runtime>::Remarked(_, _))
			)
		});
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

		let call = relay::Call::System(frame_system::Call::<relay::Runtime>::remark_with_event(vec![1, 1, 1]));
		let assets: MultiAsset = (Here, 1_000_000_000_000).into();
		assert_err!(
			para::OrmlXcm::send_as_sovereign(
				para::Origin::signed(ALICE),
				Box::new(MultiLocation::parent()),
				Box::new(WithdrawAsset {
					assets: assets.clone().into(),
					effects: vec![Order::BuyExecution {
						fees: assets,
						weight: 10_000_000,
						debt: 10_000_000,
						halt_on_error: true,
						instructions: vec![Transact {
							origin_type: SovereignAccount,
							require_weight_at_most: 1_000_000_000,
							call: call.encode().into(),
						}],
					}]
				})
			),
			DispatchError::BadOrigin,
		);
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
