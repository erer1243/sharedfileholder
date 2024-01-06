use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    rc::Rc,
};

use crate::util::{Hash, MTime};

use crate::database::DataBlockMetadata;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Backup {
    files: BTreeMap<u64, BackupFileMetadata>,
    directories: BTreeSet<PathBuf>,
    symlinks: BTreeMap<PathBuf, PathBuf>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupFileMetadata {
    // Shared with NewBackupFile
    pub path: Rc<Path>,
    pub hash: Hash,
}

#[derive(Default, Debug)]
pub struct BackupBuilder {
    inner: Backup,
    new_files: Vec<NewBackupFile>,
}

/// Details about a file inserted into a BackupBuilder via [`BackupBuilder::insert_new_file`].
/// Obtained via [`BackupBuilder::iter_new_files`].
#[derive(Debug)]
pub struct NewBackupFile {
    pub source: PathBuf,
    // Shared with BackupFileMetadata
    pub bkup_path: Rc<Path>,
    pub ino: u64,
    pub hash: Hash,
    pub mtime: MTime,
    pub size: u64,
}

impl BackupBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_directory(&mut self, path: PathBuf) {
        self.inner.directories.insert(path);
    }

    pub fn insert_symlink(&mut self, path: PathBuf, target: PathBuf) {
        self.inner.symlinks.insert(path, target);
    }

    pub fn insert_new_file(
        &mut self,
        src_path: PathBuf,
        bkup_path: PathBuf,
        hash: Hash,
        ino: u64,
        mtime: MTime,
        size: u64,
    ) {
        let bkup_path: Rc<Path> = bkup_path.into();

        self.inner.files.insert(
            ino,
            BackupFileMetadata {
                path: bkup_path.clone(),
                hash,
            },
        );
        self.new_files.push(NewBackupFile {
            source: src_path,
            bkup_path,
            ino,
            hash,
            mtime,
            size,
        });
    }

    pub fn insert_unchanged_file(&mut self, path: PathBuf, hash: Hash, ino: u64) {
        let mbf = BackupFileMetadata {
            path: path.into(),
            hash,
        };
        self.inner.files.insert(ino, mbf);
    }

    pub fn iter_new_files(&self) -> impl Iterator<Item = &NewBackupFile> {
        self.new_files.iter()
    }

    pub fn into_inner(self) -> Backup {
        self.inner
    }
}

pub struct BackupView<'a> {
    name: &'a str,
    backup: &'a Backup,
    data_blocks: &'a HashMap<Hash, DataBlockMetadata>,
}

impl<'a> BackupView<'a> {
    pub fn new(
        name: &'a str,
        backup: &'a Backup,
        data_blocks: &'a HashMap<Hash, DataBlockMetadata>,
    ) -> Self {
        Self {
            name,
            backup,
            data_blocks,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn files(&self) -> &BTreeMap<u64, BackupFileMetadata> {
        &self.backup.files
    }

    pub fn directories(&self) -> &BTreeSet<PathBuf> {
        &self.backup.directories
    }

    pub fn symlinks(&self) -> &BTreeMap<PathBuf, PathBuf> {
        &self.backup.symlinks
    }

    pub fn get_file(&self, ino: u64) -> Option<BackupFileView> {
        let meta = self.backup.files.get(&ino)?;
        let data_block_metadata = self
            .data_blocks
            .get(&meta.hash)
            .unwrap_or_else(|| panic!("inode {ino} in backup but has no data_blocks entry"));

        Some(BackupFileView {
            ino,
            meta,
            data_block_metadata,
        })
    }
}

pub struct BackupFileView<'a> {
    ino: u64,
    meta: &'a BackupFileMetadata,
    data_block_metadata: &'a DataBlockMetadata,
}

impl<'a> BackupFileView<'a> {
    pub fn ino(&self) -> u64 {
        self.ino
    }

    pub fn path(&self) -> &Path {
        &self.meta.path
    }

    pub fn hash(&self) -> Hash {
        self.meta.hash
    }

    pub fn data_block_mtime(&self) -> MTime {
        // self.data_block_metadata.mtime
        todo!()
    }
}
