use eyre::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

use crate::util::{ContextExt, Hash};

fn hash_to_path(base: &Path, hash: Hash) -> PathBuf {
    let hex = hash.inner().to_hex();
    let first_hex_byte = hex.split_at(2).0;
    let mut path = base.join(first_hex_byte);
    path.push(hex.as_str());
    path
}

pub fn store_file(vault: &Path, source: &Path, hash: Hash) -> Result<()> {
    let dest = hash_to_path(vault, hash);

    if dest.try_exists().context_2("stat", &dest)? {
        return Ok(());
    }

    let dir = dest.parent().unwrap();
    if !dir.exists() {
        fs::create_dir(dest.parent().unwrap()).context_2("mkdir", dir)?;
    }

    fs::copy(source, &dest).with_context(|| format!("copying {source:?} to {dest:?}"))?;
    Ok(())
}

pub fn delete_file(vault: &Path, hash: Hash) -> Result<()> {
    fs::remove_file(hash_to_path(vault, hash)).context("remove_file")
}

pub fn iter_files(vault: &Path) -> impl Iterator<Item = Result<PathBuf>> {
    WalkDir::new(vault)
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
