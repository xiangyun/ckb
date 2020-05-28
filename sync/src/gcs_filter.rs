use crate::{types::SyncShared, BAD_MESSAGE_BAN_TIME};
use ckb_hash::blake2b_256;
use ckb_logger::{debug, info};
use ckb_network::{bytes::Bytes, CKBProtocolContext, CKBProtocolHandler, PeerIndex};
use ckb_store::ChainStore;
use ckb_types::{packed, prelude::*};
use std::sync::Arc;
use std::time::Instant;

const MAX_FILTER_RANGE_SIZE: usize = 2000;
const MIN_CHECK_POINT_INTERVAL: u32 = 200_000;

pub struct GcsFilterProtocol {
    shared: Arc<SyncShared>,
}

impl GcsFilterProtocol {
    pub fn new(shared: Arc<SyncShared>) -> Self {
        GcsFilterProtocol { shared }
    }

    fn process<'r>(
        &self,
        nc: &dyn CKBProtocolContext,
        peer: PeerIndex,
        message: packed::GcsFilterMessageUnionReader<'r>,
    ) {
        match message {
            packed::GcsFilterMessageUnionReader::GetGcsFilters(reader) => {
                let message = reader.to_entity();
                let store = self.shared.store();
                if let Some(end_number) = store.get_block_number(&message.stop_hash()) {
                    for block_number in
                        (message.start_number().unpack()..=end_number).take(MAX_FILTER_RANGE_SIZE)
                    {
                        if let Some(hash) = store.get_block_hash(block_number) {
                            if let Some(filter) = store.get_gcs_filter(&hash) {
                                let msg = packed::GcsFilterMessage::new_builder()
                                    .set(
                                        packed::GcsFilter::new_builder()
                                            .block_hash(hash)
                                            .filter(filter.pack())
                                            .build(),
                                    )
                                    .build();
                                if let Err(err) = nc.send_message_to(peer, msg.as_bytes()) {
                                    debug!(
                                        "GcsFilterProtocol send GcsFilter message error: {:?}",
                                        err
                                    );
                                }
                            }
                        }
                    }
                }
            }
            packed::GcsFilterMessageUnionReader::GetGcsFilterHashes(reader) => {
                let message = reader.to_entity();
                let store = self.shared.store();
                let stop_hash = message.stop_hash();
                if let Some(end_number) = store.get_block_number(&stop_hash) {
                    let start_number = message.start_number().unpack();
                    if let Some(parent_hash) = store.get_block_hash(start_number).and_then(|hash| {
                        store
                            .get_block_header(&hash)
                            .map(|header| header.parent_hash())
                    }) {
                        let filter_hashes = (start_number..=end_number)
                            .take(MAX_FILTER_RANGE_SIZE)
                            .filter_map(|block_number| {
                                store
                                    .get_block_hash(block_number)
                                    .and_then(|hash| store.get_gcs_filter(&hash))
                                    .map(|bytes| blake2b_256(bytes).pack())
                            })
                            .collect::<Vec<_>>();

                        if !filter_hashes.is_empty() {
                            let msg = packed::GcsFilterMessage::new_builder()
                                .set(
                                    packed::GcsFilterHashes::new_builder()
                                        .parent_hash(parent_hash)
                                        .stop_hash(message.stop_hash())
                                        .filter_hashes(filter_hashes.pack())
                                        .build(),
                                )
                                .build();
                            if let Err(err) = nc.send_message_to(peer, msg.as_bytes()) {
                                debug!(
                                    "GcsFilterProtocol send GcsFilterHashes message error: {:?}",
                                    err
                                );
                            }
                        }
                    }
                }
            }
            packed::GcsFilterMessageUnionReader::GetGcsFilterCheckPoint(reader) => {
                let message = reader.to_entity();
                let store = self.shared.store();
                let stop_hash = message.stop_hash();
                let interval: u32 = message.interval().unpack();
                if interval >= MIN_CHECK_POINT_INTERVAL {
                    if let Some(end_number) = store.get_block_number(&stop_hash) {
                        let filter_hashes = (0..end_number)
                            .step_by(interval as usize)
                            .chain(vec![end_number])
                            .filter_map(|block_number| {
                                store
                                    .get_block_hash(block_number)
                                    .and_then(|hash| store.get_gcs_filter(&hash))
                                    .map(|bytes| blake2b_256(bytes).pack())
                            })
                            .collect::<Vec<_>>();

                        if !filter_hashes.is_empty() {
                            let msg = packed::GcsFilterMessage::new_builder()
                                .set(
                                    packed::GcsFilterCheckPoint::new_builder()
                                        .stop_hash(message.stop_hash())
                                        .filter_hashes(filter_hashes.pack())
                                        .build(),
                                )
                                .build();
                            if let Err(err) = nc.send_message_to(peer, msg.as_bytes()) {
                                debug!("GcsFilterProtocol send GcsFilterCheckPoint message error: {:?}", err);
                            }
                        }
                    }
                } else {
                    nc.ban_peer(
                        peer,
                        BAD_MESSAGE_BAN_TIME,
                        format!(
                            "send us a GetGcsFilterCheckPoint message with invalid interval: {}",
                            interval
                        ),
                    );
                }
            }
            _ => {
                nc.ban_peer(
                    peer,
                    BAD_MESSAGE_BAN_TIME,
                    format!("send us a {} message", message.item_name()),
                );
            }
        }
    }
}

impl CKBProtocolHandler for GcsFilterProtocol {
    fn init(&mut self, _nc: Arc<dyn CKBProtocolContext + Sync>) {}

    fn received(
        &mut self,
        nc: Arc<dyn CKBProtocolContext + Sync>,
        peer_index: PeerIndex,
        data: Bytes,
    ) {
        let msg = match packed::GcsFilterMessage::from_slice(&data) {
            Ok(msg) => msg.to_enum(),
            _ => {
                info!("Peer {} sends us a malformed GcsFilterMessage", peer_index);
                nc.ban_peer(
                    peer_index,
                    BAD_MESSAGE_BAN_TIME,
                    String::from("send us a malformed GcsFilterMessage"),
                );
                return;
            }
        };

        let start_time = Instant::now();
        self.process(nc.as_ref(), peer_index, msg.as_reader());
        debug!(
            "process message={}, peer={}, cost={:?}",
            msg.item_name(),
            peer_index,
            start_time.elapsed(),
        );
    }
}
