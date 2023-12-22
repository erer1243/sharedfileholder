use clap::Args;
use eyre::Result;
use std::path::PathBuf;

use crate::{database::Database, path_or_cwd};

use super::GlobalArgs;

#[derive(Args)]
pub struct CliArgs {
    vault_dir: Option<PathBuf>,
}

pub fn run(gargs: GlobalArgs, args: CliArgs) -> Result<()> {
    Ok(())
}

pub fn list(provided_vault_dir: Option<PathBuf>) -> Result<()> {
    let vault_dir = path_or_cwd(provided_vault_dir);
    let db = Database::load_from_vault(vault_dir)?;

    Ok(())
}
