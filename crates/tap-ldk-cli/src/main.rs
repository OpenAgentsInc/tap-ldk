use std::{env, fs, path::Path, process, str::FromStr};

use serde::{Deserialize, Serialize};

use tap_ldk_core::{
    ProjectInfo,
    asset::{AssetAmount, Bytes32, CompressedKey, RootHashSum},
    asset_channel_funding::{AssetChannelStore, run_asset_channel_funding_smoke},
    asset_channel_negotiation::run_negotiation_smoke,
    asset_close::run_native_asset_close_smoke,
    asset_commitment::{AssetCommitmentStore, run_asset_commitment_smoke},
    asset_htlc::run_asset_htlc_smoke,
    asset_payment::run_native_asset_payment_smoke,
    asset_peer_message::run_peer_message_smoke,
    asset_recovery::run_native_asset_recovery_matrix_smoke,
    ldk_baseline::{BaselineBtcSmokeState, BaselineLdkPlan},
    lightning_labs_blob::decode_fixture_hexdumps,
    lightning_labs_funding::run_lightning_labs_funding_interop_fixture_smoke,
    lightning_labs_interop_checks::run_lightning_labs_interop_check_smoke,
    lightning_labs_payment::{
        run_lightning_labs_incoming_payment_smoke, run_lightning_labs_outgoing_payment_smoke,
    },
    lightning_labs_rfq::run_lightning_labs_rfq_invoice_compat_smoke,
    proof::{ProofFile, VerificationScope},
    regtest::{BitcoinRegtestConfig, LightningLabsCounterpartyConfig},
    rfq_invoice::run_rfq_invoice_smoke,
    rfq_quote_store::{RfqQuoteRequest, RfqQuoteStore},
    tapd_proof::{decode_fixture_hex, decode_hex_text, decode_tapd_proof_file},
    wallet::{LocalTransferRequest, RegtestIssueRequest, TapdProofImportRequest, WalletState},
};

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let info = ProjectInfo::current();

    match args.as_slice() {
        [] => {
            print_help(info);
        }
        [flag] if flag == "--help" || flag == "-h" => {
            print_help(info);
        }
        [flag] if flag == "--version" || flag == "-V" => {
            println!("{} {}", info.name, info.version);
        }
        [command] if command == "regtest-bitcoin-config" => {
            let config = BitcoinRegtestConfig::default();
            match config.connection_material_json() {
                Ok(json) => println!("{json}"),
                Err(err) => {
                    eprintln!("invalid default regtest config: {err}");
                    process::exit(1);
                }
            }
        }
        [command] if command == "lightning-labs-counterparty-config" => {
            let config = LightningLabsCounterpartyConfig::default();
            match config.connection_material_json() {
                Ok(json) => println!("{json}"),
                Err(err) => {
                    eprintln!("invalid default Lightning Labs counterparty config: {err}");
                    process::exit(1);
                }
            }
        }
        [command, base_dir] if command == "ldk-baseline-plan" => {
            let plan = BaselineLdkPlan::for_base_dir(base_dir);
            match plan.to_json() {
                Ok(json) => println!("{json}"),
                Err(err) => {
                    eprintln!("failed to render baseline LDK plan: {err}");
                    process::exit(1);
                }
            }
        }
        [command, state_path] if command == "ldk-baseline-smoke" => {
            let state = match BaselineBtcSmokeState::run_btc_only_smoke() {
                Ok(state) => state,
                Err(err) => {
                    eprintln!("failed baseline BTC-only smoke: {err}");
                    process::exit(1);
                }
            };
            if let Err(err) = state.save_atomic(state_path) {
                eprintln!("failed to save baseline smoke state {state_path}: {err}");
                process::exit(1);
            }
            println!(
                "baseline-btc-smoke settled_payment={} bob_restarts={} asset_channels_enabled={}",
                state
                    .payment
                    .as_ref()
                    .map(|payment| payment.payment_id.as_str())
                    .unwrap_or("none"),
                state.bob.restart_count,
                state.asset_channel_features_enabled
            );
        }
        [command, asset_id] if command == "asset-negotiation-smoke" => {
            let asset_id = parse_asset_id_or_exit(asset_id);
            let report = match run_negotiation_smoke(asset_id) {
                Ok(report) => report,
                Err(err) => {
                    eprintln!("failed asset negotiation smoke: {err}");
                    process::exit(1);
                }
            };
            match serde_json::to_string_pretty(&report) {
                Ok(json) => println!("{json}"),
                Err(err) => {
                    eprintln!("failed to render asset negotiation smoke: {err}");
                    process::exit(1);
                }
            }
        }
        [command, asset_id] if command == "asset-peer-message-smoke" => {
            let asset_id = parse_asset_id_or_exit(asset_id);
            let negotiation = match run_negotiation_smoke(asset_id) {
                Ok(report) => report,
                Err(err) => {
                    eprintln!("failed asset negotiation smoke: {err}");
                    process::exit(1);
                }
            };
            let report = match run_peer_message_smoke(&negotiation.asset_channel, asset_id) {
                Ok(report) => report,
                Err(err) => {
                    eprintln!("failed asset peer message smoke: {err}");
                    process::exit(1);
                }
            };
            match serde_json::to_string_pretty(&report) {
                Ok(json) => println!("{json}"),
                Err(err) => {
                    eprintln!("failed to render asset peer message smoke: {err}");
                    process::exit(1);
                }
            }
        }
        [command, store_path] if command == "rfq-store-init" => {
            let store = RfqQuoteStore::default();
            save_rfq_store_or_exit(store_path, &store);
            println!("initialized RFQ store {store_path}");
        }
        [command, store_path, scid] if command == "rfq-register-real-scid" => {
            let scid = parse_u64_or_exit(scid, "real local SCID");
            let mut store = load_rfq_store_or_default_or_exit(store_path);
            if let Err(err) = store.register_real_local_scid(scid) {
                eprintln!("failed to register real local SCID: {err}");
                process::exit(1);
            }
            save_rfq_store_or_exit(store_path, &store);
            println!("registered real local SCID {scid}");
        }
        [
            command,
            store_path,
            peer,
            asset_id,
            asset_amount,
            expiry_unix_seconds,
            invoice_context,
            replay_domain,
            now_unix_seconds,
        ] if command == "rfq-request" => {
            let mut store = load_rfq_store_or_default_or_exit(store_path);
            let request = RfqQuoteRequest {
                peer: peer.to_owned(),
                asset_id: parse_asset_id_or_exit(asset_id),
                asset_amount: parse_u64_or_exit(asset_amount, "asset amount"),
                expiry_unix_seconds: parse_u64_or_exit(expiry_unix_seconds, "expiry unix seconds"),
                invoice_context: parse_asset_id_or_exit(invoice_context),
                replay_domain: replay_domain.to_owned(),
                now_unix_seconds: parse_u64_or_exit(now_unix_seconds, "now unix seconds"),
            };
            let quote = match store.request_quote(request) {
                Ok(quote) => quote,
                Err(err) => {
                    eprintln!("failed to request RFQ quote: {err}");
                    process::exit(1);
                }
            };
            save_rfq_store_or_exit(store_path, &store);
            print_json_or_exit(&quote, "RFQ quote");
        }
        [command, store_path, quote_id, now_unix_seconds] if command == "rfq-accept" => {
            let mut store = load_rfq_store_or_exit(store_path);
            let now_unix_seconds = parse_u64_or_exit(now_unix_seconds, "now unix seconds");
            let quote = match store.accept_quote(quote_id, now_unix_seconds) {
                Ok(quote) => quote,
                Err(err) => {
                    eprintln!("failed to accept RFQ quote {quote_id}: {err}");
                    process::exit(1);
                }
            };
            save_rfq_store_or_exit(store_path, &store);
            print_json_or_exit(&quote, "RFQ quote");
        }
        [command, store_path, quote_id, now_unix_seconds] if command == "rfq-expire" => {
            let mut store = load_rfq_store_or_exit(store_path);
            let now_unix_seconds = parse_u64_or_exit(now_unix_seconds, "now unix seconds");
            let quote = match store.expire_quote(quote_id, now_unix_seconds) {
                Ok(quote) => quote,
                Err(err) => {
                    eprintln!("failed to expire RFQ quote {quote_id}: {err}");
                    process::exit(1);
                }
            };
            save_rfq_store_or_exit(store_path, &store);
            print_json_or_exit(&quote, "RFQ quote");
        }
        [command, store_path, quote_id, now_unix_seconds, reason] if command == "rfq-reject" => {
            let mut store = load_rfq_store_or_exit(store_path);
            let now_unix_seconds = parse_u64_or_exit(now_unix_seconds, "now unix seconds");
            let quote = match store.reject_quote(quote_id, now_unix_seconds, reason.to_owned()) {
                Ok(quote) => quote,
                Err(err) => {
                    eprintln!("failed to reject RFQ quote {quote_id}: {err}");
                    process::exit(1);
                }
            };
            save_rfq_store_or_exit(store_path, &store);
            print_json_or_exit(&quote, "RFQ quote");
        }
        [command, store_path, quote_id, now_unix_seconds] if command == "rfq-authorize-htlc" => {
            let mut store = load_rfq_store_or_exit(store_path);
            let now_unix_seconds = parse_u64_or_exit(now_unix_seconds, "now unix seconds");
            let authorization = match store.authorize_asset_htlc(quote_id, now_unix_seconds) {
                Ok(authorization) => authorization,
                Err(err) => {
                    eprintln!("failed to authorize RFQ HTLC for quote {quote_id}: {err}");
                    process::exit(1);
                }
            };
            save_rfq_store_or_exit(store_path, &store);
            print_json_or_exit(&authorization, "RFQ HTLC authorization");
        }
        [command, store_path, quote_id] if command == "rfq-quote" => {
            let store = load_rfq_store_or_exit(store_path);
            let quote = match store.inspect_quote(quote_id) {
                Ok(quote) => quote,
                Err(err) => {
                    eprintln!("failed to inspect RFQ quote {quote_id}: {err}");
                    process::exit(1);
                }
            };
            print_json_or_exit(&quote, "RFQ quote");
        }
        [command, store_path] if command == "rfq-quotes" => {
            let store = load_rfq_store_or_exit(store_path);
            print_json_or_exit(&store.quotes, "RFQ quotes");
        }
        [command, asset_id] if command == "rfq-invoice-smoke" => {
            let asset_id = parse_asset_id_or_exit(asset_id);
            let report = match run_rfq_invoice_smoke(asset_id) {
                Ok(report) => report,
                Err(err) => {
                    eprintln!("failed RFQ invoice smoke: {err}");
                    process::exit(1);
                }
            };
            print_json_or_exit(&report, "RFQ invoice smoke");
        }
        [command, store_path] if command == "asset-channel-funding-smoke" => {
            let (store, report) = match run_asset_channel_funding_smoke() {
                Ok(result) => result,
                Err(err) => {
                    eprintln!("failed asset-channel funding smoke: {err}");
                    process::exit(1);
                }
            };
            if let Err(err) = store.save_atomic(store_path) {
                eprintln!("failed to save asset-channel store {store_path}: {err}");
                process::exit(1);
            }
            print_json_or_exit(&report, "asset-channel funding smoke");
        }
        [command, store_path] if command == "asset-channel-list" => {
            let store = load_asset_channel_store_or_exit(store_path);
            print_json_or_exit(&store.channels, "asset channels");
        }
        [command, store_path, channel_id] if command == "asset-channel-balances" => {
            let store = load_asset_channel_store_or_exit(store_path);
            let balances = match store.channel_balances(channel_id) {
                Ok(balances) => balances,
                Err(err) => {
                    eprintln!("failed to load asset-channel balances {channel_id}: {err}");
                    process::exit(1);
                }
            };
            print_json_or_exit(&balances, "asset-channel balances");
        }
        [command, store_path] if command == "asset-commitment-smoke" => {
            let (store, report) = match run_asset_commitment_smoke() {
                Ok(result) => result,
                Err(err) => {
                    eprintln!("failed asset-commitment smoke: {err}");
                    process::exit(1);
                }
            };
            if let Err(err) = store.save_atomic(store_path) {
                eprintln!("failed to save asset-commitment store {store_path}: {err}");
                process::exit(1);
            }
            print_json_or_exit(&report, "asset-commitment smoke");
        }
        [command, store_path] if command == "asset-commitment-list" => {
            let store = load_asset_commitment_store_or_exit(store_path);
            print_json_or_exit(&store.channels, "asset commitment channels");
        }
        [command, store_path, channel_id] if command == "asset-commitment-state" => {
            let store = load_asset_commitment_store_or_exit(store_path);
            let state = match store.channel_state(channel_id) {
                Ok(state) => state,
                Err(err) => {
                    eprintln!("failed to load asset-commitment state {channel_id}: {err}");
                    process::exit(1);
                }
            };
            print_json_or_exit(&state, "asset-commitment state");
        }
        [command] if command == "asset-htlc-smoke" => {
            let (_htlc_store, _commitment_store, report) = match run_asset_htlc_smoke() {
                Ok(result) => result,
                Err(err) => {
                    eprintln!("failed asset-HTLC smoke: {err}");
                    process::exit(1);
                }
            };
            print_json_or_exit(&report, "asset-HTLC smoke");
        }
        [command] if command == "asset-payment-smoke" => {
            let (_payment_store, _commitment_store, _htlc_store, report) =
                match run_native_asset_payment_smoke() {
                    Ok(result) => result,
                    Err(err) => {
                        eprintln!("failed native asset payment smoke: {err}");
                        process::exit(1);
                    }
                };
            print_json_or_exit(&report, "native asset payment smoke");
        }
        [command] if command == "asset-recovery-smoke" => {
            let report = match run_native_asset_recovery_matrix_smoke() {
                Ok(report) => report,
                Err(err) => {
                    eprintln!("failed native asset recovery smoke: {err}");
                    process::exit(1);
                }
            };
            print_json_or_exit(&report, "native asset recovery smoke");
        }
        [command] if command == "asset-close-smoke" => {
            let report = match run_native_asset_close_smoke() {
                Ok(report) => report,
                Err(err) => {
                    eprintln!("failed native asset close smoke: {err}");
                    process::exit(1);
                }
            };
            print_json_or_exit(&report, "native asset close smoke");
        }
        [command, fixture_dir] if command == "lightning-labs-blob-fixture-smoke" => {
            let funding = read_fixture_hexdump_or_exit(fixture_dir, "funding-blob.hexdump");
            let htlc = read_fixture_hexdump_or_exit(fixture_dir, "htlc-blob.hexdump");
            let commitment = read_fixture_hexdump_or_exit(fixture_dir, "commitment-blob.hexdump");
            let report = match decode_fixture_hexdumps(&funding, &htlc, &commitment) {
                Ok(report) => report,
                Err(err) => {
                    eprintln!("failed Lightning Labs blob fixture smoke: {err}");
                    process::exit(1);
                }
            };
            print_json_or_exit(&report, "Lightning Labs blob fixture smoke");
        }
        [command, fixture_dir] if command == "lightning-labs-proof-fixture-smoke" => {
            let proof_file_hex = read_fixture_text_or_exit(fixture_dir, "proof-file.hex");
            let single_proof_hex = read_fixture_text_or_exit(fixture_dir, "proof.hex");
            let report = match decode_fixture_hex(&proof_file_hex, &single_proof_hex) {
                Ok(report) => report,
                Err(err) => {
                    eprintln!("failed Lightning Labs proof fixture smoke: {err}");
                    process::exit(1);
                }
            };
            print_json_or_exit(&report, "Lightning Labs proof fixture smoke");
        }
        [command, fixture_dir, store_path] if command == "lightning-labs-funding-interop-smoke" => {
            let funding = read_fixture_hexdump_or_exit(fixture_dir, "funding-blob.hexdump");
            let commitment = read_fixture_hexdump_or_exit(fixture_dir, "commitment-blob.hexdump");
            let (store, report) =
                match run_lightning_labs_funding_interop_fixture_smoke(&funding, &commitment) {
                    Ok(result) => result,
                    Err(err) => {
                        eprintln!("failed Lightning Labs funding interop smoke: {err}");
                        process::exit(1);
                    }
                };
            if let Err(err) = store.save_atomic(store_path) {
                eprintln!(
                    "failed to save Lightning Labs funding interop store {store_path}: {err}"
                );
                process::exit(1);
            }
            print_json_or_exit(&report, "Lightning Labs funding interop smoke");
        }
        [command, asset_id] if command == "lightning-labs-rfq-invoice-compat-smoke" => {
            let asset_id = parse_asset_id_or_exit(asset_id);
            let report = match run_lightning_labs_rfq_invoice_compat_smoke(asset_id) {
                Ok(report) => report,
                Err(err) => {
                    eprintln!("failed Lightning Labs RFQ invoice compatibility smoke: {err}");
                    process::exit(1);
                }
            };
            print_json_or_exit(&report, "Lightning Labs RFQ invoice compatibility smoke");
        }
        [command, fixture_dir, store_path]
            if command == "lightning-labs-outgoing-payment-smoke" =>
        {
            let funding = read_fixture_hexdump_or_exit(fixture_dir, "funding-blob.hexdump");
            let commitment = read_fixture_hexdump_or_exit(fixture_dir, "commitment-blob.hexdump");
            let (store, report) =
                match run_lightning_labs_outgoing_payment_smoke(&funding, &commitment) {
                    Ok(result) => result,
                    Err(err) => {
                        eprintln!("failed Lightning Labs outgoing payment smoke: {err}");
                        process::exit(1);
                    }
                };
            if let Err(err) = store.save_atomic(store_path) {
                eprintln!(
                    "failed to save Lightning Labs outgoing payment store {store_path}: {err}"
                );
                process::exit(1);
            }
            print_json_or_exit(&report, "Lightning Labs outgoing payment smoke");
        }
        [command, fixture_dir, store_path]
            if command == "lightning-labs-incoming-payment-smoke" =>
        {
            let funding = read_fixture_hexdump_or_exit(fixture_dir, "funding-blob.hexdump");
            let commitment = read_fixture_hexdump_or_exit(fixture_dir, "commitment-blob.hexdump");
            let (store, report) =
                match run_lightning_labs_incoming_payment_smoke(&funding, &commitment) {
                    Ok(result) => result,
                    Err(err) => {
                        eprintln!("failed Lightning Labs incoming payment smoke: {err}");
                        process::exit(1);
                    }
                };
            if let Err(err) = store.save_atomic(store_path) {
                eprintln!(
                    "failed to save Lightning Labs incoming payment store {store_path}: {err}"
                );
                process::exit(1);
            }
            print_json_or_exit(&report, "Lightning Labs incoming payment smoke");
        }
        [
            command,
            tapchannel_fixture_dir,
            proof_fixture_dir,
            report_path,
        ] if command == "lightning-labs-interop-check-smoke" => {
            let funding =
                read_fixture_hexdump_or_exit(tapchannel_fixture_dir, "funding-blob.hexdump");
            let commitment =
                read_fixture_hexdump_or_exit(tapchannel_fixture_dir, "commitment-blob.hexdump");
            let proof_file_hex = read_fixture_text_or_exit(proof_fixture_dir, "proof-file.hex");
            let single_proof_hex = read_fixture_text_or_exit(proof_fixture_dir, "proof.hex");
            let report = match run_lightning_labs_interop_check_smoke(
                &funding,
                &commitment,
                &proof_file_hex,
                &single_proof_hex,
                tapchannel_fixture_dir,
                proof_fixture_dir,
                report_path,
            ) {
                Ok(report) => report,
                Err(err) => {
                    eprintln!("failed Lightning Labs interop check smoke: {err}");
                    process::exit(1);
                }
            };
            save_json_or_exit(report_path, &report, "Lightning Labs interop check report");
            print_json_or_exit(&report, "Lightning Labs interop check smoke");
        }
        [command, wallet_path] if command == "wallet-init" => {
            let wallet = WalletState::default();
            if let Err(err) = wallet.save_atomic(wallet_path) {
                eprintln!("failed to initialize wallet: {err}");
                process::exit(1);
            }
            println!("initialized wallet {wallet_path}");
        }
        [command, wallet_path, amount, script_key] if command == "wallet-issue-openusd" => {
            let amount = parse_amount_or_exit(amount);
            let script_key = parse_script_key_or_exit(script_key, "issuer script key");
            let mut wallet = load_wallet_or_default_or_exit(wallet_path);
            let outcome = match wallet.issue_regtest_asset(RegtestIssueRequest::openusd(
                AssetAmount::new(amount),
                script_key,
            )) {
                Ok(outcome) => outcome,
                Err(err) => {
                    eprintln!("failed to issue OPENUSD: {err}");
                    process::exit(1);
                }
            };
            save_wallet_or_exit(wallet_path, &wallet);
            println!(
                "{} {} amount={} asset_id={} proof_id={}",
                outcome.status, outcome.ticker, outcome.amount, outcome.asset_id, outcome.proof_id
            );
        }
        [
            command,
            wallet_path,
            asset_id,
            amount,
            receiver_script_key,
            receiver_proof_path,
        ] if command == "wallet-send-local" => {
            let asset_id = parse_asset_id_or_exit(asset_id);
            let amount = parse_amount_or_exit(amount);
            let receiver_script_key =
                parse_script_key_or_exit(receiver_script_key, "receiver script key");
            let mut wallet = load_wallet_or_exit(wallet_path);
            let outcome = match wallet.send_local_transfer(LocalTransferRequest {
                asset_id,
                amount: AssetAmount::new(amount),
                receiver_script_key,
            }) {
                Ok(outcome) => outcome,
                Err(err) => {
                    eprintln!("failed to send local transfer: {err}");
                    process::exit(1);
                }
            };
            if let Err(err) = fs::write(receiver_proof_path, &outcome.receiver_proof_tlv) {
                eprintln!("failed to write receiver proof file {receiver_proof_path}: {err}");
                process::exit(1);
            }
            save_wallet_or_exit(wallet_path, &wallet);
            println!(
                "sent amount={} asset_id={} receiver_proof_id={} receiver_proof_file={} change_amount={} change_proof_id={}",
                outcome.sent_amount,
                outcome.asset_id,
                outcome.receiver_proof_id,
                receiver_proof_path,
                outcome.change_amount,
                outcome.change_proof_id.as_deref().unwrap_or("none")
            );
        }
        [command, proof_path] if command == "wallet-verify-proof-file" => {
            let encoded = match fs::read(proof_path) {
                Ok(encoded) => encoded,
                Err(err) => {
                    eprintln!("failed to read proof file {proof_path}: {err}");
                    process::exit(1);
                }
            };
            let proof = match ProofFile::decode(&encoded)
                .and_then(|proof| proof.verify_bounded_anchor().map(|()| proof))
            {
                Ok(proof) => proof,
                Err(err) => {
                    eprintln!("failed to verify proof file {proof_path}: {err}");
                    process::exit(1);
                }
            };
            println!(
                "verified proof asset_id={} amount={} anchor_outpoint={}",
                proof.asset_id.to_hex(),
                proof.amount.value(),
                proof.anchor_outpoint
            );
        }
        [command, wallet_path, proof_path] if command == "wallet-import-proof-file" => {
            let encoded = match fs::read(proof_path) {
                Ok(encoded) => encoded,
                Err(err) => {
                    eprintln!("failed to read proof file {proof_path}: {err}");
                    process::exit(1);
                }
            };
            import_encoded_proof(wallet_path, &encoded);
        }
        [command, wallet_path, proof_path] if command == "wallet-import-proof-fixture" => {
            let proof = match load_synthetic_proof_fixture(proof_path) {
                Ok(proof) => proof,
                Err(err) => {
                    eprintln!("failed to load proof fixture {proof_path}: {err}");
                    process::exit(1);
                }
            };
            let encoded = match proof.encode() {
                Ok(encoded) => encoded,
                Err(err) => {
                    eprintln!("failed to encode proof fixture {proof_path}: {err}");
                    process::exit(1);
                }
            };
            import_encoded_proof(wallet_path, &encoded);
        }
        [
            command,
            wallet_path,
            proof_path,
            asset_id,
            amount,
            script_key,
            genesis_outpoint,
            anchor_outpoint,
        ] if command == "wallet-import-tapd-proof-file" => {
            let tapd_proof_file = read_tapd_proof_file_or_exit(proof_path);
            let asset_id = parse_asset_id_or_exit(asset_id);
            let amount = parse_amount_or_exit(amount);
            let script_key = parse_script_key_or_exit(script_key, "owner script key");
            let mut wallet = load_wallet_or_default_or_exit(wallet_path);
            let outcome = match wallet.import_tapd_proof_file(TapdProofImportRequest {
                asset_id,
                genesis_outpoint: genesis_outpoint.to_owned(),
                anchor_outpoint: anchor_outpoint.to_owned(),
                amount: AssetAmount::new(amount),
                script_key,
                tapd_proof_file,
            }) {
                Ok(outcome) => outcome,
                Err(err) => {
                    eprintln!("failed to import tapd proof file: {err}");
                    process::exit(1);
                }
            };
            save_wallet_or_exit(wallet_path, &wallet);
            println!("{} tapd proof {}", outcome.status(), outcome.proof_id());
        }
        [command, wallet_path] if command == "wallet-balances" => {
            let wallet = load_wallet_or_exit(wallet_path);
            match wallet.balances().and_then(|balances| {
                serde_json::to_string_pretty(&balances)
                    .map_err(tap_ldk_core::wallet::WalletError::Json)
            }) {
                Ok(json) => println!("{json}"),
                Err(err) => {
                    eprintln!("failed to render balances: {err}");
                    process::exit(1);
                }
            }
        }
        [command, wallet_path] if command == "wallet-proofs" => {
            let wallet = load_wallet_or_exit(wallet_path);
            for proof_id in wallet.proofs.keys() {
                println!("{proof_id}");
            }
        }
        [command, wallet_path, proof_id, output_path] if command == "wallet-export-proof-file" => {
            let wallet = load_wallet_or_exit(wallet_path);
            let encoded = match wallet.export_encoded_proof(proof_id) {
                Ok(encoded) => encoded,
                Err(err) => {
                    eprintln!("failed to export proof {proof_id}: {err}");
                    process::exit(1);
                }
            };
            if let Err(err) = fs::write(output_path, encoded) {
                eprintln!("failed to write proof file {output_path}: {err}");
                process::exit(1);
            }
            println!("exported proof {proof_id} to {output_path}");
        }
        [command, wallet_path, proof_id, output_path]
            if command == "wallet-export-tapd-proof-file" =>
        {
            let wallet = load_wallet_or_exit(wallet_path);
            let encoded = match wallet.export_tapd_proof_file(proof_id) {
                Ok(encoded) => encoded,
                Err(err) => {
                    eprintln!("failed to export tapd proof {proof_id}: {err}");
                    process::exit(1);
                }
            };
            if let Err(err) = fs::write(output_path, encoded) {
                eprintln!("failed to write tapd proof file {output_path}: {err}");
                process::exit(1);
            }
            println!("exported tapd proof {proof_id} to {output_path}");
        }
        [unknown, ..] => {
            eprintln!("unknown argument: {unknown}");
            eprintln!("run `tap-ldk --help` for usage");
            process::exit(2);
        }
    }
}

