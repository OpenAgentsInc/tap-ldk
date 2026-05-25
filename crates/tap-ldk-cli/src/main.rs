use std::{env, fs, process, str::FromStr};

use serde::Deserialize;

use tap_ldk_core::{
    ProjectInfo,
    asset::{AssetAmount, Bytes32, CompressedKey, RootHashSum},
    asset_channel_negotiation::run_negotiation_smoke,
    ldk_baseline::{BaselineBtcSmokeState, BaselineLdkPlan},
    proof::{ProofFile, VerificationScope},
    regtest::{BitcoinRegtestConfig, LightningLabsCounterpartyConfig},
    wallet::{LocalTransferRequest, RegtestIssueRequest, WalletState},
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
    println!("  tap-ldk wallet-init <wallet.json>");
    println!("  tap-ldk wallet-issue-openusd <wallet.json> <amount> <issuer-script-key>");
    println!(
        "  tap-ldk wallet-send-local <wallet.json> <asset-id> <amount> <receiver-script-key> <receiver-proof.tlv>"
    );
    println!("  tap-ldk wallet-verify-proof-file <proof.tlv>");
    println!("  tap-ldk wallet-import-proof-file <wallet.json> <proof.tlv>");
    println!("  tap-ldk wallet-import-proof-fixture <wallet.json> <proof.json>");
    println!("  tap-ldk wallet-balances <wallet.json>");
    println!("  tap-ldk wallet-proofs <wallet.json>");
    println!("  tap-ldk wallet-export-proof-file <wallet.json> <proof-id> <proof.tlv>");
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

fn parse_amount_or_exit(value: &str) -> u64 {
    match value.parse::<u64>() {
        Ok(amount) => amount,
        Err(err) => {
            eprintln!("invalid amount {value}: {err}");
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
