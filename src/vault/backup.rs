use derive_more::{Deref, DerefMut};
use fieldmap::ClonedFieldMap;
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

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

    pub fn insert_file(&mut self, backup_file: BackupFile) {
        self.files.insert(backup_file);
    }

    pub fn iter_files(&self) -> impl Iterator<Item = &BackupFile> {
        self.files.data().iter()
    }

    pub fn iter_directories(&self) -> impl Iterator<Item = &Path> {
        self.directories.iter().map(AsRef::as_ref)
    }

    pub fn iter_symlinks(&self) -> impl Iterator<Item = (&Path, &Path)> {
        self.symlinks
            .iter()
            .map(|(src, dst)| (src.as_ref(), dst.as_ref()))
    }

    pub fn get_file(&self, ino: u64) -> Option<&BackupFile> {
        self.files.get(&ino)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BackupFile {
    pub ino: u64,
    pub path: PathBuf,
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