fn print_help(info: ProjectInfo) {
    println!("{} {}", info.name, info.version);
    println!("{}", info.summary);
    println!();
    println!("Usage:");
    println!("  tap-ldk [--help]");
    println!("  tap-ldk --version");
    println!("  tap-ldk regtest-bitcoin-config");
    println!("  tap-ldk lightning-labs-counterparty-config");
    println!("  tap-ldk ldk-baseline-plan <base-dir>");
    println!("  tap-ldk ldk-baseline-smoke <state.json>");
    println!("  tap-ldk asset-negotiation-smoke <asset-id>");
    println!("  tap-ldk asset-peer-message-smoke <asset-id>");
    println!("  tap-ldk rfq-store-init <store.json>");
    println!("  tap-ldk rfq-register-real-scid <store.json> <real-local-scid>");
    println!(
        "  tap-ldk rfq-request <store.json> <peer> <asset-id> <asset-amount> <expiry-unix-seconds> <invoice-context> <replay-domain> <now-unix-seconds>"
    );
    println!("  tap-ldk rfq-accept <store.json> <quote-id> <now-unix-seconds>");
    println!("  tap-ldk rfq-reject <store.json> <quote-id> <now-unix-seconds> <reason>");
    println!("  tap-ldk rfq-expire <store.json> <quote-id> <now-unix-seconds>");
    println!("  tap-ldk rfq-authorize-htlc <store.json> <quote-id> <now-unix-seconds>");
    println!("  tap-ldk rfq-quote <store.json> <quote-id>");
    println!("  tap-ldk rfq-quotes <store.json>");
    println!("  tap-ldk rfq-invoice-smoke <asset-id>");
    println!("  tap-ldk asset-channel-funding-smoke <store.json>");
    println!("  tap-ldk asset-channel-list <store.json>");
    println!("  tap-ldk asset-channel-balances <store.json> <channel-id>");
    println!("  tap-ldk asset-commitment-smoke <store.json>");
    println!("  tap-ldk asset-commitment-list <store.json>");
    println!("  tap-ldk asset-commitment-state <store.json> <channel-id>");
    println!("  tap-ldk asset-htlc-smoke");
    println!("  tap-ldk asset-payment-smoke");
    println!("  tap-ldk asset-recovery-smoke");
    println!("  tap-ldk asset-close-smoke");
    println!("  tap-ldk lightning-labs-blob-fixture-smoke <fixture-dir>");
    println!("  tap-ldk lightning-labs-proof-fixture-smoke <fixture-dir>");
    println!("  tap-ldk lightning-labs-funding-interop-smoke <fixture-dir> <store.json>");
    println!("  tap-ldk lightning-labs-rfq-invoice-compat-smoke <asset-id>");
    println!("  tap-ldk lightning-labs-outgoing-payment-smoke <fixture-dir> <store.json>");
    println!("  tap-ldk lightning-labs-incoming-payment-smoke <fixture-dir> <store.json>");
    println!(
        "  tap-ldk lightning-labs-interop-check-smoke <tapchannel-fixture-dir> <proof-fixture-dir> <report.json>"
    );
    println!("  tap-ldk wallet-init <wallet.json>");
    println!("  tap-ldk wallet-issue-openusd <wallet.json> <amount> <issuer-script-key>");
    println!(
        "  tap-ldk wallet-send-local <wallet.json> <asset-id> <amount> <receiver-script-key> <receiver-proof.tlv>"
    );
    println!("  tap-ldk wallet-verify-proof-file <proof.tlv>");
    println!("  tap-ldk wallet-import-proof-file <wallet.json> <proof.tlv>");
    println!("  tap-ldk wallet-import-proof-fixture <wallet.json> <proof.json>");
    println!(
        "  tap-ldk wallet-import-tapd-proof-file <wallet.json> <tapd-proof-file> <asset-id> <amount> <owner-script-key> <genesis-outpoint> <anchor-outpoint>"
    );
    println!("  tap-ldk wallet-balances <wallet.json>");
    println!("  tap-ldk wallet-proofs <wallet.json>");
    println!("  tap-ldk wallet-export-proof-file <wallet.json> <proof-id> <proof.tlv>");
    println!("  tap-ldk wallet-export-tapd-proof-file <wallet.json> <proof-id> <tapd-proof-file>");
}

