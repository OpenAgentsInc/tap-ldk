# Path B Live Settlement Diagnostic Run

Date: 2026-05-28

Artifact directory:

- `target/live-lightning-labs-outgoing-payment-diagnostic/`

This run used:

- `OpenAgentsInc/rust-lightning@85189ebe7d3c3b0cf92d504c06e0e3b192a5e5c1`
- `OpenAgentsInc/ldk-node@c5ae040bf84225922c5213d9acb077e031076a9c`

## Result

The result is still blocked at live settlement, but the HTLC transcript is now
captured in the native LDK log.

- `status`: `blocked`
- `blocked_step`: `live_asset_channel_payment_settlement`
- `integrated_litd_asset_channel_fund_status`: `completed`
- `integrated_litd_asset_channel_usable_for_keysend`: `true`
- `integrated_litd_asset_channel_local_balance`: `125`
- `integrated_litd_asset_payment_status`: `timed_out`
- `integrated_litd_asset_payment_wire_status`: `IN_FLIGHT`
- `integrated_litd_asset_payment_hash`:
  `9567b1917e38ac6a3f7bb4be862277bf1ef615bed9c20f41876c18933cbf8405`
- `integrated_litd_post_payment_balance`: `999875`

The native peer reached `channel_ready`, received the live `UpdateAddHTLC`,
then closed on:

```text
Invalid simple-taproot HTLC signature from peer
```

The force-close attempt still failed with:

```text
Invalid Taproot control block size
```

## Native HTLC Transcript

From `native-ldk-litd-peer/ldk_node.log`:

- channel id:
  `422c9bd5ba245dbec837043b36446a886f690a76f5d80b587f112c7a1343cf64`
- HTLC id: `0`
- amount: `354000 msat`
- CLTV expiry: `218`
- payment hash:
  `9567b1917e38ac6a3f7bb4be862277bf1ef615bed9c20f41876c18933cbf8405`
- Taproot Asset HTLC blob:
  `012c0020c246de3d8fd2ac23f5608a6e787d6cd3601c2d62cdbc3640d1b0dbab084e1df50108000000000000007d`

Rust Lightning verified the peer HTLC signature against:

- peer HTLC signature:
  `7b9b51b2c0f3b31a3404e53104205060ea2975f4337e982928ebabcc6d3cd7954c311f906f95c5b6c33897bb343c20f8e79f16abe791aab5f59c69e75681e6f2`
- verifying key:
  `03106bc567cdb1fa700acaff1a637dc21d5f8a4a075e4ca25234b5813db0f19c7b`
- HTLC transaction:
  `0200000001565900635bf3cfabb2e715d85b36a348eff38bd3880ed3ac034235926d53e5d502000000000100000001620100000000000022512095dc737bfa556a7ebcd7c6aa2851ec72775a4fa082f580d5d92eddb58dc24db100000000`
- HTLC transaction outputs:
  `0:354:22512095dc737bfa556a7ebcd7c6aa2851ec72775a4fa082f580d5d92eddb58dc24db1`
- commitment HTLC output index: `2`
- previous output value: `354 sat`
- previous output script:
  `22512081c339b9042ea10fc4a2731bc4858c7c30fd522f8b0717b0b1b7fe0c9369ceb3`
- selected leaf:
  `5f82012088a9148928255a546cf82436ee513f08b488902c4e36d48820d06cf3f101da15d0743c5c6c42f7d2e86546a000aa7a35d152061c050825efdfad20106bc567cdb1fa700acaff1a637dc21d5f8a4a075e4ca25234b5813db0f19c7bac`
- control block:
  `c1770ea7c98b45d4fc078b2ec1024eb48490219d2a4bca3097cdc3f9827ebf94953059cd909171eb98d176e5e1885e63c85a1096a55ea9d886ac43ddb466e5646201b4fa40e88d8a63754ee74d0eaa72b1e6fd101d114590cbe7f51b444d78e8fd`
- first-level aux leaf:
  `496a47e8543f2ab163faf693971a653e54efe3d8056c52164c6b8bbf597b6ddb2802c7686af75a89232d922ca2f47ff29c7e5a6897480213ec8acb56d8b2b8f7e8cd000000000000007d`
