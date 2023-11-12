use std::path::Path;

pub async fn run_cargo_fmt(workspace_root: &Path) -> std::io::Result<()> {
    tokio::process::Command::new("cargo")
        .arg("fmt")
        .arg("--all")
        .current_dir(workspace_root)
        .kill_on_drop(true)
        .spawn()?
        .wait()
        .await?;

    Ok(())
}
