#![allow(dead_code)]

mod backup;
mod cmd;
mod database;
mod fieldmap;
mod storage;
mod util;

pub fn main() -> ! {
    cmd::cli_main()
}

pub fn main_with_args(args: &[&str]) -> eyre::Result<()> {
    cmd::cli_from_args(args)
}
