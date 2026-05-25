use std::{env, fs, process, str::FromStr};

use serde::Deserialize;

use tap_ldk_core::{
    ProjectInfo,
    asset::{AssetAmount, Bytes32, CompressedKey, RootHashSum},
    proof::{ProofFile, VerificationScope},
    regtest::{BitcoinRegtestConfig, LightningLabsCounterpartyConfig},
    wallet::WalletState,
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
        [command, wallet_path] if command == "wallet-init" => {
            let wallet = WalletState::default();
            if let Err(err) = wallet.save_atomic(wallet_path) {
                eprintln!("failed to initialize wallet: {err}");
                process::exit(1);
            }
            println!("initialized wallet {wallet_path}");
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
    println!("  tap-ldk wallet-init <wallet.json>");
    println!("  tap-ldk wallet-import-proof-file <wallet.json> <proof.tlv>");
    println!("  tap-ldk wallet-import-proof-fixture <wallet.json> <proof.json>");
    println!("  tap-ldk wallet-balances <wallet.json>");
    println!("  tap-ldk wallet-proofs <wallet.json>");
    println!("  tap-ldk wallet-export-proof-file <wallet.json> <proof-id> <proof.tlv>");
}

fn import_encoded_proof(wallet_path: &str, encoded: &[u8]) {
    let mut wallet = match WalletState::load_or_default(wallet_path) {
        Ok(wallet) => wallet,
        Err(err) => {
            eprintln!("failed to load wallet {wallet_path}: {err}");
            process::exit(1);
        }
    };
    let outcome = match wallet.import_encoded_proof(encoded) {
        Ok(outcome) => outcome,
        Err(err) => {
            eprintln!("failed to import proof: {err}");
            process::exit(1);
        }
    };
    if let Err(err) = wallet.save_atomic(wallet_path) {
        eprintln!("failed to save wallet {wallet_path}: {err}");
        process::exit(1);
    }

    println!("{} proof {}", outcome.status(), outcome.proof_id());
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
