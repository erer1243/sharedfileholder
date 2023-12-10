#![allow(unused)]

mod database;

use blake3::{Hash, Hasher};
use clap::Parser;
use database::{BackupBuilder, BackupView, Database, MTime};
use eyre::{bail, ensure, eyre, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    env::current_dir,
    fmt::Debug,
    fs::{read_dir, read_link, File, Metadata},
    io::{self, copy, BufReader, BufWriter},
    path::{Path, PathBuf},
    process::exit,
};
use walkdir::{DirEntry, DirEntryExt, WalkDir};

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

fn cmd_backup(db_path: &Path, bkup_name: &str, root: &Path) -> Result<()> {
    let mut db = Database::load(db_path).context_2("loading database", db_path)?;
    let old = db.take_backup(bkup_name).unwrap_or_default();
    let mut new = BackupBuilder::new();

    // Scan dir entries
    for dir_entry_res in WalkDir::new(root).into_iter().skip(1) {
        // TODO: Gracefully allow failure
        let dir_entry = dir_entry_res.context("dir_entry")?;
        backup_dir_entry(&mut db, &old, &mut new, root, &dir_entry)
            .context_debug(dir_entry.path())?;
    }

    println!("{old:#?}");
    println!("########");
    println!("{new:#?}");

    Ok(())
}

fn backup_dir_entry(
    db: &mut Database,
    old: &BackupView,
    new: &mut BackupBuilder,
    root: &Path,
    dir_entry: &DirEntry,
) -> Result<()> {
    let path = dir_entry.path();
    let stripped_path = path.strip_prefix(root).unwrap().to_owned();
    let metadata = dir_entry.metadata().context("stat")?;

    if metadata.is_dir() {
        new.insert_directory(stripped_path);
    } else if metadata.is_symlink() {
        let link_target = read_link(path).context("readlink")?;
        new.insert_symlink(stripped_path, link_target);
    } else if metadata.is_file() {
        let inode = dir_entry.ino();
        let mtime: MTime = metadata.modified().context("mtime")?.into();
        let hash = match old.file_by_inode(inode) {
            Some(old_file) if mtime <= old_file.data_block_mtime => old_file.hash,
            _ => hash_file(path).context("hash_file")?,
        };
        new.insert_file(stripped_path, hash, inode);
    } else {
        bail!("{path:?}: special file");
    }

    Ok(())
}

fn hash_file<P: AsRef<Path>>(p: P) -> io::Result<Hash> {
    let mut file = BufReader::new(File::open(p)?);
    let mut hasher = Hasher::new();
    copy(&mut file, &mut hasher)?;
    Ok(hasher.finalize())
}

trait ContextExt<T, E>: Context<T, E> + Sized {
    fn context_debug<D: Debug>(self, obj: D) -> Result<T> {
        self.with_context(|| format!("{obj:?}"))
    }

    fn context_2<D: Debug>(self, msg: &str, obj: D) -> Result<T> {
        self.with_context(|| format!("{msg}: {obj:?}"))
    }
}

impl<C: Context<T, E>, T, E> ContextExt<T, E> for C {}
