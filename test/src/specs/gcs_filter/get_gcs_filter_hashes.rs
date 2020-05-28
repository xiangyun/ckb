
use crate::{Net, Spec, TestProtocol};
use ckb_sync::NetworkProtocol;
use ckb_types::{packed, prelude::*};

pub struct GetGcsFilterHashes;

impl Spec for GetGcsFilterHashes {
    crate::name!("get_gcs_filter_hashes");

    crate::setup!(num_nodes: 1, protocols: vec![TestProtocol::sync(), TestProtocol::gcs_filter()]);

    fn run(&self, net: &mut Net) {
        let node = &net.nodes[0];
        let last_block_hash =
            node.generate_blocks(node.consensus().finalization_delay_length() as usize + 3).last().cloned().unwrap();
        net.connect(node);
        let (peer_id, _, _) = net.receive();

        net.send(
            NetworkProtocol::GCSFILTER.into(),
            peer_id,
            packed::GcsFilterMessage::new_builder()
                .set(
                    packed::GetGcsFilterHashes::new_builder()
                        .start_number(1.pack())
                        .stop_hash(last_block_hash.clone())
                        .build(),
                )
                .build()
                .as_bytes(),
        );

        net.should_receive(
            |data| {
                packed::GcsFilterMessage::from_slice(&data)
                    .map(|message| {
                        if let packed::GcsFilterMessageUnion::GcsFilterHashes(gcs_filter_hashes) = message.to_enum() {
                            gcs_filter_hashes.stop_hash() == last_block_hash
                            && gcs_filter_hashes.parent_hash() == node.consensus().genesis_hash()
                        } else {
                            false
                        }
                    }
                    )
                    .unwrap_or(false)
            },
            "Node0 should send back GcsFilterHashes message",
        );
    }
}