use proptest::prelude::*;
use proptest::test_runner::{Config, FileFailurePersistence};

use tap_ldk_core::{
    asset::{AssetAmount, AssetLeaf, Bytes32, CompressedKey, derive_hash_sum_root},
    proof::{
        ProofAnchorPolicy, ProofAnchorState, ProofHistoryEngine, ProofHistoryInput,
        ProofHistoryOutput, ProofHistoryRecord, ProofHistoryReplayError, ProofHistoryState,
        ProofTransitionKind,
    },
    rfq_invoice::{
        NativeRfqPolicy, QuoteBoundInvoiceRequest, bind_quote_to_invoice,
        receive_native_rfq_request,
    },
    rfq_quote_store::RfqQuoteStore,
    wallet::{AssetBalance, RegtestIssueRequest, WalletState},
};

proptest! {
    #![proptest_config(Config::with_failure_persistence(FileFailurePersistence::Off))]

    #[test]
    fn proof_replay_conserves_split_amounts(total in 2u64..10_000, left in 1u64..10_000) {
        let left = 1 + (left % (total - 1));
        let right = total - left;
        let asset_id = bytes32(7);
        let issuer = key(2);
        let receiver = key(3);

        let records = vec![
            record(
                "issue",
                ProofTransitionKind::Issuance,
                1,
                vec![],
                vec![output("issued", asset_id, total, issuer, 1, ProofHistoryState::Accepted)],
            ),
            record(
                "split",
                ProofTransitionKind::Split,
                2,
                vec!["issued"],
                vec![
                    output("left", asset_id, left, issuer, 2, ProofHistoryState::Accepted),
                    output("right", asset_id, right, receiver, 3, ProofHistoryState::Accepted),
                ],
            ),
        ];

        let replay = ProofHistoryEngine::replay(&records).expect("conserved split replays");
        let replayed_total = replay.accepted_explanations().try_fold(
            AssetAmount::ZERO,
            |total, explanation| total.checked_add(explanation.amount),
        ).expect("sum does not overflow");

        prop_assert_eq!(replayed_total.value(), total);
    }

    #[test]
    fn proof_replay_rejects_split_inflation(total in 1u64..10_000, extra in 1u64..10_000) {
        let asset_id = bytes32(8);
        let issuer = key(2);
        let inflated = total.saturating_add(extra);

        let records = vec![
            record(
                "issue",
                ProofTransitionKind::Issuance,
                1,
                vec![],
                vec![output("issued", asset_id, total, issuer, 1, ProofHistoryState::Accepted)],
            ),
            record(
                "inflated-transfer",
                ProofTransitionKind::Transfer,
                2,
                vec!["issued"],
                vec![output(
                    "inflated",
                    asset_id,
                    inflated,
                    issuer,
                    2,
                    ProofHistoryState::Accepted,
                )],
            ),
        ];

        let rejected_inflation = matches!(
            ProofHistoryEngine::replay(&records),
            Err(ProofHistoryReplayError::AmountNotConserved { .. })
        );
        prop_assert!(rejected_inflation);
    }

    #[test]
    fn proof_replay_anchor_policy_blocks_unaccepted_anchors(
        amount in 1u64..10_000,
        state_index in 0u8..5,
    ) {
        let asset_id = bytes32(9);
        let proof_output = output(
            "issued",
            asset_id,
            amount,
            key(2),
            1,
            ProofHistoryState::Accepted,
        );
        let anchor = proof_output.anchor_outpoint.clone();
        let records = vec![record(
            "issue",
            ProofTransitionKind::Issuance,
            1,
            vec![],
            vec![proof_output],
        )];
        let anchor_state = anchor_state(state_index);
        let result = ProofHistoryEngine::replay_with_anchor_policy(
            &records,
            &ProofAnchorPolicy::strict_confirmed().with_anchor_state(anchor, anchor_state),
        );

        if anchor_state == ProofAnchorState::Confirmed {
            prop_assert!(result.is_ok());
        } else {
            let rejected_anchor = matches!(
                result,
                Err(ProofHistoryReplayError::UnacceptableAnchorState { .. })
            );
            prop_assert!(rejected_anchor);
        }
    }

    #[test]
    fn wallet_restart_preserves_confirmed_balances_and_rejects_reorged_anchors(
        amount in 1u64..10_000,
        anchor_state_index in 0u8..5,
    ) {
        let script_key = key(2);
        let mut wallet = WalletState::default();
        wallet.issue_regtest_asset(RegtestIssueRequest::openusd(AssetAmount::new(amount), script_key))
            .expect("issuance succeeds");
        let (anchor, asset_id) = {
            let utxo = wallet.spendable_utxos.values().next().expect("utxo exists");
            (utxo.anchor_outpoint.clone(), utxo.asset_id.clone())
        };
        let serialized = serde_json::to_vec(&wallet).expect("wallet serializes");
        let mut reloaded = serde_json::from_slice::<WalletState>(&serialized).expect("wallet reloads");
        reloaded.validate().expect("wallet validates after restart");
        prop_assert_eq!(reloaded.balances().expect("balances"), vec![AssetBalance {
            asset_id: asset_id.clone(),
            spendable: amount,
        }]);

        let anchor_state = anchor_state(anchor_state_index);
        reloaded.update_anchor_state(&anchor, anchor_state).expect("anchor updates");
        let balances = reloaded.balances().expect("balances after anchor update");
        if anchor_state == ProofAnchorState::Confirmed {
            prop_assert_eq!(balances, vec![AssetBalance { asset_id, spendable: amount }]);
        } else {
            prop_assert!(balances.is_empty());
        }
    }

    #[test]
    fn rfq_invoice_binding_rejects_wrong_amounts(
        asset_amount in 1u64..10_000,
        delta in 1u64..1_000,
    ) {
        let asset_id = bytes32(10);
        let rfq_id = bytes32(11);
        let invoice_context = bytes32(12);
        let payment_hash = bytes32(13);
        let mut store = RfqQuoteStore::default();
        let accept = receive_native_rfq_request(
            &mut store,
            "alice",
            &tap_ldk_core::asset_peer_message::AssetPeerMessage::RfqRequest {
                rfq_id,
                asset_id,
                asset_amount,
                invoice_context,
            },
            1_000,
            NativeRfqPolicy::default(),
        ).expect("quote accepted");

        let wrong_asset_amount = asset_amount.saturating_add(delta);
        let request = QuoteBoundInvoiceRequest {
            invoice: "lnbc1tapldkproperty".to_owned(),
            payment_hash,
            peer: "alice".to_owned(),
            asset_id,
            asset_amount: wrong_asset_amount,
            btc_msat: accept.quote.btc_msat,
            invoice_context,
            invoice_expiry_unix_seconds: accept.quote.expiry_unix_seconds,
            now_unix_seconds: 1_000,
        };

        prop_assert!(bind_quote_to_invoice(&accept.quote, request).is_err());
    }
}

