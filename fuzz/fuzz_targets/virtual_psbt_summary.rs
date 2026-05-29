#![no_main]

use libfuzzer_sys::fuzz_target;
use tap_ldk_core::{
    asset::AssetAmount,
    virtual_psbt::{SigningDomain, VirtualPacketSummary},
};

fuzz_target!(|data: &[u8]| {
    if data.len() < 13 {
        return;
    }

    let mut amount = [0u8; 8];
    amount.copy_from_slice(&data[5..13]);
    let summary = VirtualPacketSummary {
        version: data[0],
        chain_params_hrp: if data[1] & 1 == 0 {
            "taprt".to_owned()
        } else {
            "bc".to_owned()
        },
        input_count: data[2] as usize,
        output_count: data[3] as usize,
        total_output_amount: AssetAmount::new(u64::from_be_bytes(amount)),
        signing_domain: if data[4] & 1 == 0 {
            SigningDomain::TaprootAssets
        } else {
            SigningDomain::Bitcoin
        },
    };

    let _ = summary.validate();
    let _ = summary.canonical_summary();
});
