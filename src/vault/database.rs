use super::backup::Backup;
use crate::util::ContextExt;

use eyre::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

const DATABASE_NAME: &str = "database.json";

#[derive(Serialize, Deserialize, Debug)]
pub struct Database {
    #[serde(skip)]
    path: PathBuf,
    backups: BTreeMap<String, Backup>,
}

impl Database {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().join(DATABASE_NAME);
        Self {
            path,
            backups: BTreeMap::new(),
        }
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().join(DATABASE_NAME);
        let f = BufReader::new(File::open(&path).context_2("reading db file", &path)?);
        let mut db: Database = serde_json::from_reader(f)?;
        db.path = path;
        Ok(db)
    }

    pub fn write(&self) -> Result<()> {
        let f = BufWriter::new(File::create(&self.path).context_2("writing db file", &self.path)?);
        serde_json::to_writer_pretty(f, self)?;
        Ok(())
    }

    pub fn iter_backups(&self) -> impl Iterator<Item = &Backup> {
        self.backups.values()
    }

    pub fn get_backup(&self, name: &str) -> Option<&Backup> {
        self.backups.get(name)
    }

    pub fn insert_backup(&mut self, name: &str, backup: Backup) {
        self.backups.insert(name.to_owned(), backup);
    }
}
