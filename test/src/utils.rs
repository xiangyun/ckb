use crate::{Net, Node, TXOSet};
use ckb_jsonrpc_types::{BlockTemplate, TransactionWithStatus, TxStatus};
use ckb_types::{
    bytes::Bytes,
    core::{BlockNumber, BlockView, HeaderView, TransactionView},
    h256,
    packed::{
        Block, BlockTransactions, Byte32, CompactBlock, GetBlocks, RelayMessage, RelayTransaction,
        RelayTransactionHashes, RelayTransactions, SendBlock, SendHeaders, SyncMessage,
    },
    prelude::*,
    H256,
};
use std::convert::Into;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tempfile::tempdir;

pub const FLAG_SINCE_RELATIVE: u64 =
    0b1000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;
pub const FLAG_SINCE_BLOCK_NUMBER: u64 =
    0b000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;
// pub const FLAG_SINCE_EPOCH_NUMBER: u64 =
//    0b010_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;
pub const FLAG_SINCE_TIMESTAMP: u64 =
    0b100_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;

// Build compact block based on core block, and specific prefilled indices
pub fn build_compact_block_with_prefilled(block: &BlockView, prefilled: Vec<usize>) -> Bytes {
    let prefilled = prefilled.into_iter().collect();
    let compact_block = CompactBlock::build_from_block(block, &prefilled);
    RelayMessage::new_builder()
        .set(compact_block)
        .build()
        .as_bytes()
}

// Build compact block based on core block
pub fn build_compact_block(block: &BlockView) -> Bytes {
    build_compact_block_with_prefilled(block, Vec::new())
}

pub fn build_block_transactions(block: &BlockView) -> Bytes {
    // compact block has always prefilled cellbase
    let block_txs = BlockTransactions::new_builder()
        .block_hash(block.header().hash())
        .transactions(
            block
                .transactions()
                .into_iter()
                .map(|view| view.data())
                .skip(1)
                .pack(),
        )
        .build();
    RelayMessage::new_builder()
        .set(block_txs)
        .build()
        .as_bytes()
}

pub fn build_header(header: &HeaderView) -> Bytes {
    build_headers(&[header.clone()])
}

pub fn build_headers(headers: &[HeaderView]) -> Bytes {
    let send_headers = SendHeaders::new_builder()
        .headers(
            headers
                .iter()
                .map(|view| view.data())
                .collect::<Vec<_>>()
                .pack(),
        )
        .build();
    SyncMessage::new_builder()
        .set(send_headers)
        .build()
        .as_bytes()
}

pub fn build_block(block: &BlockView) -> Bytes {
    SyncMessage::new_builder()
        .set(SendBlock::new_builder().block(block.data()).build())
        .build()
        .as_bytes()
}

pub fn build_get_blocks(hashes: &[Byte32]) -> Bytes {
    let get_blocks = GetBlocks::new_builder()
        .block_hashes(hashes.iter().map(ToOwned::to_owned).pack())
        .build();
    SyncMessage::new_builder()
        .set(get_blocks)
        .build()
        .as_bytes()
}

pub fn build_relay_txs(transactions: &[(TransactionView, u64)]) -> Bytes {
    let transactions = transactions.iter().map(|(tx, cycles)| {
        RelayTransaction::new_builder()
            .cycles(cycles.pack())
            .transaction(tx.data())
            .build()
    });
    let txs = RelayTransactions::new_builder()
        .transactions(transactions.pack())
        .build();
    RelayMessage::new_builder().set(txs).build().as_bytes()
}

pub fn build_relay_tx_hashes(hashes: &[Byte32]) -> Bytes {
    let content = RelayTransactionHashes::new_builder()
        .tx_hashes(hashes.iter().map(ToOwned::to_owned).pack())
        .build();
    RelayMessage::new_builder().set(content).build().as_bytes()
}

pub fn new_block_with_template(template: BlockTemplate) -> Block {
    Block::from(template)
        .as_advanced_builder()
        .nonce(rand::random::<u128>().pack())
        .build()
        .data()
}

