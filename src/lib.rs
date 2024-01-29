#![allow(dead_code)]

mod cmd;
mod util;
mod vault;

pub fn main() -> ! {
    cmd::cli_main()
}

pub fn main_with_args(args: &[&str]) -> eyre::Result<()> {
    cmd::cli_from_args(args)
}
