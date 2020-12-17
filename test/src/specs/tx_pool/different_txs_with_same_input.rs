use crate::util::cell::gen_spendable;
use crate::util::check::is_transaction_committed;
use crate::util::mining::{mine, mine_until_out_bootstrap_period};
use crate::util::transaction::always_success_transaction;
use crate::{Node, Spec};
use ckb_types::packed::CellInput;
use ckb_types::{
    core::{capacity_bytes, Capacity, TransactionView},
    packed::CellOutputBuilder,
    prelude::*,
};
use log::info;

pub struct DifferentTxsWithSameInput;

impl Spec for DifferentTxsWithSameInput {
    fn run(&self, nodes: &mut Vec<Node>) {
        let node0 = &nodes[0];

        mine_until_out_bootstrap_period(node0);
        let tx_hash_0 = node0.generate_transaction();
        info!("Generate 2 txs with same input");
        let tx1 = node0.new_transaction(tx_hash_0.clone());
        let tx2_temp = node0.new_transaction(tx_hash_0);
        // Set tx2 fee to a higher value, tx1 capacity is 100, set tx2 capacity to 80 for +20 fee.
        let output = CellOutputBuilder::default()
            .capacity(capacity_bytes!(80).pack())
            .build();

        let tx2 = tx2_temp
            .as_advanced_builder()
            .set_outputs(vec![output])
            .build();
        node0.rpc_client().send_transaction(tx1.data().into());
        node0.rpc_client().send_transaction(tx2.data().into());

        mine(&node0, 1);
        mine(&node0, 1);

        info!("RBF (Replace-By-Fees) is not implemented, but transaction fee sorting is ready");
        info!("tx2 should be included in the next + 2 block, and tx1 should be ignored");
        mine(&node0, 1);
        let tip_block = node0.get_tip_block();
        let commit_txs_hash: Vec<_> = tip_block
            .transactions()
            .iter()
            .map(TransactionView::hash)
            .collect();

        assert!(commit_txs_hash.contains(&tx2.hash()));
        assert!(!commit_txs_hash.contains(&tx1.hash()));

        // when tx2 was confirmed, tx1 should be discarded
        let tx = node0.rpc_client().get_transaction(tx1.hash());
        assert!(tx.is_none(), "tx1 should be discarded");
    }
}

pub struct ProposeConflictTransactionsThenSubmit;

impl Spec for ProposeConflictTransactionsThenSubmit {
    fn run(&self, nodes: &mut Vec<Node>) {
        let node0 = &nodes[0];

        // tx_a and tx_b spends the same cell, hence they are conflict transactions for each other
        let inputs = gen_spendable(node0, 1);
        let input_a = CellInput::new(inputs[0].out_point.clone(), 0);
        let input_b = CellInput::new(inputs[0].out_point.clone(), 1);
        let tx_template = always_success_transaction(node0, &inputs[0]);
        let tx_a = tx_template
            .as_advanced_builder()
            .set_inputs(vec![input_a])
            .build();
        let tx_b = tx_template
            .as_advanced_builder()
            .set_inputs(vec![input_b])
            .build();

        // propose tx_a and tx_b
        // and move `block` into proposal window
        let block = node0
            .new_block_builder(None, None, None)
            .proposal(tx_a.proposal_short_id())
            .proposal(tx_b.proposal_short_id())
            .build();
        node0.submit_block(&block);

        let will_fail = true;
        if will_fail {
            // 如果将上述 `block` 推进到 proposal window，
            // 那么在接下来 node0.submit_transaction(&tx_b) 时会返回错误，错误是：
            // TransactionFailedToResolve: OutPoint(Dead(OutPoint(...)))，指 tx_b input was dead，但实际上并 tx_a 还没上链，并不算 dead。
            mine(node0, node0.consensus().tx_proposal_window().closest());
            node0.submit_transaction(&tx_a);
            node0.submit_transaction(&tx_b);
        } else {
            // 如果不推进到 proposal window 这样就一切正常
            node0.submit_transaction(&tx_a);
            node0.submit_transaction(&tx_b);
        }

        // Inside `mine`, RPC `get_block_template` will be involved, that's our testing interface.
        mine(node0, node0.consensus().tx_proposal_window().farthest());
        assert!(is_transaction_committed(node0, &tx_a) || is_transaction_committed(node0, &tx_b));
    }
}
