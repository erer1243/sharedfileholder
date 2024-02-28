use clap::Args;
use eyre::{bail, Result};
use std::{
    fs::{read_link, symlink_metadata},
    io,
    path::{Path, PathBuf},
};

use walkdir::{DirEntryExt, WalkDir};

use crate::{
    cmd::GlobalArgs,
    util::{ContextExt, Hash, MTime},
    vault::{
        backup::{Backup, BackupFile},
        Vault,
    },
};

#[derive(Args)]
pub struct CliArgs {
    backup_name: String,
    backup_source_dir: PathBuf,
}

pub fn run(gargs: GlobalArgs, args: CliArgs) -> Result<()> {
    backup(gargs.vault_dir, &args.backup_name, &args.backup_source_dir)
}

type NewFile = (PathBuf, Hash);

fn backup(provided_vault_dir: Option<PathBuf>, bkup_name: &str, bkup_root: &Path) -> Result<()> {
    let mut vault = Vault::open_cwd(provided_vault_dir)?;
    let old_bkup = vault.database.get_backup(bkup_name);
    let (backup, new_files) = match old_bkup {
        Some(old_bkup) => update_existing_backup(bkup_root, old_bkup)?,
        None => new_backup(bkup_root)?,
    };
    vault.storage.insert_iter(new_files)?;
    vault.database.insert_backup(bkup_name, backup);
    vault.database.write()?;
    Ok(())
}

fn new_backup(root: &Path) -> Result<(Backup, Vec<NewFile>)> {
    scan_dir_into_backup(root, |path, _, _| Ok((Hash::of_file(path)?, true)))
}

fn update_existing_backup(root: &Path, old: &Backup) -> Result<(Backup, Vec<NewFile>)> {
    scan_dir_into_backup(root, |path, ino, mtime| {
        match old.get_file(ino) {
            // A prior file exists with the same inode and a lower mtime.
            // From, this, we assume that the file has not changed and reuse the old hash.
            Some(old) if mtime <= old.mtime => Ok((old.hash, false)),

            // A prior file exists with the same inode but a newer mtime.
            // We need to hash the file to check if it has changed.
            Some(old) => {
                let new_hash = Hash::of_file(path)?;
                if new_hash != old.hash {
                    Ok((new_hash, true))
                } else {
                    Ok((new_hash, false))
                }
            }

            // This inode was never seen before - we must hash it.
            // It may be the a file with identical contents of another,
            // meaning it is technically not "new" as far as storage is concerned.
            // This leads to a minor amount of excess work in new file insertion.
            None => Ok((Hash::of_file(path)?, true)),
        }
    })
}

fn scan_dir_into_backup<F>(root: &Path, mut file_hook: F) -> Result<(Backup, Vec<NewFile>)>
where
    // (path, inode, mtime) -> result<(file_hash, is_file_new)>
    F: FnMut(&Path, u64, MTime) -> io::Result<(Hash, bool)>,
{
    let mut backup = Backup::new();
    let mut new_files = Vec::new();
    for dir_entry in WalkDir::new(root).min_depth(1) {
        let dir_entry = dir_entry?;
        let ino = dir_entry.ino();
        let path = dir_entry.into_path();
        let metadata = symlink_metadata(&*path)?;
        let path_from_root = path.strip_prefix(root).unwrap().to_path_buf();
        if metadata.is_file() {
            let mtime = MTime::from(metadata.modified().path_context(&path)?);
            let (hash, is_new) = file_hook(&path, ino, mtime).path_context(&path)?;
            if is_new {
                new_files.push((path, hash));
            }
            backup.insert_file(BackupFile {
                path: path_from_root,
                ino,
                hash,
                mtime,
            })
        } else if metadata.is_dir() {
            backup.insert_directory(path_from_root);
        } else if metadata.is_symlink() {
            let target = read_link(&*path).path_context(&path)?;
            backup.insert_symlink(target, path_from_root);
        } else {
            bail!("{}: special file", path.display());
        };
    }
    Ok((backup, new_files))
}
