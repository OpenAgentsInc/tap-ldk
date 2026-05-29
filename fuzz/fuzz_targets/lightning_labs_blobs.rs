#![no_main]

use libfuzzer_sys::fuzz_target;
use tap_ldk_core::lightning_labs_blob::{
    decode_commitment_blob, decode_funding_blob, decode_htlc_blob,
};

fuzz_target!(|data: &[u8]| {
    let _ = decode_funding_blob(data);
    let _ = decode_htlc_blob(data);
    let _ = decode_commitment_blob(data);
});
