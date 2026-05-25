use std::{env, process};

use tap_ldk_core::ProjectInfo;

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
}