- second-level aux leaf:
  `496a47e8543f2ab163faf693971a653e54efe3d8056c52164c6b8bbf597b6ddb280288dbd45e53728ca954abfa2e522f44d22f6b74f5b7dd460bb800ee58ddc99876000000000000007d`
- sighash type: `SinglePlusAnyoneCanPay`
- computed BIP341 sighash:
  `a03dc2c5816d1b158b5966351690f4e49b4e937cd90bfaaef974eb36665afa42`
- `adjusted_for_taproot_asset`: `true`

## `litd` Side Evidence

The `litd` trace shows Lightning Labs using the `tapchannel`/`tapsend`
allocation path for the same HTLC.

For the second-level HTLC asset allocation, Lightning Labs logged:

- internal key:
  `03770ea7c98b45d4fc078b2ec1024eb48490219d2a4bca3097cdc3f9827ebf9495`
- tweaked internal key:
  `0370e5d041406c7b40a8044ec1e38e1e1501c18c4b08d38b15842e6cd3fa06ed83`
- base second-level asset script key:
  `e0a9208bc65205adb7b192cc7515b946aeb98a3de0ae037e777d31a343ffda97`
- tweaked second-level asset script key:
  `f12638d93f43df7ce215f8669a3db27f37ab155b9591fc68f400655666815f96`
- asset commitment key:
  `cfc3784d78ad19d8fab58aa122f9f38f574620b278d75d2c290107733224d79c`
- asset commitment leaf:
  `03bd8afc62c03a029fbc32db9250772092d0dd0fb4fa2b6a067b5c3913886ea6`
- inclusion taproot asset root:
  `01b4fa40e88d8a63754ee74d0eaa72b1e6fd101d114590cbe7f51b444d78e8fd`
- output commitment root:
  `879e7097820ec7ed9f065c91d6482bdefaec33ed5d30f70fbf4cff5d5d7cf12f`

The Lightning Labs virtual signing descriptor for the asset layer used:

- witness script:
  `82012088a9148928255a546cf82436ee513f08b488902c4e36d48820d06cf3f101da15d0743c5c6c42f7d2e86546a000aa7a35d152061c050825efdfad20106bc567cdb1fa700acaff1a637dc21d5f8a4a075e4ca25234b5813db0f19c7bac`
- output value: `125`
- output script:
  `5120994993a3810a1990bcccf848e3d76a5e612312c56abc74d05903374d13c92c07`
- hash type: `131`

Lightning Labs then sent the normal BOLT `CommitSig` HTLC signature:

- HTLC signature:
  `7b9b51b2c0f3b31a3404e53104205060ea2975f4337e982928ebabcc6d3cd7954c311f906f95c5b6c33897bb343c20f8e79f16abe791aab5f59c69e75681e6f2`
- signature type: `1` (Schnorr)

## Current Diagnosis

The BIP340 signature bytes and witness script are visible on both sides, and
the Rust path is using anchor-style BIP342 HTLC signing. The remaining
mismatch is not "ECDSA versus Schnorr" or "anchor sighash versus non-anchor
sighash".

The concrete mismatch is the payment-time Taproot Asset commitment model:

- Rust derives a second-level aux leaf from a bounded local no-split template.
- Lightning Labs derives second-level asset material through
  `tapchannel`/`tapsend`, including the HTLC-index script-key tweak,
  asset-commitment leaf, inclusion proof, output-commitment root, and assigned
  output commitment.
- Rust's second-level aux leaf contains local root/script-key material that
  does not match the Lightning Labs second-level allocation trace.
- Until Rust ports the exact bounded single-asset Lightning Labs allocation
  semantics, it will continue verifying the peer signature against the wrong
  HTLC transaction transcript.

## Next Code Work

1. Add a fixture test around the native transcript above so the current
   mismatch is replayable without Docker.
2. Port the bounded single-asset subset of Lightning Labs
   `CreateSecondLevelHtlcTx` and its `tapsend` output commitment inputs.
3. Replace `taproot_asset_second_level_htlc_aux_leaf_script_for_commitment_output`
   so it uses that exact model instead of the local no-split approximation.
4. Rerun the same live harness and require the native log to show the same
   transcript values Lightning Labs signed.
5. Only after the HTLC signature verifies, fix the force-close control-block
   path and record observed native/`litd`/native balances.