pub fn wait_until<F>(secs: u64, mut f: F) -> bool
where
    F: FnMut() -> bool,
{
    let start = Instant::now();
    let timeout = Duration::new(secs, 0);
    while Instant::now().duration_since(start) <= timeout {
        if f() {
            return true;
        }
        sleep(Duration::new(1, 0));
    }
    false
}

// Clear net message channel
pub fn clear_messages(net: &Net) {
    while let Ok(_) = net.receive_timeout(Duration::new(3, 0)) {}
}

pub fn since_from_relative_block_number(block_number: BlockNumber) -> u64 {
    FLAG_SINCE_RELATIVE | FLAG_SINCE_BLOCK_NUMBER | block_number
}

pub fn since_from_absolute_block_number(block_number: BlockNumber) -> u64 {
    FLAG_SINCE_BLOCK_NUMBER | block_number
}

// pub fn since_from_relative_epoch_number(epoch_number: EpochNumber) -> u64 {
//     FLAG_SINCE_RELATIVE | FLAG_SINCE_EPOCH_NUMBER | epoch_number
// }
//
// pub fn since_from_absolute_epoch_number(epoch_number: EpochNumber) -> u64 {
//     FLAG_SINCE_EPOCH_NUMBER | epoch_number
// }

pub fn since_from_relative_timestamp(timestamp: u64) -> u64 {
    FLAG_SINCE_RELATIVE | FLAG_SINCE_TIMESTAMP | timestamp
}

pub fn since_from_absolute_timestamp(timestamp: u64) -> u64 {
    FLAG_SINCE_TIMESTAMP | timestamp
}

pub fn assert_send_transaction_fail(node: &Node, transaction: &TransactionView, message: &str) {
    let result = node
        .rpc_client()
        .inner()
        .lock()
        .send_transaction(transaction.data().into())
        .call();
    assert!(
        result.is_err(),
        "expect error \"{}\" but got \"Ok(())\"",
        message,
    );
    let error = result.expect_err(&format!("transaction is invalid since {}", message));
    let error_string = error.to_string();
    assert!(
        error_string.contains(message),
        "expect error \"{}\" but got \"{}\"",
        message,
        error_string,
    );
}

pub fn is_committed(tx_status: &TransactionWithStatus) -> bool {
    let committed_status = TxStatus::committed(h256!("0x0"));
    tx_status.tx_status.status == committed_status.status
}

/// Return a random path located on temp_dir
///
/// We use `tempdir` only for generating a random path, and expect the corresponding directory
/// that `tempdir` creates be deleted when go out of this function.
pub fn temp_path() -> String {
    let tempdir = tempdir().expect("create tempdir failed");
    let path = tempdir.path().to_str().unwrap().to_owned();
    tempdir.close().expect("close tempdir failed");
    path
}

/// Generate new blocks and explode these cellbases into `n` live cells
pub fn generate_utxo_set(node: &Node, n: usize) -> TXOSet {
    // Ensure all the cellbases will be used later are already mature.
    let cellbase_maturity = node.consensus().cellbase_maturity();
    node.generate_blocks(cellbase_maturity as usize);

    // Explode these mature cellbases into multiple cells
    let mut n_outputs = 0;
    let mut txs = Vec::new();
    while n > n_outputs {
        node.generate_block();
        let mature_number = node.get_tip_block_number() - cellbase_maturity;
        let mature_block = node.get_block_by_number(mature_number);
        let mature_cellbase = mature_block.transaction(0).unwrap();
        let mature_utxos: TXOSet = TXOSet::from(&mature_cellbase);
        let tx = mature_utxos.boom(vec![node.always_success_cell_dep()]);
        n_outputs += tx.outputs().len();
        txs.push(tx);
    }

    // Ensure all the transactions were committed
    txs.iter().for_each(|tx| {
        node.submit_transaction(tx);
    });
    while txs
        .iter()
        .any(|tx| !is_committed(&node.rpc_client().get_transaction(tx.hash()).unwrap()))
    {
        node.generate_blocks(node.consensus().finalization_delay_length() as usize);
    }

    let mut utxos = TXOSet::default();
    txs.iter()
        .for_each(|tx| utxos.extend(Into::<TXOSet>::into(tx)));
    utxos.truncate(n);
    utxos
}
