use ckb_shared::shared::Shared;
use ckb_store::ChainStore;
use ckb_sync::Synchronizer;
use ckb_traits::BlockMedianTimeContext;
use jsonrpc_core::Result;
use jsonrpc_derive::rpc;
use jsonrpc_types::{ChainInfo, PeerState};

#[rpc]
pub trait StatsRpc {
    #[rpc(name = "get_blockchain_info")]
    fn get_blockchain_info(&self) -> Result<ChainInfo>;

    #[rpc(name = "get_peers_state")]
    fn get_peers_state(&self) -> Result<Vec<PeerState>>;
}

pub(crate) struct StatsRpcImpl<CS>
where
    CS: ChainStore,
{
    pub shared: Shared<CS>,
    pub synchronizer: Synchronizer<CS>,
}

impl<CS: ChainStore + 'static> StatsRpc for StatsRpcImpl<CS> {
    fn get_blockchain_info(&self) -> Result<ChainInfo> {
        let chain = self.synchronizer.shared.consensus().id.clone();
        let (tip_header, median_time) = {
            let chain_state = self.shared.chain_state().lock();
            let tip_header = chain_state.tip_header().clone();
            let median_time = (&*chain_state)
                .block_median_time(tip_header.number())
                .expect("current block median time should exists");
            (tip_header, median_time)
        };
        let epoch = tip_header.epoch();
        let difficulty = tip_header.difficulty().clone();
        let is_initial_block_download = self.synchronizer.shared.is_initial_block_download();

        Ok(ChainInfo {
            chain,
            median_time: median_time.to_string(),
            epoch: epoch.to_string(),
            difficulty,
            is_initial_block_download,
            warnings: String::new(),
        })
    }

    fn get_peers_state(&self) -> Result<Vec<PeerState>> {
        Ok(self
            .synchronizer
            .peers()
            .blocks_inflight
            .read()
            .iter()
            .map(|(peer, in_flight)| {
                PeerState::new(peer.value(), in_flight.timestamp, in_flight.blocks.len())
            })
            .collect())
    }
}
