use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs::File,
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, SystemTime},
};

use blake3::Hash;
use eyre::Result;
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;

slotmap::new_key_type! { struct BackupFileKey; }

#[derive(Default, Debug)]
pub struct BackupView {
    directories: BTreeSet<PathBuf>,
    symlinks: BTreeMap<PathBuf, PathBuf>,

    files: SlotMap<BackupFileKey, BackupFile>,
    files_by_inode: HashMap<u64, BackupFileKey>,
}

impl BackupView {
    fn expand(mini: MinimizedBackup, db: &Database) -> Self {
        let mut files = SlotMap::with_capacity_and_key(mini.files.len());
        let mut files_by_inode = HashMap::with_capacity(mini.files.len());

        for (path, file) in mini.files {
            let key = files.insert(BackupFile {
                path: path.clone(),
                hash: file.hash,
                source_inode: file.source_inode,
                data_block_mtime: db.get_data_block_mtime(file.hash),
            });
            files_by_inode.insert(file.source_inode, key);
        }

        Self {
            directories: mini.directories,
            symlinks: mini.symlinks,
            files,
            files_by_inode,
        }
    }

    pub fn file_by_inode(&self, ino: u64) -> Option<&BackupFile> {
        self.files_by_inode
            .get(&ino)
            .map(|key| self.files.get(*key).unwrap())
    }
}

#[derive(Debug)]
pub struct BackupFile {
    pub path: PathBuf,
    pub hash: Hash,
    pub source_inode: u64,
    pub data_block_mtime: MTime,
}

#[derive(Default, Debug)]
pub struct BackupBuilder {
    mini: MinimizedBackup,
}

impl BackupBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_directory(&mut self, path: PathBuf) {
        self.mini.directories.insert(path);
    }

    pub fn insert_symlink(&mut self, path: PathBuf, target: PathBuf) {
        self.mini.symlinks.insert(path, target);
    }

    pub fn insert_file(&mut self, path: PathBuf, hash: Hash, source_inode: u64) {
        let mbf = MinimizedBackupFile { hash, source_inode };
        self.mini.files.insert(path, mbf);
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Database {
    backups: BTreeMap<String, MinimizedBackup>,
    data_blocks: HashMap<Hash, DataBlockMetadata>,
}

impl Database {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let f = BufReader::new(File::open(path)?);
        Ok(serde_json::from_reader(f)?)
    }

    pub fn write<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let f = BufWriter::new(File::create(path)?);
        serde_json::to_writer(f, self)?;
        Ok(())
    }

    pub fn take_backup(&mut self, name: &str) -> Option<BackupView> {
        self.backups
            .remove(name)
            .map(|mini| BackupView::expand(mini, self))
    }

    pub fn insert_backup_builder(&mut self, name: &str, bb: BackupBuilder) {
        self.backups.insert(name.to_owned(), bb.mini);
    }

    fn get_data_block_mtime(&self, hash: Hash) -> MTime {
        self.data_blocks.get(&hash).unwrap().mtime
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct MinimizedBackup {
    files: BTreeMap<PathBuf, MinimizedBackupFile>,
    directories: BTreeSet<PathBuf>,
    symlinks: BTreeMap<PathBuf, PathBuf>,
}

#[derive(Serialize, Deserialize, Debug)]
struct MinimizedBackupFile {
    hash: Hash,
    source_inode: u64,
}

#[derive(Serialize, Deserialize)]
struct DataBlockMetadata {
    mtime: MTime,
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Ord, Debug)]
pub struct MTime {
    sec: u64,
    nano: u32,
}

impl MTime {
    fn as_duration(self) -> Duration {
        Duration::new(self.sec, self.nano)
    }
}

impl PartialOrd for MTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_duration().partial_cmp(&other.as_duration())
    }
}

impl From<SystemTime> for MTime {
    fn from(st: SystemTime) -> Self {
        let dur = st.duration_since(SystemTime::UNIX_EPOCH).unwrap();
        MTime {
            sec: dur.as_secs(),
            nano: dur.subsec_nanos(),
        }
    }
}
