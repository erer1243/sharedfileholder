use derive_more::{Deref, DerefMut};
use fieldmap::ClonedFieldMap;
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    rc::Rc,
};

use super::database::{DataBlock, DataBlocks};
use crate::util::{Hash, MTime};

#[derive(Serialize, Deserialize, Debug)]
pub struct Backup {
    files: BackupFiles,
    directories: BTreeSet<PathBuf>,
    symlinks: BTreeMap<PathBuf, PathBuf>,
}

impl Backup {
    fn new() -> Self {
        Self {
            files: BackupFiles::new(),
            directories: BTreeSet::new(),
            symlinks: BTreeMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BackupFile {
    pub ino: u64,
    // Shared with NewBackupFile
    pub path: Rc<Path>,
    pub hash: Hash,
    pub mtime: MTime,
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

#[derive(Debug)]
pub struct BackupBuilder {
    inner: Backup,
    new_files: Vec<NewBackupFile>,
}

/// Details about a file inserted into a BackupBuilder via [`BackupBuilder::insert_new_file`].
/// Obtained via [`BackupBuilder::iter_new_files`].
#[derive(Debug)]
pub struct NewBackupFile {
    pub source: PathBuf,
    // Shared with BackupFile
    pub bkup_path: Rc<Path>,
    pub ino: u64,
    pub hash: Hash,
    pub mtime: MTime,
    pub apparent_size: u64,
}

impl BackupBuilder {
    pub fn new() -> Self {
        Self {
            inner: Backup::new(),
            new_files: Vec::new(),
        }
    }

    pub fn insert_directory(&mut self, path: PathBuf) {
        self.inner.directories.insert(path);
    }

    pub fn insert_symlink(&mut self, path: PathBuf, target: PathBuf) {
        self.inner.symlinks.insert(path, target);
    }

    pub fn insert_new_file(
        &mut self,
        source: PathBuf,
        bkup_path: PathBuf,
        hash: Hash,
        ino: u64,
        mtime: MTime,
        apparent_size: u64,
    ) {
        let bkup_path: Rc<Path> = bkup_path.into();
        self.inner.files.insert(BackupFile {
            ino,
            path: bkup_path.clone(),
            hash,
            mtime,
        });
        self.new_files.push(NewBackupFile {
            source,
            bkup_path,
            ino,
            hash,
            mtime,
            apparent_size,
        });
    }

    pub fn insert_unchanged_file(&mut self, path: PathBuf, hash: Hash, ino: u64, mtime: MTime) {
        self.inner.files.insert(BackupFile {
            ino,
            path: Rc::from(path),
            hash,
            mtime,
        });
    }

    pub fn iter_new_files(&self) -> impl Iterator<Item = &NewBackupFile> {
        self.new_files.iter()
    }

    pub fn into_inner(self) -> Backup {
        self.inner
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

    fn data_block_of_file(&self, backup_file: &BackupFile) -> &DataBlock {
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
    data_block: &'a DataBlock,
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