fn import_encoded_proof(wallet_path: &str, encoded: &[u8]) {
    let mut wallet = load_wallet_or_default_or_exit(wallet_path);
    let outcome = match wallet.import_encoded_proof(encoded) {
        Ok(outcome) => outcome,
        Err(err) => {
            eprintln!("failed to import proof: {err}");
            process::exit(1);
        }
    };
    save_wallet_or_exit(wallet_path, &wallet);

    println!("{} proof {}", outcome.status(), outcome.proof_id());
}

fn load_wallet_or_default_or_exit(wallet_path: &str) -> WalletState {
    match WalletState::load_or_default(wallet_path) {
        Ok(wallet) => wallet,
        Err(err) => {
            eprintln!("failed to load wallet {wallet_path}: {err}");
            process::exit(1);
        }
    }
}

fn load_wallet_or_exit(wallet_path: &str) -> WalletState {
    match WalletState::load(wallet_path) {
        Ok(wallet) => wallet,
        Err(err) => {
            eprintln!("failed to load wallet {wallet_path}: {err}");
            process::exit(1);
        }
    }
}

fn save_wallet_or_exit(wallet_path: &str, wallet: &WalletState) {
    if let Err(err) = wallet.save_atomic(wallet_path) {
        eprintln!("failed to save wallet {wallet_path}: {err}");
        process::exit(1);
    }
}

