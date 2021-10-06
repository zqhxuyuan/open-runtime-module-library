#![cfg(test)]

use super::*;
use crate as orml_xtokens;

use serde::{Deserialize, Serialize};
use sp_io::TestExternalities;
use sp_runtime::AccountId32;

use xcm_simulator::{decl_test_network, decl_test_parachain, decl_test_relay_chain};

pub mod para;
pub mod relay;
pub mod location;

pub const ALICE: AccountId32 = AccountId32::new([0u8; 32]);
pub const ALICE1: AccountId32 = AccountId32::new([3u8; 32]);
pub const ALICE9: AccountId32 = AccountId32::new([9u8; 32]);
pub const BOB: AccountId32 = AccountId32::new([1u8; 32]);
pub const Charlie: AccountId32 = AccountId32::new([2u8; 32]);
pub const INITIAL_BALANCE: u128 = 1_000;

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	/// Relay chain token.
	R,
	/// Parachain A token.
	A,
	/// Parachain B token.
	B,
}

pub struct CurrencyIdConvert;
// CurrencyId 转换为 MultiLocation，即通过币种算出该币种在哪条链上（中继链、平行链）
impl Convert<CurrencyId, Option<MultiLocation>> for CurrencyIdConvert {
	fn convert(id: CurrencyId) -> Option<MultiLocation> {
		match id {
			// MultiLocation 包括 parents 和 interior:Junctions 字段
			// 但下面并没有直接构造出MultiLocation，而是通过 (..,..,..).into()转换成MultiLocation
			CurrencyId::R => Some(Parent.into()),
			CurrencyId::A => Some((Parent, Parachain(1), GeneralKey("A".into())).into()),
			CurrencyId::B => Some((Parent, Parachain(2), GeneralKey("B".into())).into()),
		}
	}
}
// 通过当前的链，算出这条链的币种
impl Convert<MultiLocation, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(l: MultiLocation) -> Option<CurrencyId> {
		let a: Vec<u8> = "A".into();
		let b: Vec<u8> = "B".into();
		if l == MultiLocation::parent() {
			return Some(CurrencyId::R);
		}
		match l {
			// Parent 的 parents 字段值 = 1
			MultiLocation { parents, interior } if parents == 1 => match interior {
				X2(Parachain(1), GeneralKey(k)) if k == a => Some(CurrencyId::A),
				X2(Parachain(2), GeneralKey(k)) if k == b => Some(CurrencyId::B),
				_ => None,
			},
			_ => None,
		}
	}
}

// MultiAsset 转换为"币种"，那么如何将币种转换为MultiAsset?
impl Convert<MultiAsset, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(a: MultiAsset) -> Option<CurrencyId> {
		if let MultiAsset {
			fun: Fungible(_),
			id: Concrete(id), // AssetId枚举有两种，Concrete的带有MultiLocation
		} = a
		{
			// 通过MultiLocation可以转换成CurrencyId币种信息
			Self::convert(id)
		} else {
			Option::None
		}
	}
}

pub type Balance = u128;
pub type Amount = i128;

decl_test_parachain! {
	pub struct ParaA {
		Runtime = para::Runtime,
		XcmpMessageHandler = para::XcmpQueue,
		DmpMessageHandler = para::DmpQueue,
		new_ext = para_ext(1),
	}
}

decl_test_parachain! {
	pub struct ParaB {
		Runtime = para::Runtime,
		XcmpMessageHandler = para::XcmpQueue,
		DmpMessageHandler = para::DmpQueue,
		new_ext = para_ext(2),
	}
}

decl_test_parachain! {
	pub struct ParaC {
		Runtime = para::Runtime,
		XcmpMessageHandler = para::XcmpQueue,
		DmpMessageHandler = para::DmpQueue,
		new_ext = para_ext(3),
	}
}

decl_test_relay_chain! {
	pub struct Relay {
		Runtime = relay::Runtime,
		XcmConfig = relay::XcmConfig,
		new_ext = relay_ext(),
	}
}

decl_test_network! {
	pub struct TestNet {
		relay_chain = Relay,
		parachains = vec![
			(1, ParaA),
			(2, ParaB),
			(3, ParaC),
		],
	}
}

pub type RelayBalances = pallet_balances::Pallet<relay::Runtime>;
pub type ParaBalances = pallet_balances::Pallet<para::Runtime>;
pub type ParaTokens = orml_tokens::Pallet<para::Runtime>;
pub type ParaXTokens = orml_xtokens::Pallet<para::Runtime>;

pub type RelayChainPalletXcm = pallet_xcm::Pallet<relay::Runtime>;
pub type ParachainPalletXcm = pallet_xcm::Pallet<para::Runtime>;

pub fn para_ext(para_id: u32) -> TestExternalities {
	use para::{Runtime, System};

	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let parachain_info_config = parachain_info::GenesisConfig {
		parachain_id: para_id.into(),
	};
	<parachain_info::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(&parachain_info_config, &mut t)
		.unwrap();

	orml_tokens::GenesisConfig::<Runtime> {
		balances: vec![(ALICE, CurrencyId::R, 1_000)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn relay_ext() -> sp_io::TestExternalities {
	use relay::{Runtime, System};

	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![(ALICE, 1_000)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
