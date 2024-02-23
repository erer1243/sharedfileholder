pub mod backup;
pub mod database;
pub mod storage;

use database::Database;
use storage::Storage;

pub struct Vault {
    database: Database,
    storage: Storage,
}

// pub enum Error {

// }
