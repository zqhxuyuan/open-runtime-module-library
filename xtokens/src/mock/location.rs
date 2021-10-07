use sp_std::marker::PhantomData;
use xcm::latest::prelude::*;

use xcm::latest::{NetworkId, MultiLocation, Junction};
use frame_support::pallet_prelude::Get;
use xcm_executor::traits::Convert;

/// Extracts the `AccountId32` from the passed `location` if the network matches.
pub struct AccountId32Aliases2<Network, AccountId>(PhantomData<(Network, AccountId)>);
impl<Network: Get<NetworkId>, AccountId: From<[u8; 32]> + Into<[u8; 32]> + Clone>
Convert<MultiLocation, AccountId> for AccountId32Aliases2<Network, AccountId>
{
    fn convert(location: MultiLocation) -> Result<AccountId, MultiLocation> {
        let id = match location {
            MultiLocation {
                parents: 0,
                interior: X1(Junction::AccountId32 { id, network: NetworkId::Any }),
            } => id,
            MultiLocation {
                parents: 1,
                interior: X1(Junction::AccountId32 { id, network: NetworkId::Any }),
            } => id,
            MultiLocation {
                parents: 1,
                interior: X2(Junction::Parachain(para), Junction::AccountId32 { id, network: NetworkId::Any }),
            } => id,
            MultiLocation {
                parents: 0,
                interior: X1(Junction::AccountId32 { id, network }) }
            if network == Network::get() =>
                id,
            _ => return Err(location),
        };
        Ok(id.into())
    }

    fn reverse(who: AccountId) -> Result<MultiLocation, AccountId> {
        Ok(Junction::AccountId32 { id: who.into(), network: Network::get() }.into())
    }
}