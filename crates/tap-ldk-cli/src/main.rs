use std::{env, process};

use tap_ldk_core::{
    ProjectInfo,
    regtest::{BitcoinRegtestConfig, LightningLabsCounterpartyConfig},
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
}
