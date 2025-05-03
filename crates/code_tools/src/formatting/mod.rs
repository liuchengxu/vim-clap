use std::path::Path;
use std::process::Stdio;

pub async fn run_cargo_fmt(workspace_root: &Path) -> std::io::Result<()> {
    let exit_status = tokio::process::Command::new("cargo")
        .arg("fmt")
        .arg("--all")
        .current_dir(workspace_root)
        .kill_on_drop(true)
        .spawn()?
        .wait()
        .await?;

    if exit_status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "error: {:?}",
            exit_status.code()
        )))
    }
}

pub async fn run_rustfmt(source_file: &Path, workspace_root: &Path) -> std::io::Result<()> {
    let exit_status = tokio::process::Command::new("rustfmt")
        .arg("--edition")
        .arg("2021")
        .arg(source_file)
        .current_dir(workspace_root)
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?
        .wait()
        .await?;

    if exit_status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "error: {:?}",
            exit_status.code()
        )))
    }
}
