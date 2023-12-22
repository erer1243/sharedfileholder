use clap::Args;
use eyre::{ensure, Context, Result};
use std::{fs::read_dir, path::PathBuf};

use crate::{database::Database, path_or_cwd};

use super::GlobalArgs;

#[derive(Args)]
pub struct CliArgs {}

pub fn run(gargs: GlobalArgs, _args: CliArgs) -> Result<()> {
    init(gargs.vault_dir)
}

pub fn init(vault_dir: Option<PathBuf>) -> Result<()> {
    let dir = path_or_cwd(vault_dir);

    // Check that dir is empty
    let mut read_dir = read_dir(&dir).context("read_dir")?;
    ensure!(read_dir.next().is_none(), "{} is not empty", dir.display());

    // Make empty database file
    Database::default().write(dir.join("database.ron"))
}
