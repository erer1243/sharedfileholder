use eyre::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

use crate::util::{ContextExt, Hash};

pub struct Storage<'a> {
    vault_dir: &'a Path,
}

impl<'a> Storage<'a> {
    pub fn new(vault_dir: &'a Path) -> Self {
        Self { vault_dir }
    }

    pub fn path_of(&self, hash: Hash) -> PathBuf {
        let hex = hash.inner().to_hex();
        let first_hex_byte = hex.split_at(2).0;
        let mut path = self.vault_dir.to_owned();
        path.push("data");
        path.push(first_hex_byte);
        path.push(hex.as_str());
        path
    }

    pub fn insert_file(&self, path: &Path, hash: Hash) -> Result<()> {
        let dest = self.path_of(hash);

        if dest.try_exists().context_2("stat", &dest)? {
            return Ok(());
        }

        let dir = dest.parent().unwrap();
        if !dir.exists() {
            fs::create_dir(dest.parent().unwrap()).context_2("mkdir", dir)?;
        }

        let path_disp = path.display();
        let dest_disp = dest.display();
        fs::copy(path, &dest).with_context(|| format!("copying {path_disp} to {dest_disp}"))?;
        Ok(())
    }

    pub fn delete_file(&self, hash: Hash) -> Result<()> {
        let path = self.path_of(hash);
        fs::remove_file(&path).context_2("remove_file", path)
    }

    pub fn iter_files(&self) -> impl Iterator<Item = Result<PathBuf>> {
        WalkDir::new(&self.vault_dir)
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

// pub fn store_file(_: &Path, _: &Path, _: Hash) -> Result<()> {
//     unimplemented!()
// }

// pub fn path_of_hash(_: &Path, _: Hash) -> PathBuf {
//     unimplemented!()
// }
