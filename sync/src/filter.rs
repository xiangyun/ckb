use crate::{types::SyncShared, BAD_MESSAGE_BAN_TIME};
use bloom_filters::{
    BloomFilter, ClassicBloomFilter, DefaultBuildHashKernels, DefaultBuildHasher,
    UpdatableBloomFilter,
};
use ckb_logger::{debug, info, warn};
use ckb_network::{bytes::Bytes, CKBProtocolContext, CKBProtocolHandler, PeerIndex};
use ckb_notify::NotifyController;
use ckb_store::ChainStore;
use ckb_types::{core, packed, prelude::*, utilities::merkle_proof};
use ckb_util::RwLock;
use crossbeam_channel::Receiver;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

const FILTER_NOTIFY_INTERVAL: Duration = Duration::from_millis(200);
const SEND_FILTERED_BLOCK_TOKEN: u64 = 0;
const MAX_GET_FILTERED_BLOCKS_LEN: usize = 512;
const MAX_FILTERED_BLOCKS_LEN: usize = 4;

pub type Filter = ClassicBloomFilter<DefaultBuildHashKernels<DefaultBuildHasher>>;

pub fn new_filter(items_count: usize, fp_rate: f64, hash_seed: u32) -> Filter {
    Filter::new(
        items_count,
        fp_rate,
        DefaultBuildHashKernels::new(hash_seed as usize, DefaultBuildHasher),
    )
}

#[derive(Clone)]
pub struct FilterProtocol {
    peer_filters: Arc<RwLock<HashMap<PeerIndex, Filter>>>,
    shared: Arc<SyncShared>,
    new_block_receiver: Receiver<core::BlockView>,
}

impl FilterProtocol {
    pub fn new(notify_controller: &NotifyController, shared: Arc<SyncShared>) -> Self {
        FilterProtocol {
            peer_filters: Default::default(),
            new_block_receiver: notify_controller.subscribe_new_block("FilterProtocol"),
            shared,
        }
    }

