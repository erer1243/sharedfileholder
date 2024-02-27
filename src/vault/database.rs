use super::backup::{Backup, BackupView};
use crate::util::{ContextExt, Hash};

use derive_more::{Deref, DerefMut};
use eyre::Result;
use fieldmap::ClonedFieldMap;
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

const DATABASE_NAME: &str = "database.json";

#[derive(Serialize, Deserialize)]
pub struct Database {
    #[serde(skip)]
    path: PathBuf,

    backups: BTreeMap<String, Backup>,

    /// XXX currently unused! Anything using this will be wrong
    files_metadata: FilesMetadata,
}

/// POD struct with information about a file in storage.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
pub struct FileMetadata {
    pub hash: Hash,
    pub bytes: u64,
}

impl Database {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().join(DATABASE_NAME);
        Self {
            path,
            backups: BTreeMap::new(),
            files_metadata: FilesMetadata::new(),
        }
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().join(DATABASE_NAME);
        let f = BufReader::new(File::open(&path).context_2("reading db file", &path)?);
        let db = serde_json::from_reader(f)?;
        Ok(db)
    }

    pub fn write(&self) -> Result<()> {
        let f = BufWriter::new(File::create(&self.path).context_2("writing db file", &self.path)?);
        serde_json::to_writer_pretty(f, self)?;
        Ok(())
    }

    pub fn iter_backups(&self) -> impl Iterator<Item = BackupView> {
        self.backups
            .keys()
            .map(|name| self.get_backup(name).unwrap())
    }

    pub fn get_backup(&self, name: &str) -> Option<BackupView> {
        let (name, backup) = self.backups.get_key_value(name)?;
        let files_metadata = &self.files_metadata;
        Some(BackupView::new(name, backup, files_metadata))
    }

    pub fn get_file_metadata(&self, hash: Hash) -> Option<FileMetadata> {
        self.files_metadata.get(&hash).copied()
    }

    pub fn insert_backup(&mut self, name: &str, backup: Backup) {
        self.backups.insert(name.to_owned(), backup);
    }
}

#[derive(Serialize, Deserialize, Deref, DerefMut)]
pub struct FilesMetadata(
    #[serde(deserialize_with = "FilesMetadata::deserialize")] ClonedFieldMap<FileMetadata, Hash>,
);

impl FilesMetadata {
    fn new() -> Self {
        Self(ClonedFieldMap::new(|datablock| &datablock.hash))
    }

    fn deserialize<'de, D>(deserializer: D) -> Result<ClonedFieldMap<FileMetadata, Hash>, D::Error>
    where
        D: Deserializer<'de>,
    {
        ClonedFieldMap::deserialize(|datablock| &datablock.hash, deserializer)
    }
}
