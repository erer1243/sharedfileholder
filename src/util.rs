use eyre::{ensure, Context, Result};
use serde::{de::Visitor, Deserialize, Serialize};
use std::{
    env::current_dir,
    fmt::{Debug, Display},
    fs::read_dir,
    io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

pub fn path_or_cwd(path: Option<PathBuf>) -> PathBuf {
    path.unwrap_or_else(|| current_dir().expect("current_dir"))
}

pub fn ensure_dir_exists_and_is_empty(path: &Path) -> Result<()> {
    let mut read_dir = read_dir(path).context_2("read_dir", path)?;
    ensure!(read_dir.next().is_none(), "{} is not empty", path.display());
    Ok(())
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

pub trait ContextExt<T, E>: Context<T, E> + Sized {
    fn path_context<P: AsRef<Path>>(self, path: P) -> Result<T> {
        self.with_context(|| format!("{}", path.as_ref().display()))
    }

    fn context_2<P: AsRef<Path>>(self, msg: &str, path: P) -> Result<T> {
        self.with_context(|| format!("{msg} ({})", path.as_ref().display()))
    }
}

impl<C: Context<T, E>, T, E> ContextExt<T, E> for C {}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct Hash(blake3::Hash);

impl Hash {
    pub fn of_file<P: AsRef<Path>>(path: P) -> io::Result<Hash> {
        let hash = blake3::Hasher::new().update_mmap(path)?.finalize();
        Ok(Hash(hash))
    }

    pub fn inner(&self) -> blake3::Hash {
        self.0
    }
}

impl Serialize for Hash {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.0.to_hex().as_str())
    }
}

impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        return deserializer.deserialize_str(HashVisitor);

        struct HashVisitor;
        impl<'de> Visitor<'de> for HashVisitor {
            type Value = Hash;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a 64-digit hex string")
            }

            fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                use serde::de::Unexpected;
                match v.parse() {
                    Ok(hash) => Ok(Hash(hash)),
                    Err(_) => Err(E::invalid_value(Unexpected::Str(v), &self)),
                }
            }
        }
    }
}

impl Debug for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.to_hex().as_str())
    }
}

impl Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
