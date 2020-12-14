use ckb_hash::blake2b_256;
use wasm_bindgen_test::*;
wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn ckb_hash_should_work() {
    let input = b"ckb";
    let digest = blake2b_256(&input);
    assert_eq!(
        [
            58, 30, 65, 28, 36, 68, 184, 213, 134, 227, 185, 160, 52, 83, 238, 138, 26, 52, 233,
            78, 15, 18, 48, 117, 88, 24, 220, 169, 207, 158, 73, 120
        ],
        digest
    );
}
