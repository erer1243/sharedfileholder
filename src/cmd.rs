mod backup;
mod init;
mod list;
mod mount;

use clap::{Args, Parser, Subcommand};
use eyre::Result;
use std::{path::PathBuf, process::exit};

#[derive(Args)]
pub struct GlobalArgs {
    #[arg(short, help = "Vault Directory", global = true)]
    vault_dir: Option<PathBuf>,
}

#[derive(Parser)]
pub struct Cli {
    #[command(flatten)]
    global_args: GlobalArgs,

    #[command(subcommand)]
    subcommand: SubCmd,
}

#[derive(Subcommand)]
enum SubCmd {
    Init(init::CliArgs),
    Backup(backup::CliArgs),
    List(list::CliArgs),
    Mount(mount::CliArgs),
}

pub fn cli_main() -> ! {
    if let Err(e) = run_cli(Cli::parse()) {
        eprintln!("[error] {e:#}");
        exit(1)
    } else {
        exit(0)
    }
}

pub fn cli_from_args(args: &[&str]) -> Result<()> {
    // tack on a binary name, for argv[0]
    let args = std::iter::once(&"cli_from_args").chain(args);
    let cli = Cli::parse_from(args);
    run_cli(cli)
}

fn run_cli(cli: Cli) -> Result<()> {
    let Cli {
        global_args,
        subcommand,
    } = cli;

    match subcommand {
        SubCmd::Init(args) => init::run(global_args, args),
        SubCmd::Backup(args) => backup::run(global_args, args),
        SubCmd::List(args) => list::run(global_args, args),
        SubCmd::Mount(args) => mount::run(global_args, args),
    }
}
