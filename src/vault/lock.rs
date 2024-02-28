use std::{
    fs::{remove_file, OpenOptions},
    io,
    path::{Path, PathBuf},
};

use inotify::{Inotify, WatchMask};
use thiserror::Error;

const LOCKFILE_NAME: &str = "lock";

#[derive(Debug)]
pub struct DirectoryLock(PathBuf);

impl DirectoryLock {
    pub fn new(vault_dir: impl AsRef<Path>) -> Self {
        let path = vault_dir.as_ref().join(LOCKFILE_NAME);
        Self(path)
    }

    pub fn nonblocking_lock(&self) -> io::Result<Result<(), AlreadyLocked>> {
        let res = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.0);
        match res {
            Ok(_) => Ok(Ok(())),
            Err(io_err) if io_err.kind() == io::ErrorKind::AlreadyExists => Ok(Err(AlreadyLocked)),
            Err(io_err) => Err(io_err),
        }
    }

    pub fn blocking_lock(&self) -> io::Result<()> {
        loop {
            break match self.nonblocking_lock() {
                Ok(Ok(())) => Ok(()),
                Err(io_err) => Err(io_err),
                Ok(Err(AlreadyLocked)) => {
                    let mut inotify = Inotify::init()?;
                    inotify.watches().add(&self.0, WatchMask::DELETE_SELF)?;
                    let buf_size = inotify::get_buffer_size(&self.0)?;
                    inotify.read_events_blocking(vec![0; buf_size].as_mut_slice())?;
                    continue;
                }
            };
        }
    }

    pub fn unlock(&self) -> io::Result<()> {
        remove_file(&self.0)
    }
}

#[derive(Debug, Error)]
#[error("already locked")]
pub struct AlreadyLocked;
