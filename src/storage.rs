use crate::ContextExt;
use blake3::Hash;
use eyre::{bail, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn hash_to_path(vault: &Path, hash: Hash) -> PathBuf {
    let mut path = vault.to_owned();
    let hex = hash.to_hex();
    let first_hex_byte = hex.split_at(2).0;
    path.push(first_hex_byte);
    path.push(hex.as_str());
    path
}

pub fn store_file(vault: &Path, source: &Path, hash: Hash) -> Result<()> {
    let dest = hash_to_path(vault, hash);
    if dest.try_exists().context_2("stat", &dest)? {
        bail!("File with hash {hash} already exists in vault. It should be skipped!");
    }
    fs::copy(source, &dest).with_context(|| format!("copying {source:?} to {dest:?}"))?;
    Ok(())
}

pub fn delete_file(vault: &Path, hash: Hash) -> Result<()> {
    fs::remove_file(hash_to_path(vault, hash)).context("remove_file")
}
