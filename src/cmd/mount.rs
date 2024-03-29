use clap::Args;
use eyre::{Context, ContextCompat, Result};
use path_absolutize::Absolutize;
use std::{
    env::current_dir,
    fs::create_dir_all,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
};

use crate::{
    util::{ensure_dir_exists_and_is_empty, ContextExt},
    vault::Vault,
};

use super::GlobalArgs;

#[derive(Args)]
pub struct CliArgs {
    backup_name: String,
    mount_point: PathBuf,
}

pub fn run(gargs: GlobalArgs, args: CliArgs) -> Result<()> {
    mount(gargs.vault_dir, &args.mount_point, &args.backup_name)
}

fn mount(vault_dir: Option<PathBuf>, mount_point: &Path, backup: &str) -> Result<()> {
    ensure_dir_exists_and_is_empty(mount_point)?;
    let vault = Vault::open(vault_dir)?;
    let bkup = vault
        .database
        .get_backup(backup)
        .with_context(|| format!("backup {backup:?} does not exist"))?;

    // create the directory structure
    for dir in bkup.iter_directories() {
        let dir_dest = mount_point.join(dir);
        create_dir_all(&dir_dest).context_2("mkdir", dir_dest)?;
    }

    let cwd = current_dir().expect("current_dir");

    // symlink the stored files into the directories
    for file in bkup.iter_files() {
        let file_dest = mount_point.join(&file.path);
        let file_dest = file_dest
            .absolutize_from(&cwd)
            .context_2("absolutize", &file_dest)?;

        let file_source = vault.storage.path_of(file.hash);
        let file_source = file_source
            .absolutize_from(&cwd)
            .context_2("absolutize", &file_source)?;
        let file_source = pathdiff::diff_paths(file_source, file_dest.parent().unwrap()).unwrap();

        symlink(&file_source, &file_dest).with_context(|| {
            format!(
                "symlinking {} -> {}",
                file_source.display(),
                file_dest.display()
            )
        })?;
    }

    // create the backed-up symlinks in the directories
    for (link_name, target) in bkup.iter_symlinks() {
        let link_dest = mount_point.join(link_name);
        symlink(&link_dest, target).with_context(|| {
            format!("symlinking {} -> {}", link_dest.display(), target.display())
        })?;
    }

    Ok(())
}
