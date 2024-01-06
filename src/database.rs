use crate::{
    backup::{Backup, BackupBuilder, BackupView},
    util::{ContextExt, Hash, MTime},
};
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
};

#[derive(Serialize, Deserialize, Default)]
pub struct Database {
    backups: BTreeMap<String, Backup>,
    data_blocks: HashMap<Hash, DataBlockMetadata>,
}

/// DataBlock but
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct DataBlockMetadata {
    mtime: MTime,
    size: u64,
}

/// POD struct with information about a data block in storage.
pub struct DataBlock {
    pub hash: Hash,
    pub mtime: MTime,
    pub size: u64,
}

const DATABASE_NAME: &str = "database.json";

impl Database {
    pub fn load_from_vault<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().join(DATABASE_NAME);
        let f = BufReader::new(File::open(&path).context_2("reading db file", &path)?);
        // let db = ron::de::from_reader(f)?;
        let db = serde_json::from_reader(f)?;
        Ok(db)
    }

    pub fn write_to_vault<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref().join(DATABASE_NAME);
        let f = BufWriter::new(File::create(&path).context_2("writing db file", &path)?);
        // ron::ser::to_writer_pretty(f, self, ron::ser::PrettyConfig::default())?;
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
            let metadata = DataBlockMetadata {
                mtime: new_file.mtime,
                size: new_file.size,
            };
            let prev = self.data_blocks.insert(new_file.hash, metadata);
            assert!(prev.is_none());
        }
        self.backups.insert(name.to_owned(), bb.into_inner());
        self.get_backup(name).unwrap()
    }
}
