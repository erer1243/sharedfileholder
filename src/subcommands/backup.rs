use blake3::{Hash, Hasher};
use clap::Args;
use eyre::{bail, Context, Result};
use std::{
    fs::read_link,
    io,
    path::{Path, PathBuf},
};
use walkdir::{DirEntry as WalkDirEntry, DirEntryExt, WalkDir};

use crate::{
    database::{BackupBuilder, BackupView, Database, MTime},
    path_or_cwd, storage, ContextExt,
};

use super::GlobalArgs;

#[derive(Args)]
pub struct CliArgs {
    backup_name: String,
    backup_source_dir: PathBuf,
}

pub fn run(gargs: GlobalArgs, args: CliArgs) -> Result<()> {
    backup(gargs.vault_dir, &args.backup_name, &args.backup_source_dir)
}

struct BackupState<'a, 'b> {
    bkup_root: &'a Path,
    old: Option<BackupView<'b>>,
    new: BackupBuilder,
}

fn backup(provided_vault_dir: Option<PathBuf>, bkup_name: &str, bkup_root: &Path) -> Result<()> {
    let vault_dir = path_or_cwd(provided_vault_dir);
    let mut db = Database::load_from_vault(&vault_dir)?;
    let old = db.get_backup(bkup_name);

    let mut state = BackupState {
        bkup_root,
        old,
        new: BackupBuilder::new(),
    };

    // Scan dir entries
    // Skip first entry in WalkDir because that's the backup root dir
    let walk_dir = WalkDir::new(bkup_root).into_iter().skip(1);
    for dir_entry_res in walk_dir {
        let dir_entry = dir_entry_res.context("dir_entry").map(DirEntry::new)??;
        backup_single_dir_entry(&mut state, dir_entry)?;
    }

    // Ingest new fields into storage
    for (path, hash, _) in state.new.iter_new_files() {
        storage::store_file(&vault_dir, path, hash).context_2("store_file", path)?;
    }

    db.insert_backup_builder(bkup_name, state.new);
    db.write_to_vault(&vault_dir)?;

    Ok(())
}

fn backup_single_dir_entry(state: &mut BackupState, dir_entry: DirEntry) -> Result<()> {
    let bkup_path = dir_entry.path_relative_to(state.bkup_root);

    match dir_entry {
        DirEntry::File { path, ino, mtime } => {
            let maybe_old_file = state.old.as_ref().and_then(|bkup| bkup.get_file(ino));
            match maybe_old_file {
                Some(old_file) if mtime <= old_file.data_block_mtime() => {
                    let hash = old_file.hash();
                    state.new.insert_unchanged_file(bkup_path, hash, ino);
                }
                Some(old_file) => {
                    let hash = hash_file(&path)?;
                    if old_file.hash() == hash {
                        state.new.insert_unchanged_file(bkup_path, hash, ino);
                    } else {
                        state.new.insert_new_file(path, bkup_path, hash, ino, mtime);
                    }
                }
                None => {
                    let hash = hash_file(&path)?;
                    state.new.insert_new_file(path, bkup_path, hash, ino, mtime);
                }
            }
        }
        DirEntry::Directory { .. } => state.new.insert_directory(bkup_path),
        DirEntry::Symlink { target, .. } => state.new.insert_symlink(bkup_path, target),
        DirEntry::Special { path } => bail!("{}: special file", path.display()),
    }

    Ok(())
}

enum DirEntry {
    File {
        path: PathBuf,
        ino: u64,
        mtime: MTime,
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
            DirEntry::File { path, ino, mtime }
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

fn hash_file(path: &Path) -> io::Result<Hash> {
    Ok(Hasher::new().update_mmap(path)?.finalize())
}
