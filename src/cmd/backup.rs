use clap::Args;
use eyre::{bail, Context, Result};
use std::{
    fs::{read_link, symlink_metadata},
    io,
    path::{Path, PathBuf},
};
use thiserror::Error;
use walkdir::{DirEntryExt, WalkDir};

use crate::{
    cmd::GlobalArgs,
    util::{path_or_cwd, ContextExt, Hash, MTime, PathBufDisplay},
    vault::{
        backup::{Backup, BackupView},
        database::Database,
        storage::Storage,
    },
};

#[derive(Args)]
pub struct CliArgs {
    backup_name: String,
    backup_source_dir: PathBuf,
}

struct BackupState<'a, 'b> {
    bkup_root: &'a Path,
    old_bkup: Option<BackupView<'b>>,
    // new_bkup: BackupBuilder,
}

pub fn run(gargs: GlobalArgs, args: CliArgs) -> Result<()> {
    backup(gargs.vault_dir, &args.backup_name, &args.backup_source_dir)
}

fn backup(provided_vault_dir: Option<PathBuf>, bkup_name: &str, bkup_root: &Path) -> Result<()> {
    let vault_dir = path_or_cwd(provided_vault_dir);
    let storage = Storage::new(&vault_dir);
    let mut db = Database::load(&vault_dir).context_2("loading vault", &vault_dir)?;
    let old_bkup = db.get_backup(bkup_name);
    // let new_bkup = BackupBuilder::new();
    let mut state = BackupState {
        bkup_root,
        old_bkup,
        // new_bkup,
    };

    // Scan dir entries
    // let mut walk_dir = WalkDir::new(bkup_root).into_iter();
    // let _ = walk_dir.next().unwrap().context("scanning backup root")?;
    // for dir_entry_res in walk_dir {
    //     let dir_entry = dir_entry_res
    //         .context("reading DirEntry")
    //         .map(DirEntry::new)??;
    //     handle_dir_entry(&mut state, dir_entry)?;
    // }

    // Ingest new files into storage
    // for NewBackupFile { source, hash, .. } in state.new_bkup.iter_new_files() {
    //     storage
    //         .insert_file(source, *hash)
    //         .context_2("storage::insert_file", source)?;
    // }

    // // Insert new backup into database
    // db.insert_backup_builder(bkup_name, state.new_bkup);
    // db.write(&vault_dir)?;

    Ok(())
}

fn new_backup(root: &Path) -> Result<Backup> {
    let mut backup = Backup::new();
    let handle_dir_entry = |entry: DirEntry| -> Result<()> {
        use DirEntryKind::*;

        match entry.kind {
            File { ino, mtime, size } => {
                let hash = Hash::of_file(&entry.path).context(PathBufDisplay(entry.path))?;
                backup.insert_file(entry.path_from_root, ino, mtime, hash, size);
            }
            Directory => backup.insert_directory(entry.path_from_root),
            Symlink { target } => backup.insert_symlink(target, entry.path_from_root),
            // TODO perhaps add an option to ignore special files?
            // Check what other tools like Git, Unison, and Rsync do
            Special => bail!("{}: special file", entry.path.display()),
        }
        Ok(())
    };
    scan_dir(root, handle_dir_entry)?;
    Ok(backup)
}

// fn update_existing_backup(vault: &mut Vault, root: &Path) -> Result<()> {}

fn scan_dir(root: &Path, mut handle_dir_entry: impl FnMut(DirEntry) -> Result<()>) -> Result<()> {
    // Skip the first entry, the entry for `root`
    let walkdir = WalkDir::new(root).into_iter().skip(1);
    for waldir_entry_res in walkdir {
        let walkdir_entry = waldir_entry_res?;
        let dir_entry = DirEntry::new(root, walkdir_entry)?;
        handle_dir_entry(dir_entry)?;
    }

    Ok(())
}

// fn handle_dir_entry(state: &mut BackupState, dir_entry: DirEntry) -> Result<()> {
//     let BackupState {
//         bkup_root,
//         new_bkup,
//         old_bkup,
//         ..
//     } = state;
//     let bkup_path = dir_entry.path_relative_to(bkup_root);

//     eprintln!("{: <8}\t{}", dir_entry.as_ref(), bkup_path.display());

//     match dir_entry {
//         DirEntry::File {
//             path,
//             ino,
//             mtime,
//             size,
//         } => {
//             let old_file = old_bkup.as_ref().and_then(|bkup| bkup.get_file(ino));

//             // The rules used for determining if a path has not changed:
//             //
//             // * path: has the type of file at the given path changed (i.e. a file became a dir)? If not, continue.
//             // * file = has the inode changed? If not, is the mtime later than the previous one (TODO store mtime)? If not, the file is the considered identical.
//             match old_file {
//                 // Same inode & older mtime, we assume it's the same file.
//                 // TODO: check size
//                 Some(old_file) if mtime <= old_file.mtime() => {
//                     let hash = old_file.hash();
//                     new_bkup.insert_unchanged_file(bkup_path, hash, ino, mtime);
//                 }

//                 // Same inode but mtime changed, we need to hash the file to check if it has changed
//                 Some(old_file) => {
//                     let hash = Hash::file(&path)?;
//                     eprintln!("\t\t{hash}");
//                     if old_file.hash() == hash {
//                         new_bkup.insert_unchanged_file(bkup_path, hash, ino, mtime);
//                     } else {
//                         new_bkup.insert_new_file(path, bkup_path, hash, ino, mtime, size);
//                     }
//                 }

//                 // There was no old version of the file
//                 None => {
//                     let hash = Hash::file(&path)?;
//                     eprintln!("\t\t{hash}");
//                     new_bkup.insert_new_file(path, bkup_path, hash, ino, mtime, size);
//                 }
//             }
//         }
//         DirEntry::Directory { .. } => state.new_bkup.insert_directory(bkup_path),
//         DirEntry::Symlink { target, .. } => state.new_bkup.insert_symlink(bkup_path, target),
//         DirEntry::Special { path } => bail!("{}: special file", path.display()),
//     }

//     Ok(())
// }

struct DirEntry {
    path: PathBuf,
    path_from_root: PathBuf,
    kind: DirEntryKind,
}

#[derive(strum::AsRefStr)]
enum DirEntryKind {
    File { ino: u64, mtime: MTime, size: u64 },
    Directory,
    Symlink { target: PathBuf },
    Special,
}

impl DirEntry {
    fn new(root: &Path, walk_dir_entry: walkdir::DirEntry) -> Result<Self, DirEntryError> {
        let ino = walk_dir_entry.ino();
        let path = walk_dir_entry.into_path();
        let path_from_root = path.strip_prefix(root).unwrap().to_path_buf();
        let ioerr = |err: io::Error| DirEntryError {
            inner: err,
            path: (*path).to_owned(),
        };
        let metadata = symlink_metadata(&*path).map_err(ioerr)?;
        let kind = if metadata.is_file() {
            let mtime = MTime::from(metadata.modified().map_err(ioerr)?);
            let size = metadata.len();
            DirEntryKind::File { ino, mtime, size }
        } else if metadata.is_dir() {
            DirEntryKind::Directory
        } else if metadata.is_symlink() {
            let target = read_link(&*path).map_err(ioerr)?;
            DirEntryKind::Symlink { target }
        } else {
            DirEntryKind::Special
        };

        Ok(DirEntry {
            path,
            path_from_root,
            kind,
        })
    }
}

#[derive(Debug, Error)]
#[error("{}: {inner}", .path.display())]
struct DirEntryError {
    inner: io::Error,
    path: PathBuf,
}