fn load_rfq_store_or_default_or_exit(store_path: &str) -> RfqQuoteStore {
    match RfqQuoteStore::load_or_default(store_path) {
        Ok(store) => store,
        Err(err) => {
            eprintln!("failed to load RFQ store {store_path}: {err}");
            process::exit(1);
        }
    }
}

fn load_rfq_store_or_exit(store_path: &str) -> RfqQuoteStore {
    match RfqQuoteStore::load(store_path) {
        Ok(store) => store,
        Err(err) => {
            eprintln!("failed to load RFQ store {store_path}: {err}");
            process::exit(1);
        }
    }
}

fn save_rfq_store_or_exit(store_path: &str, store: &RfqQuoteStore) {
    if let Err(err) = store.save_atomic(store_path) {
        eprintln!("failed to save RFQ store {store_path}: {err}");
        process::exit(1);
    }
}

fn load_asset_channel_store_or_exit(store_path: &str) -> AssetChannelStore {
    match AssetChannelStore::load(store_path) {
        Ok(store) => store,
        Err(err) => {
            eprintln!("failed to load asset-channel store {store_path}: {err}");
            process::exit(1);
        }
    }
}

fn load_asset_commitment_store_or_exit(store_path: &str) -> AssetCommitmentStore {
    match AssetCommitmentStore::load(store_path) {
        Ok(store) => store,
        Err(err) => {
            eprintln!("failed to load asset-commitment store {store_path}: {err}");
            process::exit(1);
        }
    }
}

