use clap::Args;
use eyre::Result;
use std::path::PathBuf;

use super::GlobalArgs;
use crate::vault::Vault;

#[derive(Args)]
pub struct CliArgs {
    backup_name: Option<String>,

    /// Print full output
    #[arg(short = 'f')]
    full: bool,
}

pub fn run(gargs: GlobalArgs, args: CliArgs) -> Result<()> {
    list(gargs.vault_dir, args.backup_name, args.full)
}

fn list(
    provided_vault_dir: Option<PathBuf>,
    backup_name: Option<String>,
    full: bool,
) -> Result<()> {
    let vault = Vault::open_cwd(provided_vault_dir)?;

    match backup_name {
        Some(name) => list_backup(&vault, &name, full),
        None => list_all_backups(&vault, full),
    }

    Ok(())
}

fn list_all_backups(vault: &Vault, full: bool) {
    if vault.database.iter_backups().len() == 0 {
        println!("No backups.");
        return;
    }

    println!("Backups:");
    for (name, bkup) in vault.database.iter_backups() {
        println!("- {name}");
        if !full {
            let n_files = bkup.iter_files().len();
            let n_dirs = bkup.iter_directories().len();
            let n_links = bkup.iter_symlinks().len();
            if n_files > 0 {
                println!("  files:       {n_files}");
            }
            if n_dirs > 0 {
                println!("  directories: {n_dirs}");
            }
            if n_links > 0 {
                println!("  symlinks:    {n_links}");
            }
        } else {
            todo!()
        }
    }
}

fn list_backup(vault: &Vault, backup_name: &str, full: bool) {
    todo!()
}