fn record(
    record_id: &str,
    kind: ProofTransitionKind,
    transition_seed: u8,
    inputs: Vec<&str>,
    outputs: Vec<ProofHistoryOutput>,
) -> ProofHistoryRecord {
    ProofHistoryRecord {
        record_id: record_id.to_owned(),
        kind,
        virtual_transition_id: bytes32(transition_seed),
        inputs: inputs.into_iter().map(ProofHistoryInput::new).collect(),
        outputs,
    }
}

fn output(
    output_id: &str,
    asset_id: Bytes32,
    amount: u64,
    script_key: CompressedKey,
    anchor_seed: u8,
    resulting_state: ProofHistoryState,
) -> ProofHistoryOutput {
    let amount = AssetAmount::new(amount);
    ProofHistoryOutput {
        output_id: output_id.to_owned(),
        asset_id,
        amount,
        script_key,
        anchor_outpoint: format!("{}:0", bytes32(anchor_seed).to_hex()),
        tap_asset_root: derive_hash_sum_root(&[AssetLeaf {
            asset_id,
            script_key,
            amount,
        }])
        .expect("root derives"),
        resulting_state,
    }
}

fn anchor_state(index: u8) -> ProofAnchorState {
    match index % 5 {
        0 => ProofAnchorState::Unknown,
        1 => ProofAnchorState::Pending,
        2 => ProofAnchorState::Confirmed,
        3 => ProofAnchorState::Stale,
        _ => ProofAnchorState::Reorged,
    }
}

fn bytes32(seed: u8) -> Bytes32 {
    Bytes32([seed; 32])
}

fn key(prefix: u8) -> CompressedKey {
    CompressedKey([prefix; 33])
}
