use ckb_types::{
    bytes::Bytes,
    core::{cell::CellMeta, TransactionInfo},
    packed::{Byte32, OutPoint},
};

/// TODO(doc): @quake
pub trait CellDataProvider {
    /// load cell_data from memory, fallback to storage access
    fn load_cell_data(&self, cell: &CellMeta) -> Option<Bytes> {
        cell.mem_cell_data
            .as_ref()
            .map(ToOwned::to_owned)
            .or_else(|| self.get_cell_data(&cell.out_point))
    }

    /// load cell_data_hash from memory, fallback to storage access
    fn load_cell_data_hash(&self, cell: &CellMeta) -> Option<Byte32> {
        cell.mem_cell_data_hash
            .as_ref()
            .map(ToOwned::to_owned)
            .or_else(|| self.get_cell_data_hash(&cell.out_point))
    }

    /// load transaction info from memory, fallback to storage access
    fn load_transaction_info(&self, cell: &CellMeta) -> Option<TransactionInfo> {
        cell.transaction_info
            .as_ref()
            .map(ToOwned::to_owned)
            .or_else(|| self.get_transaction_info(&cell.out_point.tx_hash()))
    }

    /// fetch cell_data from storage
    fn get_cell_data(&self, out_point: &OutPoint) -> Option<Bytes>;

    /// fetch cell_data_hash from storage
    fn get_cell_data_hash(&self, out_point: &OutPoint) -> Option<Byte32>;

    /// Fetch the transaction information of the cell from storage
    fn get_transaction_info(&self, tx_hash: &Byte32) -> Option<TransactionInfo>;
}
