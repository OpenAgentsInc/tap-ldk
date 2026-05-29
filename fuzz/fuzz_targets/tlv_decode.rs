#![no_main]

use libfuzzer_sys::fuzz_target;
use tap_ldk_core::tlv::decode_stream;

fuzz_target!(|data: &[u8]| {
    let _ = decode_stream(data);
});
