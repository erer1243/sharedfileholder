use clap::Args;
use eyre::{bail, Context, Result};
use std::{
    fs::read_link,
    path::{Path, PathBuf},
};
use walkdir::{DirEntry as WalkDirEntry, DirEntryExt, WalkDir};

use crate::{
    backup::{BackupBuilder, BackupView, NewBackupFile},
    database::Database,
    storage,
    util::{path_or_cwd, ContextExt, Hash, MTime},
};

use super::GlobalArgs;

#[derive(Args)]
pub struct CliArgs {
    backup_name: String,
    backup_source_dir: PathBuf,
}

struct BackupState<'a, 'b> {
    bkup_root: &'a Path,
    old_bkup: Option<BackupView<'b>>,
    new_bkup: BackupBuilder,
}

pub fn run(gargs: GlobalArgs, args: CliArgs) -> Result<()> {
    backup(gargs.vault_dir, &args.backup_name, &args.backup_source_dir)
}

fn backup(provided_vault_dir: Option<PathBuf>, bkup_name: &str, bkup_root: &Path) -> Result<()> {
    let vault_dir = path_or_cwd(provided_vault_dir);
    let mut db = Database::load_from_vault(&vault_dir).context_2("loading vault", &vault_dir)?;
    let old_bkup = db.get_backup(bkup_name);
    let new_bkup = BackupBuilder::new();
    let mut state = BackupState {
        bkup_root,
        old_bkup,
        new_bkup,
    };

    // Scan dir entries
    let mut walk_dir = WalkDir::new(bkup_root).into_iter();
    let _ = walk_dir.next().unwrap().context("scanning backup root")?;
    for dir_entry_res in walk_dir {
        let dir_entry = dir_entry_res
            .context("reading DirEntry")
            .map(DirEntry::new)??;
        backup_single_dir_entry(&mut state, dir_entry)?;
    }

    // Ingest new fields into storage
    for NewBackupFile { source, hash, .. } in state.new_bkup.iter_new_files() {
        storage::store_file(&vault_dir, source, *hash).context_2("store_file", source)?;
    }

    // Insert new backup into database
    db.insert_backup_builder(bkup_name, state.new_bkup);
    db.write_to_vault(&vault_dir)?;

    Ok(())
}

/// Handle a single
fn backup_single_dir_entry(state: &mut BackupState, dir_entry: DirEntry) -> Result<()> {
    let BackupState {
        bkup_root,
        new_bkup,
        old_bkup,
        ..
    } = state;
    let bkup_path = dir_entry.path_relative_to(bkup_root);

    eprintln!("{}\t{}", dir_entry.as_ref(), bkup_path.display());

    match dir_entry {
        DirEntry::File {
            path,
            ino,
            mtime,
            size,
        } => {
            let old_file = old_bkup.as_ref().and_then(|bkup| bkup.get_file(ino));

            // Check if the old version of the file matches the current one
            match old_file {
                // Same inode & older mtime, we assume it's the same file.
                // TODO: check size
                Some(old_file) if mtime <= old_file.mtime() => {
                    let hash = old_file.hash();
                    new_bkup.insert_unchanged_file(bkup_path, hash, ino);
                }

                // Same inode but mtime changed, we need to hash the file to check
                Some(old_file) => {
                    let hash = Hash::file(&path)?;
                    eprintln!("\t{hash}");

                    if old_file.hash() == hash {
                        new_bkup.insert_unchanged_file(bkup_path, hash, ino);
                    } else {
                        new_bkup.insert_new_file(path, bkup_path, hash, ino, mtime, size);
                    }
                }

                // There was no old version of the file
                None => {
                    let hash = Hash::file(&path)?;
                    eprintln!("\t{hash}");

                    new_bkup.insert_new_file(path, bkup_path, hash, ino, mtime, size);
                }
            }
        }
        DirEntry::Directory { .. } => state.new_bkup.insert_directory(bkup_path),
        DirEntry::Symlink { target, .. } => state.new_bkup.insert_symlink(bkup_path, target),
        DirEntry::Special { path } => bail!("{}: special file", path.display()),
    }

    Ok(())
}

#[derive(strum::AsRefStr)]
enum DirEntry {
    File {
        path: PathBuf,
        ino: u64,
        mtime: MTime,
        size: u64,
    },
    Directory {
        path: PathBuf,
    },
    Symlink {
        path: PathBuf,
        target: PathBuf,
    },
    Special {
        path: PathBuf,
    },
}

impl DirEntry {
    fn new(source: WalkDirEntry) -> Result<Self> {
        let metadata = source.metadata().context("metadata")?;
        let ino = source.ino();
        let path = source.into_path();
        Ok(if metadata.is_file() {
            let mtime = MTime::from(metadata.modified().context("mtime")?);
            let size = metadata.len();
            DirEntry::File {
                path,
                ino,
                mtime,
                size,
            }
        } else if metadata.is_dir() {
            DirEntry::Directory { path }
        } else if metadata.is_symlink() {
            let target = read_link(&path).context("readlink")?;
            DirEntry::Symlink { path, target }
        } else {
            DirEntry::Special { path }
        })
    }

    fn path(&self) -> &Path {
        match self {
            DirEntry::File { path, .. } => path,
            DirEntry::Directory { path, .. } => path,
            DirEntry::Symlink { path, .. } => path,
            DirEntry::Special { path, .. } => path,
        }
    }

    fn path_relative_to(&self, bkup_root: &Path) -> PathBuf {
        self.path().strip_prefix(bkup_root).unwrap().to_owned()
    }
}
