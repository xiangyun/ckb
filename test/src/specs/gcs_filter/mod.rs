use ckb_types::{packed, prelude::*};

mod get_gcs_filters;
mod get_gcs_filter_hashes;

pub use get_gcs_filters::*;
pub use get_gcs_filter_hashes::*;

fn gcs_filter_match(filter: &[u8], script_hashes: &[packed::Byte32]) -> bool {
    // use same value as bip158
    let p = 19;
    let m = 1.497_137 * f64::from(2u32.pow(p));
    let reader = golomb_coded_set::GCSFilterReader::new(0, 0, m as u64, p as u8);
    let mut input = std::io::Cursor::new(filter);
    reader.match_any(&mut input, &mut script_hashes.iter().map(|v| v.as_slice())).unwrap_or_default()
}