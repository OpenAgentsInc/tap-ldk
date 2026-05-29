#![no_main]

use libfuzzer_sys::fuzz_target;
use tap_ldk_core::tapd_proof::{decode_tapd_proof_file, decode_tapd_single_proof};

fuzz_target!(|data: &[u8]| {
    let _ = decode_tapd_single_proof(data);
    let _ = decode_tapd_proof_file(data);
});
