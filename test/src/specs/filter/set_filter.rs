use super::build_set_filter;
use crate::utils::sleep;
use crate::{Net, Spec, TestProtocol};
use ckb_sync::NetworkProtocol;
use ckb_types::{packed, prelude::*};

pub struct SetFilter;

impl Spec for SetFilter {
    crate::name!("set_filter");

    crate::setup!(num_nodes: 1, protocols: vec![TestProtocol::sync(), TestProtocol::filter()]);

    fn run(&self, net: &mut Net) {
        let node = &net.nodes[0];
        node.generate_blocks(node.consensus().finalization_delay_length() as usize);
        net.connect(node);
        let (peer_id, _, _) = net.receive();

        net.send(
            NetworkProtocol::FILTER.into(),
            peer_id,
            build_set_filter(&node.always_success_script().calc_script_hash()),
        );

        sleep(5);

        node.generate_block();

        net.should_receive(
            |data| {
                packed::FilterMessage::from_slice(&data)
                    .map(|message| message.to_enum().item_name() == packed::FilteredBlock::NAME)
                    .unwrap_or(false)
            },
            "Node0 should send back FilteredBlock message",
        );
    }
}
