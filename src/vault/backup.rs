use derive_more::{Deref, DerefMut};
use fieldmap::ClonedFieldMap;
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    rc::Rc,
};

use super::database::{DataBlocks, FileMetadata};
use crate::util::{Hash, MTime};

#[derive(Serialize, Deserialize, Debug)]
pub struct Backup {
    files: BackupFiles,
    directories: BTreeSet<PathBuf>,
    symlinks: BTreeMap<PathBuf, PathBuf>,
}

impl Backup {
    pub fn new() -> Self {
        Self {
            files: BackupFiles::new(),
            directories: BTreeSet::new(),
            symlinks: BTreeMap::new(),
        }
    }

    pub fn insert_directory(&mut self, path: PathBuf) {
        self.directories.insert(path);
    }

    pub fn insert_symlink(&mut self, target: PathBuf, link_name: PathBuf) {
        self.symlinks.insert(link_name, target);
    }

    pub fn insert_file(&mut self, path: PathBuf, ino: u64, mtime: MTime, hash: Hash, bytes: u64) {
        self.files.insert(BackupFile {
            path,
            ino,
            mtime,
            hash,
            bytes,
        });
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BackupFile {
    pub ino: u64,
    pub path: PathBuf,
    pub hash: Hash,
    pub mtime: MTime,
    pub bytes: u64,
}

impl BackupFile {
    fn ino(&self) -> &u64 {
        &self.ino
    }
}

#[derive(Serialize, Deserialize, Debug, Deref, DerefMut)]
pub struct BackupFiles(
    #[serde(deserialize_with = "BackupFiles::deserialize")] ClonedFieldMap<BackupFile, u64>,
);

impl BackupFiles {
    fn new() -> Self {
        Self(ClonedFieldMap::new(BackupFile::ino))
    }

    fn deserialize<'de, D>(deserializer: D) -> Result<ClonedFieldMap<BackupFile, u64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        ClonedFieldMap::deserialize(BackupFile::ino, deserializer)
    }
}

/// A read-only view of a backup in the database. It is used to interface with otherwise
/// private fields inside `Backup`s and `Database`s. It is constructed with information
/// from both, so the type is kind of co-owned between those two modules.
pub struct BackupView<'a> {
    name: &'a str,
    backup: &'a Backup,
    data_blocks: &'a DataBlocks,
}

impl<'a> BackupView<'a> {
    pub fn new(name: &'a str, backup: &'a Backup, data_blocks: &'a DataBlocks) -> Self {
        Self {
            name,
            backup,
            data_blocks,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn files(&self) -> &BackupFiles {
        &self.backup.files
    }

    pub fn directories(&self) -> &BTreeSet<PathBuf> {
        &self.backup.directories
    }

    pub fn symlinks(&self) -> &BTreeMap<PathBuf, PathBuf> {
        &self.backup.symlinks
    }

    fn data_block_of_file(&self, backup_file: &BackupFile) -> &FileMetadata {
        let ino = backup_file.ino;
        self.data_blocks
            .get(&backup_file.hash)
            .unwrap_or_else(|| panic!("inode {ino} in backup but has no data_blocks entry"))
    }

    pub fn get_file(&self, ino: u64) -> Option<BackupFileView> {
        let backup_file = self.backup.files.get(&ino)?;
        let data_block = self.data_block_of_file(backup_file);

        Some(BackupFileView {
            backup_file,
            data_block,
        })
    }

    pub fn iter_files(&self) -> impl Iterator<Item = BackupFileView> {
        self.files()
            .data()
            .iter()
            .map(|backup_file| BackupFileView {
                backup_file,
                data_block: self.data_block_of_file(backup_file),
            })
    }
}

pub struct BackupFileView<'a> {
    backup_file: &'a BackupFile,
    data_block: &'a FileMetadata,
}

impl<'a> BackupFileView<'a> {
    pub fn ino(&self) -> u64 {
        self.backup_file.ino
    }

    pub fn path(&self) -> &Path {
        &self.backup_file.path
    }

    pub fn hash(&self) -> Hash {
        self.backup_file.hash
    }

    pub fn mtime(&self) -> MTime {
        self.backup_file.mtime
    }

    pub fn apparent_size(&self) -> u64 {
        self.data_block.apparent_size
    }
}
