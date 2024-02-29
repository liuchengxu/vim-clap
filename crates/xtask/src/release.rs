use anyhow::{bail, Result};
use chrono::Datelike;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};
use xshell::{cmd, Shell};

pub fn run(sh: &Shell, dry_run: bool, project_root: PathBuf) -> Result<()> {
    let tag = cmd!(sh, "git rev-list --tags --max-count=1").read()?;
    let current_tag = cmd!(sh, "git describe --abbrev=0 --tags {tag}").read()?;
    let current_version: usize = current_tag.split('.').nth(1).unwrap().parse().unwrap();
    println!("    current tag: {current_tag:?}");
    println!("current version: {current_version}");
    println!("   project root: {}", project_root.display());

    sh.change_dir(&project_root);

    // install.ps1
    let target_line = format!("$version = 'v0.{current_version}'");
    bump_version(
        sh,
        project_root.join("install.ps1"),
        target_line,
        current_version,
    )?;

    // plugin/clap.vim
    let target_line = format!("\" Version:   0.{current_version}");
    bump_version(
        sh,
        project_root.join("plugin").join("clap.vim"),
        target_line,
        current_version,
    )?;

    // Cargo.toml
    let target_line = format!("version = \"0.1.{current_version}\"");
    bump_version(
        sh,
        project_root.join("Cargo.toml"),
        target_line,
        current_version,
    )?;

    // CHANGELOG.md
    let path = project_root.join("CHANGELOG.md");
    let file = sh.read_file(&path)?;
    let mut lines = file.split('\n').map(|s| s.to_string()).collect::<Vec<_>>();

    let index = lines
        .iter()
        .position(|line| line == "## [unreleased]")
        .unwrap();

    let current_date = chrono::Utc::now();
    let version_line = format!(
        "## [0.{}] {}-{}-{}",
        current_version + 1,
        current_date.year(),
        current_date.month(),
        current_date.day()
    );

    lines.insert(index + 1, "".to_string());
    lines.insert(index + 2, version_line);

    fs::write(path, lines.join("\n"))?;

    if !dry_run {
        // Update Cargo.lock.
        let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let status = Command::new(cargo)
            .current_dir(&project_root)
            .args(["build", "--release"])
            .status()?;

        if !status.success() {
            bail!("cargo build failed");
        }

        let Ok(token) = std::env::var("GH_TOKEN") else {
            bail!("Please obtain a personal access token from https://github.com/settings/tokens and set the `GH_TOKEN` environment variable.")
        };

        cmd!(sh, "git add -u").run()?;
        let next_version = current_version + 1;
        let next_version = next_version.to_string();
        cmd!(sh, "git commit -m v0.{next_version}").run()?;
        cmd!(
            sh,
            "git push https://{token}@github.com/liuchengxu/vim-clap master"
        )
        .run()?;
        cmd!(sh, "git tag v0.{next_version}").run()?;
        cmd!(
            sh,
            "git push https://{token}@github.com/liuchengxu/vim-clap v0.{next_version}"
        )
        .run()?;
    }

    Ok(())
}

// Find the target_line in the file and update the line by incrementing the version.
fn bump_version(
    sh: &Shell,
    path: PathBuf,
    target_line: String,
    current_version: usize,
) -> Result<()> {
    let file = sh.read_file(&path)?;
    let next_version = current_version + 1;
    let current_version = current_version.to_string();
    let next_version = next_version.to_string();
    let mut lines = file.split('\n').map(|s| s.to_string()).collect::<Vec<_>>();
    if lines
        .iter_mut()
        .try_for_each(|line| {
            if line.starts_with(&target_line) {
                *line = line.replace(&current_version, &next_version);
                return Err(());
            }
            Ok(())
        })
        .is_ok()
    {
        bail!("line `{target_line}` not found in {}", path.display())
    }

    fs::write(path, lines.join("\n"))?;

    Ok(())
}