fn read_fixture_hexdump_or_exit(fixture_dir: &str, file_name: &str) -> String {
    read_fixture_text_or_exit(fixture_dir, file_name)
}

fn read_fixture_text_or_exit(fixture_dir: &str, file_name: &str) -> String {
    let path = Path::new(fixture_dir).join(file_name);
    match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(err) => {
            eprintln!("failed to read fixture {}: {err}", path.display());
            process::exit(1);
        }
    }
}

fn read_tapd_proof_file_or_exit(path: &str) -> Vec<u8> {
    let raw = match fs::read(path) {
        Ok(raw) => raw,
        Err(err) => {
            eprintln!("failed to read tapd proof file {path}: {err}");
            process::exit(1);
        }
    };
    if decode_tapd_proof_file(&raw).is_ok() {
        return raw;
    }

    let text = match String::from_utf8(raw) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("tapd proof file {path} is neither raw TAPF nor UTF-8 hex: {err}");
            process::exit(1);
        }
    };
    match decode_hex_text(&text).and_then(|bytes| decode_tapd_proof_file(&bytes).map(|_| bytes)) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("failed to decode tapd proof file {path}: {err}");
            process::exit(1);
        }
    }
}

fn print_json_or_exit<T: Serialize>(value: &T, label: &str) {
    match serde_json::to_string_pretty(value) {
        Ok(json) => println!("{json}"),
        Err(err) => {
            eprintln!("failed to render {label}: {err}");
            process::exit(1);
        }
    }
}

