mod release;
mod update_release_notes;

use anyhow::Result;
use clap::Parser;
use std::path::{Path, PathBuf};
use xshell::Shell;

#[derive(Parser, Debug)]
enum Cmd {
    /// Publish a new release on GitHub.
    #[clap(name = "release")]
    Release {
        #[clap(long)]
        dry_run: bool,
    },
    /// Update the release notes of latest GitHub release.
    #[clap(name = "update-release-notes")]
    UpdateReleaseNotes {
        #[clap(long)]
        dry_run: bool,
    },
}

fn main() -> Result<()> {
    let sh = &Shell::new()?;

    let project_root = project_root();

    match Cmd::parse() {
        Cmd::Release { dry_run } => release::run(sh, dry_run, project_root)?,
        Cmd::UpdateReleaseNotes { dry_run } => {
            update_release_notes::run(sh, dry_run, project_root)?;
        }
    }

    Ok(())
}

// ~/.vim/plugged/vim-clap
fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}
