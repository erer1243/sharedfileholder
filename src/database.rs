use blake3::Hash;
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::File,
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

#[derive(Serialize, Deserialize, Default)]
pub struct Database {
    backups: BTreeMap<String, Backup>,
    data_blocks: HashMap<Hash, DataBlockMetadata>,
}

const DATABASE_NAME: &str = "database.ron";

impl Database {
    pub fn load_from_vault<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::load(path.as_ref().join(DATABASE_NAME))
    }

    pub fn write_to_vault<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        self.write(path.as_ref().join(DATABASE_NAME))
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let f = BufReader::new(File::open(path)?);
        Ok(ron::de::from_reader(f)?)
    }

    pub fn write<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let f = BufWriter::new(File::create(path)?);
        ron::ser::to_writer_pretty(f, self, ron::ser::PrettyConfig::default())?;
        Ok(())
    }

    pub fn get_backup(&self, name: &str) -> Option<BackupView> {
        let backup = self.backups.get(name)?;
        Some(BackupView {
            backup,
            data_blocks: &self.data_blocks,
        })
    }

    pub fn insert_backup_builder(&mut self, name: &str, bb: BackupBuilder) -> BackupView {
        self.backups.insert(name.to_owned(), bb.inner);
        for (_, hash, mtime) in bb.new_files {
            let prev = self.data_blocks.insert(hash, DataBlockMetadata { mtime });
            assert!(prev.is_none());
        }
        self.get_backup(name).unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct Backup {
    files: BTreeMap<u64, BackupFileMetadata>,
    directories: BTreeSet<PathBuf>,
    symlinks: BTreeMap<PathBuf, PathBuf>,
}

#[derive(Serialize, Deserialize, Debug)]
struct BackupFileMetadata {
    path: PathBuf,
    hash: Hash,
}

#[derive(Serialize, Deserialize)]
struct DataBlockMetadata {
    mtime: MTime,
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Debug)]
pub struct MTime {
    sec: u64,
    nano: u32,
}

impl PartialOrd for MTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MTime {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let a = Duration::new(self.sec, self.nano);
        let b = Duration::new(other.sec, other.nano);
        a.cmp(&b)
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

pub struct BackupView<'a> {
    backup: &'a Backup,
    data_blocks: &'a HashMap<Hash, DataBlockMetadata>,
}

impl<'a> BackupView<'a> {
    pub fn get_file(&self, ino: u64) -> Option<BackupFile> {
        let meta = self.backup.files.get(&ino)?;
        let data_block_mtime = self
            .data_blocks
            .get(&meta.hash)
            .unwrap_or_else(|| panic!("inode {ino} in backup but has no data_blocks entry"))
            .mtime;

        Some(BackupFile {
            ino,
            meta,
            data_block_mtime,
        })
    }
}

pub struct BackupFile<'a> {
    meta: &'a BackupFileMetadata,
    ino: u64,
    data_block_mtime: MTime,
}

impl<'a> BackupFile<'a> {
    pub fn ino(&self) -> u64 {
        self.ino
    }

    pub fn path(&self) -> &PathBuf {
        &self.meta.path
    }

    pub fn hash(&self) -> Hash {
        self.meta.hash
    }

    pub fn data_block_mtime(&self) -> MTime {
        self.data_block_mtime
    }
}

#[derive(Default, Debug)]
pub struct BackupBuilder {
    inner: Backup,
    new_files: Vec<(PathBuf, Hash, MTime)>,
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
        source: PathBuf,
        path: PathBuf,
        hash: Hash,
        ino: u64,
        mtime: MTime,
    ) {
        let mbf = BackupFileMetadata { path, hash };
        self.inner.files.insert(ino, mbf);
        self.new_files.push((source, hash, mtime));
    }

    pub fn insert_unchanged_file(&mut self, path: PathBuf, hash: Hash, ino: u64) {
        let mbf = BackupFileMetadata { path, hash };
        self.inner.files.insert(ino, mbf);
    }

    pub fn iter_new_files(&self) -> impl Iterator<Item = (&Path, Hash, MTime)> {
        self.new_files
            .iter()
            .map(|(pb, h, mt)| (pb.as_ref(), *h, *mt))
    }
}
