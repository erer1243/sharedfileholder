use std::fs::create_dir;

use clap::Args;
use eyre::Result;

use super::GlobalArgs;
use crate::{
    util::{ensure_dir_exists_and_is_empty, path_or_cwd},
    vault::database::Database,
};

#[derive(Args)]
pub struct CliArgs {}

pub fn run(gargs: GlobalArgs, _args: CliArgs) -> Result<()> {
    let vault_dir = &path_or_cwd(gargs.vault_dir);
    ensure_dir_exists_and_is_empty(vault_dir)?;
    create_dir(vault_dir.join("data"))?;
    Database::new().write(vault_dir)
}
