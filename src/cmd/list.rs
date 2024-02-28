use super::GlobalArgs;

use clap::Args;
use eyre::Result;
use std::path::PathBuf;

#[derive(Args)]
pub struct CliArgs {}

pub fn run(gargs: GlobalArgs, _args: CliArgs) -> Result<()> {
    list(gargs.vault_dir)
}

fn list(provided_vault_dir: Option<PathBuf>) -> Result<()> {
    // let vault_dir = path_or_cwd(provided_vault_dir);
    // let db = Database::load(vault_dir)?;

    // for bkup in db.iter_backups() {
    //     print_bkup_info(&db, &bkup);
    // }

    Ok(())
}

// fn print_bkup_info(_db: &Database, bkup: &Backup) {
//     println!("* {}", bkup.name());
//     let n_items = bkup.files().len() + bkup.directories().len() + bkup.symlinks().len();
//     println!("  {n_items} items");
// }
