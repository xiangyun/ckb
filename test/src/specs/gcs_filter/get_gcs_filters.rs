
use super::gcs_filter_match;
use crate::{Net, Spec, TestProtocol};
use ckb_sync::NetworkProtocol;
use ckb_types::{packed, prelude::*};

pub struct GetGcsFilters;

impl Spec for GetGcsFilters {
    crate::name!("get_gcs_filters");

    crate::setup!(num_nodes: 1, protocols: vec![TestProtocol::sync(), TestProtocol::gcs_filter()]);

    fn run(&self, net: &mut Net) {
        let node = &net.nodes[0];
        let last_block_hash =
            node.generate_blocks(node.consensus().finalization_delay_length() as usize + 3).last().cloned().unwrap();
        let script_hash = node.always_success_script().calc_script_hash();
        net.connect(node);
        let (peer_id, _, _) = net.receive();

        net.send(
            NetworkProtocol::GCSFILTER.into(),
            peer_id,
            packed::GcsFilterMessage::new_builder()
                .set(
                    packed::GetGcsFilters::new_builder()
                        .start_number(0.pack())
                        .stop_hash(last_block_hash.clone())
                        .build(),
                )
                .build()
                .as_bytes(),
        );

        net.should_receive(
            |data| {
                packed::GcsFilterMessage::from_slice(&data)
                    .map(|message| match_filter(message, last_block_hash.clone(), script_hash.clone()))
                    .unwrap_or(false)
            },
            "Node0 should send back GcsFilter message",
        );
    }
}

fn match_filter(message: packed::GcsFilterMessage, block_hash: packed::Byte32, script_hash: packed::Byte32) -> bool {
    if let packed::GcsFilterMessageUnion::GcsFilter(gcs_filter) = message.to_enum() {
        gcs_filter.block_hash() == block_hash && gcs_filter_match(&gcs_filter.filter().raw_data().to_vec(), &[script_hash])
    } else {
        false
    }
}