fn save_json_or_exit<T: Serialize>(path: &str, value: &T, label: &str) {
    let path_ref = Path::new(path);
    if let Some(parent) = path_ref.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(err) = fs::create_dir_all(parent) {
                eprintln!("failed to create parent directory for {path}: {err}");
                process::exit(1);
            }
        }
    }
    let raw = match serde_json::to_vec_pretty(value) {
        Ok(raw) => raw,
        Err(err) => {
            eprintln!("failed to render {label}: {err}");
            process::exit(1);
        }
    };
    if let Err(err) = fs::write(path_ref, raw) {
        eprintln!("failed to write {label} {path}: {err}");
        process::exit(1);
    }
}

fn parse_amount_or_exit(value: &str) -> u64 {
    match value.parse::<u64>() {
        Ok(amount) => amount,
        Err(err) => {
            eprintln!("invalid amount {value}: {err}");
            process::exit(2);
        }
    }
}

fn parse_u64_or_exit(value: &str, field: &str) -> u64 {
    match value.parse::<u64>() {
        Ok(amount) => amount,
        Err(err) => {
            eprintln!("invalid {field} {value}: {err}");
            process::exit(2);
        }
    }
}

fn parse_asset_id_or_exit(value: &str) -> Bytes32 {
    match Bytes32::from_str(value) {
        Ok(asset_id) => asset_id,
        Err(err) => {
            eprintln!("invalid asset id {value}: {err}");
            process::exit(2);
        }
    }
}

