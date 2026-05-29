#![no_main]

use libfuzzer_sys::fuzz_target;
use tap_ldk_core::taproot_commitment::parse_tap_leaf_script_root;

fuzz_target!(|data: &[u8]| {
    let _ = parse_tap_leaf_script_root(data);
});
