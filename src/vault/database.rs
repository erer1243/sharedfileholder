use super::backup::{Backup, BackupBuilder, BackupView};
use crate::util::{ContextExt, Hash};

use derive_more::{Deref, DerefMut};
use eyre::Result;
use fieldmap::ClonedFieldMap;
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
};

const DATABASE_NAME: &str = "database.json";

#[derive(Serialize, Deserialize)]
pub struct Database {
    backups: BTreeMap<String, Backup>,
    data_blocks: DataBlocks,
}

/// POD struct with information about a data block in storage.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
pub struct DataBlock {
    pub hash: Hash,
    pub apparent_size: u64,
}

impl Database {
    pub fn new() -> Self {
        Self {
            backups: BTreeMap::new(),
            data_blocks: DataBlocks::new(),
        }
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().join(DATABASE_NAME);
        let f = BufReader::new(File::open(&path).context_2("reading db file", &path)?);
        let db = serde_json::from_reader(f)?;
        Ok(db)
    }

    pub fn write<P: AsRef<Path>>(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().join(DATABASE_NAME);
        let f = BufWriter::new(File::create(&path).context_2("writing db file", &path)?);
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
        let data_blocks = &self.data_blocks;
        Some(BackupView::new(name, backup, data_blocks))
    }

    pub fn get_data_block(&self, hash: Hash) -> Option<DataBlock> {
        self.data_blocks.get(&hash)?;
        todo!()
    }

    pub fn insert_backup_builder(&mut self, name: &str, bb: BackupBuilder) -> BackupView {
        for new_file in bb.iter_new_files() {
            let data_block = DataBlock {
                hash: new_file.hash,
                apparent_size: new_file.apparent_size,
            };
            let prev = self.data_blocks.insert(data_block);

            if let Some(prev) = prev {
                assert_eq!(
                    new_file.apparent_size, prev.apparent_size,
                    "two different sized data blocks with the same hash"
                );
            }
        }
        self.backups.insert(name.to_owned(), bb.into_inner());
        self.get_backup(name).unwrap()
    }
}

#[derive(Serialize, Deserialize, Deref, DerefMut)]
pub struct DataBlocks(
    #[serde(deserialize_with = "DataBlocks::deserialize")] ClonedFieldMap<DataBlock, Hash>,
);

impl DataBlocks {
    fn new() -> Self {
        Self(ClonedFieldMap::new(|datablock| &datablock.hash))
    }

    fn deserialize<'de, D>(deserializer: D) -> Result<ClonedFieldMap<DataBlock, Hash>, D::Error>
    where
        D: Deserializer<'de>,
    {
        ClonedFieldMap::deserialize(|datablock| &datablock.hash, deserializer)
    }
}
