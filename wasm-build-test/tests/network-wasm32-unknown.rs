use ckb_network::{
    BlockingFlag, CKBProtocol, DefaultExitHandler, ExitHandler, NetworkController, NetworkService,
    NetworkState, SupportProtocols,
};
use multiaddr::Multiaddr;
use core::future::Future;
use std::sync::Arc;
use wasm_bindgen_test::*;
use std::str::FromStr;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn network_should_work() {
    let resource = ckb_resource::Resource::bundled("specs/mainnet.toml".to_string());
    let spec = ckb_chain_spec::ChainSpec::load_from(&resource).expect("load spec by name");
    let consensus = spec.build_consensus().expect("build consensus");

    let mut config = ckb_app_config::NetworkConfig::default();
    config.max_outbound_peers = 8;
    config.bootnodes = vec![Multiaddr::from_str(
        "/ip4/192.168.64.1/tcp/8115/ws/p2p/QmWzDLD9E5ideU2kcjFmzsmJVYiSiAezQNzrYZmLHEZVmX",
    )
    .unwrap()];
    let network_state =
        Arc::new(NetworkState::from_config(config).expect("Init network state failed"));
    let exit_handler = DefaultExitHandler::default();
    let required_protocol_ids = vec![SupportProtocols::Sync.protocol_id()];

    let handle = WasmHandle;

    NetworkService::new(
        Arc::clone(&network_state),
        vec![],
        required_protocol_ids,
        consensus.identify_name(),
        "ckb-network-wasm32-unknown-test".to_string(),
        exit_handler,
    )
    .start(&handle)
    .expect("Start network service failed");
}

struct WasmHandle;

impl ckb_spawn::Spawn for WasmHandle {
    fn spawn_task<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        wasm_bindgen_futures::spawn_local(future);
    }
}
