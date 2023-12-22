pub mod backup;
pub mod init;
pub mod list;

use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct GlobalArgs {
    #[arg(short, help = "Vault Directory")]
    vault_dir: Option<PathBuf>,
}
