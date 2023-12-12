#![allow(unused)]

mod database;
mod storage;

use blake3::{Hash, Hasher};
use clap::Parser;
use database::{BackupBuilder, BackupView, Database, MTime};
use derive_more::{Display, From};
use eyre::{bail, ensure, eyre, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    env::current_dir,
    fmt::Debug,
    fs::{read_dir, read_link, File, Metadata},
    io::{self, copy, BufReader, BufWriter},
    path::{Path, PathBuf},
    process::exit,
};
use thiserror::Error;
use walkdir::{DirEntry as WalkDirEntry, DirEntryExt, WalkDir};

#[derive(Parser)]
enum Command {
    Init,
    Backup {
        database: PathBuf,
        backup_name: String,
        dir: PathBuf,
    },
}

fn main() {
    let cmd = Command::parse();
    let res = match cmd {
        Command::Init => cmd_init(),
        Command::Backup {
            database,
            backup_name,
            dir,
        } => cmd_backup(&database, &backup_name, &dir),
    };
    if let Err(e) = res {
        eprintln!("[error] {e:#}");
        exit(1);
    }
}

fn cmd_init() -> Result<()> {
    // Check that dir is empty
    let cwd = current_dir().context("current_dir")?;
    let mut read_dir = read_dir(&cwd).context("read_dir")?;
    ensure!(read_dir.next().is_none(), "{cwd:?} is not empty");

    // Make empty database file
    Database::default().write("database.json")
}

struct BackupState<'a> {
    bkup_name: &'a str,
    bkup_root: &'a Path,
    db: Database,
    old: BackupView,
    new: BackupBuilder,
    unchanged_files: HashSet<PathBuf>,
}

fn cmd_backup(db_path: &Path, bkup_name: &str, bkup_root: &Path) -> Result<()> {
    let mut db = Database::load(db_path).context_2("loading database", db_path)?;
    let old = db.take_backup(bkup_name).unwrap_or_default();
    let mut state = BackupState {
        bkup_name,
        bkup_root,
        db,
        old,
        new: BackupBuilder::new(),
        unchanged_files: HashSet::new(),
    };

    // Scan dir entries
    // Skip first entry in WalkDir because that's the backup root dir
    let walk_dir = WalkDir::new(bkup_root).into_iter().skip(1);
    for dir_entry_res in walk_dir {
        let dir_entry = dir_entry_res.context("dir_entry").map(DirEntry::new)??;
        backup_single_dir_entry(&mut state, dir_entry);
    }

    Ok(())
}

fn backup_single_dir_entry(state: &mut BackupState, dir_entry: DirEntry) -> Result<()> {
    let bkup_path = dir_entry.path_relative_to(state.bkup_root);
    match dir_entry {
        DirEntry::File { path, ino, mtime } => {
            let hash = match backup_unchanged_file_hash(state, ino, mtime) {
                Some(old_hash) => {
                    state.unchanged_files.insert(path);
                    old_hash
                }
                None => hash_file(&path)?,
            };
            state.new.insert_file(bkup_path, hash, ino);
        }
        DirEntry::Directory { .. } => state.new.insert_directory(bkup_path),
        DirEntry::Symlink { target, .. } => state.new.insert_symlink(bkup_path, target),
        DirEntry::Special { path } => bail!("{path:?}: special file"),
    }

    Ok(())
}

fn backup_unchanged_file_hash(state: &BackupState, ino: u64, mtime: MTime) -> Option<Hash> {
    state
        .old
        .file_by_inode(ino)
        .filter(|bkup_file| bkup_file.data_block_mtime <= mtime)
        .map(|bkup_file| bkup_file.hash)
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
            DirEntry::File { path, .. } => &path,
            DirEntry::Directory { path, .. } => &path,
            DirEntry::Symlink { path, .. } => &path,
            DirEntry::Special { path, .. } => &path,
        }
    }

    fn path_relative_to(&self, bkup_root: &Path) -> PathBuf {
        self.path().strip_prefix(bkup_root).unwrap().to_owned()
    }
}

fn hash_file(path: &Path) -> io::Result<Hash> {
    let mut file = BufReader::new(File::open(path)?);
    let mut hasher = Hasher::new();
    copy(&mut file, &mut hasher)?;
    Ok(hasher.finalize())
}

/*
#[derive(Error, Debug)]
enum FatalError {
    #[error("IO error: {0}")]
    IO(#[from] io::Error),

    #[error("vault in invalid state: {0}")]
    InvalidState(#[from] InvalidStateError),
}

#[derive(Error, From, Debug, Display)]
struct InvalidStateError {
    description: String,
}
*/

trait ContextExt<T, E>: Context<T, E> + Sized {
    fn context_debug<D: Debug>(self, obj: D) -> Result<T> {
        self.with_context(|| format!("{obj:?}"))
    }

    fn context_2<D: Debug>(self, msg: &str, obj: D) -> Result<T> {
        self.with_context(|| format!("{msg}: {obj:?}"))
    }
}

impl<C: Context<T, E>, T, E> ContextExt<T, E> for C {}