    fn process<'r>(
        &self,
        nc: &dyn CKBProtocolContext,
        peer: PeerIndex,
        message: packed::FilterMessageUnionReader<'r>,
    ) {
        match message {
            packed::FilterMessageUnionReader::SetFilter(reader) => {
                let message = reader.to_entity();
                let filter = Filter::with_raw_data(
                    &message.filter().raw_data(),
                    Into::<u8>::into(message.num_hashes()) as usize,
                    DefaultBuildHashKernels::new(message.hash_seed().unpack(), DefaultBuildHasher),
                );
                self.peer_filters.write().insert(peer, filter);
            }
            packed::FilterMessageUnionReader::AddFilter(reader) => {
                let message = reader.to_entity();
                self.peer_filters
                    .write()
                    .entry(peer)
                    .and_modify(|filter| filter.update(&message.filter().raw_data()));
            }
            packed::FilterMessageUnionReader::ClearFilter(_) => {
                self.peer_filters.write().remove(&peer);
            }
            packed::FilterMessageUnionReader::GetFilteredBlocks(reader) => {
                let block_hashes = reader.block_hashes();
                if block_hashes.len() <= MAX_GET_FILTERED_BLOCKS_LEN {
                    match self.peer_filters.read().get(&peer) {
                        Some(filter) => {
                            let store = self.shared.store();
                            let mut unmatched_block_hashes: Vec<packed::Byte32> = Vec::new();
                            let mut matched_block_hashes: Vec<packed::Byte32> = Vec::new();
                            let mut matched_blocks: Vec<packed::FilteredTransactions> = Vec::new();

                            for (i, block_hash) in block_hashes.iter().enumerate() {
                                let block_hash = block_hash.to_entity();

                                if let Some(block) = store.get_block(&block_hash) {
                                    match build_filtered_transactions(&block, filter, store) {
                                        Some(filtered_transactions) => {
                                            matched_block_hashes.push(block_hash);
                                            matched_blocks.push(filtered_transactions);
                                        }
                                        None => unmatched_block_hashes.push(block_hash),
                                    }
                                } else {
                                    unmatched_block_hashes.push(block_hash);
                                }
                                if matched_block_hashes.len() == MAX_FILTERED_BLOCKS_LEN
                                    || i == block_hashes.len() - 1
                                {
                                    let message = packed::FilterMessage::new_builder()
                                        .set(
                                            packed::FilteredBlocks::new_builder()
                                                .unmatched_block_hashes(
                                                    unmatched_block_hashes.drain(..).pack(),
                                                )
                                                .matched_block_hashes(
                                                    matched_block_hashes.drain(..).pack(),
                                                )
                                                .matched_blocks(
                                                    packed::FilteredTransactionsVec::new_builder()
                                                        .extend(matched_blocks.drain(..))
                                                        .build(),
                                                )
                                                .build(),
                                        )
                                        .build();

                                    if let Err(err) = nc.send_message_to(peer, message.as_bytes()) {
                                        debug!(
                                            "filter send FilteredBlocks message error: {:?}",
                                            err
                                        );
                                    }
                                }
                            }
                        }
                        None => {
                            warn!(
                                "Peer {} send us a GetFilteredBlocks message but no filter is set",
                                peer
                            );
                            let message = packed::FilterMessage::new_builder()
                                .set(packed::FilteredBlocks::default())
                                .build();
                            if let Err(err) = nc.send_message_to(peer, message.as_bytes()) {
                                debug!("filter send empty FilteredBlocks message error: {:?}", err);
                            }
                        }
                    }
                } else {
                    nc.ban_peer(
                        peer,
                        BAD_MESSAGE_BAN_TIME,
                        format!("send us a GetFilteredBlocks message and block_hashes len ({}) is over limit", block_hashes.len()),
                    );
                }
            }
            packed::FilterMessageUnionReader::FilteredBlocks(_)
            | packed::FilterMessageUnionReader::FilteredBlock(_) => {
                nc.ban_peer(
                    peer,
                    BAD_MESSAGE_BAN_TIME,
                    "send us a FilteredBlock or FilteredBlocks message".to_string(),
                );
            }
        }
    }
}

impl CKBProtocolHandler for FilterProtocol {
    fn init(&mut self, nc: Arc<dyn CKBProtocolContext + Sync>) {
        nc.set_notify(FILTER_NOTIFY_INTERVAL, SEND_FILTERED_BLOCK_TOKEN)
            .expect("set_notify at init is ok");
    }