fn parse_script_key_or_exit(value: &str, field: &str) -> CompressedKey {
    match CompressedKey::from_str(value) {
        Ok(script_key) => script_key,
        Err(err) => {
            eprintln!("invalid {field} {value}: {err}");
            process::exit(2);
        }
    }
}

fn load_synthetic_proof_fixture(path: &str) -> Result<ProofFile, String> {
    let raw = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let fixture =
        serde_json::from_str::<SyntheticProofFixture>(&raw).map_err(|err| err.to_string())?;
    Ok(ProofFile {
        version: 0,
        asset_id: Bytes32::from_str(&fixture.asset_id).map_err(|err| err.to_string())?,
        genesis_outpoint: fixture.genesis_outpoint,
        anchor_outpoint: fixture.anchor_outpoint,
        amount: AssetAmount::new(fixture.amount),
        script_key: CompressedKey::from_str(&fixture.script_key).map_err(|err| err.to_string())?,
        tap_asset_root: RootHashSum {
            hash: Bytes32::from_str(&fixture.tap_asset_root.hash).map_err(|err| err.to_string())?,
            sum: AssetAmount::new(fixture.tap_asset_root.sum),
        },
        verification_scope: VerificationScope::from_str(&fixture.verification_scope)
            .map_err(|err| err.to_string())?,
    })
}

#[derive(Debug, Deserialize)]
struct SyntheticProofFixture {
    asset_id: String,
    genesis_outpoint: String,
    anchor_outpoint: String,
    amount: u64,
    script_key: String,
    tap_asset_root: SyntheticRoot,
    verification_scope: String,
}

#[derive(Debug, Deserialize)]
struct SyntheticRoot {
    hash: String,
    sum: u64,
}
