#![allow(unused)]

mod database;

use blake3::{Hash, Hasher};
use clap::Parser;
use eyre::{ensure, eyre, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    env::current_dir,
    fmt::Debug,
    fs::{read_dir, read_link, File},
    io::{copy, BufReader, BufWriter},
    path::{Path, PathBuf},
    process::exit,
};
use walkdir::{DirEntry, DirEntryExt, WalkDir};

#[derive(Parser)]
enum Command {
    Init,
    Backup { database: PathBuf, dir: PathBuf },
}

fn main() {
    let cmd = Command::parse();
    let res = match cmd {
        Command::Init => cmd_init(),
        Command::Backup { database, dir } => cmd_backup(&database, "XXX", &dir),
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

fn cmd_backup(db_path: &Path, name: &str, root: &Path) -> Result<()> {
    let mut db = Database::load(db_path).context_2("loading database", db_path)?;
    let old_backup = db.backups.get(name).unwrap_or_else(|| todo!());
    let mut new_backup = Backup::default();

    for dir_entry_res in WalkDir::new(root) {
        // TODO: Gracefully allow failure
        let dir_entry = dir_entry_res.context("dir_entry")?;

        let path = dir_entry.path();
        let stripped_path = path.strip_prefix(root).unwrap().to_owned();
        let metadata = dir_entry.metadata().context_2("metadata", path)?;

        if metadata.is_dir() {
            new_backup.directories.insert(stripped_path);
        } else if metadata.is_symlink() {
            let link_target = read_link(path).context("read_link")?;
            new_backup.symlinks.insert(stripped_path, link_target);
        } else if metadata.is_file() {
            let inode = dir_entry.ino();
            let mut file = BufReader::new(File::open(path).context("open")?);
            let mut hasher = Hasher::new();
            copy(&mut file, &mut hasher).context("hashing file")?;
            let hash = hasher.finalize();
            new_backup.files.insert(stripped_path, hash);
        } else {
            Err(eyre!("special file")).context_debug(path)?;
        }
    }

    Ok(())
}

fn backup_dir_entry(bkup: &mut Backup, dir_entry: &DirEntry, path: &Path) -> Result<()> {
    let metadata = dir_entry.metadata().context("metadata")?;

    if metadata.is_file() {
        backup_file(bkup, path)
    } else if metadata.is_dir() {
        backup_dir(bkup, path)
    } else if metadata.is_symlink() {
        backup_symlink(bkup, path)
    } else {
        Err(eyre!("special file"))
    }
}

fn backup_file(bkup: &mut Backup, path: &Path) -> Result<()> {
    let mut file = BufReader::new(File::open(path).context("open")?);
    let mut hasher = Hasher::new();
    copy(&mut file, &mut hasher).context("hashing file")?;
    let hash = hasher.finalize();
    bkup.files.insert(path.into(), hash);
    Ok(())
}

fn backup_dir(bkup: &mut Backup, path: &Path) -> Result<()> {
    bkup.directories.insert(path.into());
    Ok(())
}

fn backup_symlink(bkup: &mut Backup, path: &Path) -> Result<()> {
    let link_target = read_link(path).context("read_link")?;
    bkup.symlinks.insert(path.into(), link_target);
    Ok(())
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
