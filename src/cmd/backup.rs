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
        backup::{Backup, BackupView},
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

fn backup(provided_vault_dir: Option<PathBuf>, bkup_name: &str, bkup_root: &Path) -> Result<()> {
    let mut vault = Vault::open_cwd(provided_vault_dir)?;

    match vault.database.get_backup(bkup_name) {
        Some(old_bkup) => {
            let (backup, new_files) = update_existing_backup(bkup_root, old_bkup)?;
        }
        None => (),
    }

    // Ingest new files into storage
    // for NewBackupFile { source, hash, .. } in state.new_bkup.iter_new_files() {
    //     storage
    //         .insert_file(source, *hash)
    //         .context_2("storage::insert_file", source)?;
    // }

    // // Insert new backup into database
    // db.insert_backup_builder(bkup_name, state.new_bkup);
    // db.write(&vault_dir)?;

    vault.close()?;
    Ok(())
}

fn new_backup(root: &Path) -> Result<Backup> {
    let backup = scan_dir_into_backup(root, |path, _, _| Hash::of_file(path))?;
    Ok(backup)
}

fn update_existing_backup(root: &Path, old: BackupView) -> Result<(Backup, Vec<NewFile>)> {
    let mut new_files = Vec::new();
    let backup = scan_dir_into_backup(root, |path, ino, mtime| {
        match old.get_file(ino) {
            // A prior file exists with the same inode and a lower mtime.
            // From, this, we assume that the file has not changed and reuse the old hash.
            Some(old_f) if mtime <= old_f.mtime() => Ok(old_f.hash()),

            // A prior file exists with the same inode but a newer mtime.
            // We need to hash the file to check if it has changed.
            Some(old_f) => {
                let new_hash = Hash::of_file(path)?;
                if new_hash != old_f.hash() {
                    new_files.push(NewFile {
                        path: path.to_owned(),
                        hash: new_hash,
                    });
                }
                Ok(new_hash)
            }

            // This inode was never seen before - we must hash it.
            // It may be the a file with identical contents of another,
            // meaning it is technically not "new" as far as storage is concerned.
            // This leads to a minor amount of excess work in new file insertion.
            None => {
                let hash = Hash::of_file(path)?;
                new_files.push(NewFile {
                    path: path.to_owned(),
                    hash,
                });
                Ok(hash)
            }
        }
    })?;
    Ok((backup, new_files))
}

struct NewFile {
    path: PathBuf,
    hash: Hash,
}

fn scan_dir_into_backup<F>(root: &Path, mut file_hash_hook: F) -> Result<Backup>
where
    F: FnMut(&Path, u64, MTime) -> io::Result<Hash>,
{
    let mut backup = Backup::new();
    for dir_entry in WalkDir::new(root).min_depth(1) {
        let dir_entry = dir_entry?;
        let ino = dir_entry.ino();
        let path = dir_entry.into_path();
        let metadata = symlink_metadata(&*path)?;
        let path_from_root = path.strip_prefix(root).unwrap().to_path_buf();
        if metadata.is_file() {
            let mtime = MTime::from(metadata.modified().path_context(&path)?);
            let hash = file_hash_hook(&path, ino, mtime).path_context(&path)?;
            backup.insert_file(path_from_root, ino, mtime, hash)
        } else if metadata.is_dir() {
            backup.insert_directory(path_from_root);
        } else if metadata.is_symlink() {
            let target = read_link(&*path).path_context(&path)?;
            backup.insert_symlink(target, path_from_root);
        } else {
            bail!("{}: special file", path.display());
        };
    }
    Ok(backup)
}
