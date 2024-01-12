use clap::Args;
use eyre::Result;

use super::GlobalArgs;
use crate::{
    database::Database,
    util::{ensure_dir_exists_and_is_empty, path_or_cwd},
};

#[derive(Args)]
pub struct CliArgs {}

pub fn run(gargs: GlobalArgs, _args: CliArgs) -> Result<()> {
    let vault_dir = &path_or_cwd(gargs.vault_dir);
    ensure_dir_exists_and_is_empty(vault_dir)?;
    Database::new().write_to_vault(vault_dir)
}
