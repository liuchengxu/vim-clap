use anyhow::{bail, Result};
use std::path::PathBuf;
use xshell::{cmd, Shell};

pub fn run(sh: &Shell, dry_run: bool, project_root: PathBuf) -> Result<()> {
    let path = project_root.join("CHANGELOG.md");
    let changelog = sh.read_file(path)?;
    let lines = changelog
        .split('\n')
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    let tag = cmd!(sh, "git rev-list --tags --max-count=1").read()?;
    let current_tag = cmd!(sh, "git describe --abbrev=0 --tags {tag}").read()?;

    let current_version: usize = current_tag.split('.').nth(1).unwrap().parse().unwrap();

    let previous_version = current_version - 1;

    let target_prefix = format!("## [0.{current_version}]");
    let current_tag_line_index = lines
        .iter()
        .position(|line| line.starts_with(&target_prefix))
        .unwrap_or_else(|| panic!("line prefixed with `## [0.{current_version}]` not found"));

    let target_prefix = format!("## [0.{previous_version}]");
    let previous_tag_line_index = lines
        .iter()
        .position(|line| line.starts_with(&target_prefix))
        .unwrap();

    let extracted_changelog = &lines[current_tag_line_index..previous_tag_line_index];

    println!(
        "Changelog for version v0.{current_version}:\n\n>>>>>>>>\n{}\n<<<<<<<<<",
        extracted_changelog.join("\n")
    );

    if !dry_run {
        do_update_release(sh, &current_tag, extracted_changelog.join("\n"))?;
    }

    Ok(())
}

// Adapted from https://github.com/rust-lang/rust-analyzer/blob/ac998a74b3c8ff4b81c3eeb9a18811d4cc76226d/xtask/src/publish.rs#L64
fn do_update_release(sh: &Shell, tag_name: &str, release_notes: String) -> anyhow::Result<()> {
    #[derive(Debug, serde::Deserialize)]
    struct Release {
        id: u32,
    }

    let token = match std::env::var("GH_TOKEN") {
        Ok(token) => token,
        Err(_) => bail!("Please obtain a personal access token from https://github.com/settings/tokens and set the `GH_TOKEN` environment variable."),
    };
    let accept = "Accept: application/vnd.github+json";
    let authorization = format!("Authorization: Bearer {token}");
    let api_version = "X-GitHub-Api-Version: 2022-11-28";
    let release_url = "https://api.github.com/repos/liuchengxu/vim-clap/releases";

    let release_json = cmd!(
        sh,
        "curl -sf -H {accept} -H {authorization} -H {api_version} {release_url}/tags/{tag_name}"
    )
    .read()?;
    let release: Release = serde_json::from_str(&release_json).unwrap();
    let release_id = release.id.to_string();

    let mut patch = String::new();

    // note: the GitHub API doesn't update the target commit if the tag already exists
    write_json::object(&mut patch)
        .string("tag_name", tag_name)
        .string("target_commitish", "master")
        .string("name", tag_name)
        .string("body", &release_notes)
        .bool("draft", false)
        .bool("prerelease", false);

    let _ = cmd!(sh,
      "curl -sf -X PATCH -H {accept} -H {authorization} -H {api_version} {release_url}/{release_id} -d {patch}"
    ).read()?;

    Ok(())
}
