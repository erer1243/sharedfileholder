pub mod backup;
pub mod database;
pub mod lock;
pub mod storage;

use eyre::{Context, Result};
use std::path::{Path, PathBuf};

use database::Database;
use storage::Storage;

use lock::DirectoryLock;

#[derive(Debug)]
pub struct Vault {
    pub database: Database,
    pub storage: Storage,
    lock: DirectoryLock,
}

impl Vault {
    pub fn open_cwd(vault_dir: Option<PathBuf>) -> Result<Self> {
        let vault_dir = vault_dir.unwrap_or_else(|| std::env::current_dir().expect("current_dir"));
        Self::open(vault_dir)
    }

    pub fn open(vault_dir: impl AsRef<Path>) -> Result<Self> {
        let vault_dir = vault_dir.as_ref();
        let lock = DirectoryLock::new(vault_dir);
        lock.blocking_lock()?;

        let database = Database::load(vault_dir).context("Loading database")?;
        let storage = Storage::new(vault_dir);
        Ok(Vault {
            database,
            storage,
            lock,
        })
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        if let Err(e) = self.lock.unlock() {
            eprintln!("Unlocking vault failed:");
            eprintln!("{e}");
        }
    }
}
