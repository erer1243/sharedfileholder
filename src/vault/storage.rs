use eyre::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

use crate::util::{ContextExt, Hash};

const DATA_DIR_NAME: &str = "data";

pub struct Storage {
    data_dir: PathBuf,
}

impl Storage {
    pub fn new(vault_dir: impl AsRef<Path>) -> Self {
        Self {
            data_dir: vault_dir.as_ref().join(DATA_DIR_NAME),
        }
    }

    pub fn path_of(&self, hash: Hash) -> PathBuf {
        let hex = hash.inner().to_hex();
        let first_hex_byte = hex.split_at(2).0;
        let mut path = self.data_dir.clone();
        path.push(first_hex_byte);
        path.push(hex.as_str());
        path
    }

    pub fn insert_file(&self, source: &Path, hash: Hash) -> Result<()> {
        let dest = self.path_of(hash);

        if dest.try_exists().context_2("stat", &dest)? {
            return Ok(());
        }

        let dir = dest.parent().unwrap();
        if !dir.exists() {
            fs::create_dir(dest.parent().unwrap()).context_2("mkdir", dir)?;
        }

        let source_disp = source.display();
        let dest_disp = dest.display();
        fs::copy(source, &dest).with_context(|| format!("copying {source_disp} to {dest_disp}"))?;
        Ok(())
    }

    pub fn insert_iter(
        &self,
        iter: impl IntoIterator<Item = (impl AsRef<Path>, Hash)>,
    ) -> Result<()> {
        for (source, hash) in iter {
            self.insert_file(source.as_ref(), hash)
                .context_2("inserting file into storage", source)?;
        }
        Ok(())
    }

    pub fn delete_file(&self, hash: Hash) -> Result<()> {
        let path = self.path_of(hash);
        fs::remove_file(&path).context_2("remove_file", path)
    }

    pub fn iter_files(&self) -> impl Iterator<Item = Result<PathBuf>> {
        WalkDir::new(&self.data_dir)
            .into_iter()
            .skip(1)
            .filter_map(|res| match res {
                Ok(dir_entry) => match dir_entry.metadata().map(|m| m.is_file()) {
                    Ok(true) => Some(Ok(dir_entry.into_path())),
                    Ok(false) => None,
                    Err(e) => Some(Err(e.into())),
                },
                Err(e) => Some(Err(e.into())),
            })
    }
}