    fn received(
        &mut self,
        nc: Arc<dyn CKBProtocolContext + Sync>,
        peer_index: PeerIndex,
        data: Bytes,
    ) {
        let msg = match packed::FilterMessage::from_slice(&data) {
            Ok(msg) => msg.to_enum(),
            _ => {
                info!("Peer {} sends us a malformed filter message", peer_index);
                nc.ban_peer(
                    peer_index,
                    BAD_MESSAGE_BAN_TIME,
                    String::from("send us a malformed filter message"),
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

    fn notify(&mut self, nc: Arc<dyn CKBProtocolContext + Sync>, token: u64) {
        match token {
            SEND_FILTERED_BLOCK_TOKEN => {
                if let Ok(block) = self.new_block_receiver.try_recv() {
                    for (peer, filter) in self.peer_filters.read().iter() {
                        let filtered_block =
                            build_filtered_block(&block, filter, self.shared.store());
                        let message = packed::FilterMessage::new_builder()
                            .set(filtered_block)
                            .build();
                        if let Err(err) = nc.send_message_to(*peer, message.as_bytes()) {
                            debug!("filter send FilteredBlock message error: {:?}", err);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn disconnected(&mut self, _nc: Arc<dyn CKBProtocolContext + Sync>, peer: PeerIndex) {
        self.peer_filters.write().remove(&peer);
    }
}

fn build_filtered_block<'a, S: ChainStore<'a>>(
    block: &core::BlockView,
    filter: &Filter,
    store: &'a S,
) -> packed::FilteredBlock {
    let filtered_transactions = build_filtered_transactions(block, filter, store);

    packed::FilteredBlock::new_builder()
        .header(block.header().data())
        .transactions(
            packed::FilteredTransactionsOpt::new_builder()
                .set(filtered_transactions)
                .build(),
        )
        .build()
}

fn build_filtered_transactions<'a, S: ChainStore<'a>>(
    block: &core::BlockView,
    filter: &Filter,
    store: &'a S,
) -> Option<packed::FilteredTransactions> {
    let (matched_indices, matched_txs): (Vec<_>, Vec<_>) = block
        .transactions()
        .iter()
        .enumerate()
        .filter_map(|(index, tx)| {
            // 1. Check the hash of the transaction itself
            // 2. For each output, check the script hash of lock and type
            // 3. For each input, check the script hash of previous output's lock and type
            if filter.contains(&tx.hash())
                || tx
                    .outputs()
                    .into_iter()
                    .any(|output| match_output(&output, filter))
                || tx.input_pts_iter().any(|out_point| {
                    store
                        .get_transaction(&out_point.tx_hash())
                        .map_or(false, |(tx, _)| {
                            match_output(
                                &tx.outputs()
                                    .get(out_point.index().unpack())
                                    .expect("stored tx"),
                                filter,
                            )
                        })
                })
            {
                Some((index as u32, tx.data()))
            } else {
                None
            }
        })
        .unzip();

    if matched_indices.is_empty() {
        None
    } else {
        let tx_hashes: Vec<_> = block.transactions().iter().map(|tx| tx.hash()).collect();
        let proof = merkle_proof(&tx_hashes, &matched_indices).expect("verified index");

        Some(
            packed::FilteredTransactions::new_builder()
                .transactions(
                    packed::TransactionVec::new_builder()
                        .set(matched_txs)
                        .build(),
                )
                .proof_indices(proof.indices().pack())
                .proof_lemmas(
                    packed::Byte32Vec::new_builder()
                        .set(proof.lemmas().into())
                        .build(),
                )
                .build(),
        )
    }
}

fn match_output(output: &packed::CellOutput, filter: &Filter) -> bool {
    filter.contains(&output.lock().calc_script_hash())
        || output.type_().to_opt().map_or(false, |type_script| {
            filter.contains(&type_script.calc_script_hash())
        })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tests::util::build_chain;

    #[test]
    fn test_build_filtered_block() {
        {
            let mut filter = new_filter(10, 0.0001, rand::random());
            let (shared, _chain) = build_chain(1);
            let block = shared
                .active_chain()
                .get_block(&shared.active_chain().tip_hash())
                .unwrap();
            assert!(build_filtered_block(&block, &filter, shared.store())
                .transactions()
                .is_none());

            filter.insert(&block.transactions().first().unwrap().hash());
            let filtered = build_filtered_block(&block, &filter, shared.store())
                .transactions()
                .to_opt()
                .expect("filtered transactions");
            assert_eq!(1, filtered.transactions().len());
            assert_eq!(
                block.transactions().first().unwrap().data().as_slice(),
                filtered.transactions().get(0).unwrap().as_slice()
            );
        }
        {
            let mut filter = new_filter(10, 0.0001, rand::random());
            let (shared, _chain) = build_chain(20);
            let block = shared
                .active_chain()
                .get_block(&shared.active_chain().tip_hash())
                .unwrap();

            filter.insert(
                &block
                    .transactions()
                    .first()
                    .unwrap()
                    .outputs()
                    .get(0)
                    .unwrap()
                    .lock()
                    .calc_script_hash(),
            );
            let filtered = build_filtered_block(&block, &filter, shared.store())
                .transactions()
                .to_opt()
                .expect("filtered transactions");
            assert_eq!(1, filtered.transactions().len());
            assert_eq!(
                block.transactions().first().unwrap().data().as_slice(),
                filtered.transactions().get(0).unwrap().as_slice()
            );
        }
    }
}
