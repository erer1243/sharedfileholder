// #![allow(unused)]

mod database;
mod storage;
mod subcommands;

use clap::{Parser, Subcommand};
use eyre::{Context, Result};
use std::{
    env::current_dir,
    path::{Path, PathBuf},
    process::exit,
};
use subcommands::{backup, init, list, GlobalArgs};

#[derive(Parser)]
struct Cli {
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
}

fn main() {
    let Cli {
        global_args,
        subcommand,
    } = Cli::parse();

    let res = match subcommand {
        SubCmd::Init(args) => init::run(global_args, args),
        SubCmd::Backup(args) => backup::run(global_args, args),
        SubCmd::List(args) => subcommands::list::run(global_args, args),
    };

    if let Err(e) = res {
        eprintln!("[error] {e:#}");
        exit(1);
    }
}

trait ContextExt<T, E>: Context<T, E> + Sized {
    fn context_2<P: AsRef<Path>>(self, msg: &str, path: P) -> Result<T> {
        self.with_context(|| format!("{msg} ({})", path.as_ref().display()))
    }
}

impl<C: Context<T, E>, T, E> ContextExt<T, E> for C {}

fn path_or_cwd(path: Option<PathBuf>) -> PathBuf {
    path.unwrap_or_else(|| current_dir().expect("current_dir"))
}
