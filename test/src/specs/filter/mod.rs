use ckb_network::bytes::Bytes;
use ckb_sync::{new_filter, BloomFilter};
use ckb_types::{packed, prelude::*};

mod get_filtered_blocks;
mod set_filter;

pub use get_filtered_blocks::*;
pub use set_filter::*;

fn build_set_filter(hash: &packed::Byte32) -> Bytes {
    let hash_seed = rand::random();
    let mut filter = new_filter(10, 0.0001, hash_seed);
    filter.insert(hash);
    packed::FilterMessage::new_builder()
        .set(
            packed::SetFilter::new_builder()
                .hash_seed(hash_seed.pack())
                .filter(filter.buckets().raw_data().pack())
                .num_hashes(10.into())
                .build(),
        )
        .build()
        .as_bytes()
}
