use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs::File,
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
    rc::Rc,
};

use blake3::Hash;
use eyre::Result;
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;

slotmap::new_key_type! { struct BackupFileKey; }

pub struct BackupView {
    directories: BTreeSet<PathBuf>,
    symlinks: BTreeMap<PathBuf, PathBuf>,

    files: SlotMap<BackupFileKey, BackupFile>,
    files_by_path: HashMap<Rc<Path>, BackupFileKey>,
    files_by_inode: HashMap<u64, BackupFileKey>,
}

impl BackupView {
    fn expand(mini: MinimizedBackup) -> Self {
        let mut files = SlotMap::with_capacity_and_key(mini.files.len());
        let mut files_by_path = HashMap::with_capacity(mini.files.len());
        let mut files_by_inode = HashMap::with_capacity(mini.files.len());

        for (path, file) in mini.files {
            let path: Rc<Path> = Rc::from(path);
            let key = files.insert(BackupFile {
                path: path.clone(),
                hash: file.hash,
                source_inode: file.source_inode,
            });
            files_by_path.insert(path, key);
            files_by_inode.insert(file.source_inode, key);
        }

        Self {
            directories: mini.directories,
            symlinks: mini.symlinks,
            files,
            files_by_path,
            files_by_inode,
        }
    }

    pub fn file_by_path<P: AsRef<Path>>(&self, p: P) -> Option<&BackupFile> {
        self.files_by_path
            .get(p.as_ref())
            .map(|key| self.files.get(*key).unwrap())
    }

    pub fn file_by_inode(&self, ino: u64) -> Option<&BackupFile> {
        self.files_by_inode
            .get(&ino)
            .map(|key| self.files.get(*key).unwrap())
    }
}

pub struct BackupFile {
    pub path: Rc<Path>,
    pub hash: Hash,
    pub source_inode: u64,
}

pub struct BackupBuilder {
    mini: MinimizedBackup,
}

impl BackupBuilder {}

#[derive(Serialize, Deserialize)]
struct Database {
    backups: BTreeMap<String, MinimizedBackup>,
    data_blocks: HashMap<Hash, DataBlockMetadata>,
}

impl Database {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let f = BufReader::new(File::open(path)?);
        Ok(serde_json::from_reader(f)?)
    }

    pub fn write<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let f = BufWriter::new(File::create(path)?);
        serde_json::to_writer(f, self)?;
        Ok(())
    }

    pub fn take_backup_view(&mut self, name: &str) -> Option<BackupView> {
        self.backups.remove(name).map(BackupView::expand)
    }

    pub fn insert_backup_builder(&mut self, name: &str, bb: BackupBuilder) {
        self.backups.insert(name.to_owned(), bb.mini);
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct MinimizedBackup {
    files: BTreeMap<PathBuf, MinimizedBackupFile>,
    directories: BTreeSet<PathBuf>,
    symlinks: BTreeMap<PathBuf, PathBuf>,
}

#[derive(Serialize, Deserialize, Debug)]
struct MinimizedBackupFile {
    hash: Hash,
    source_inode: u64,
}

#[derive(Serialize, Deserialize)]
struct DataBlockMetadata {
    mtime: MTime,
}

#[derive(Serialize, Deserialize)]
struct MTime {
    sec: i64,
    nano: u32,
}
