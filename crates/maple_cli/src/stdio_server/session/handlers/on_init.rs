use std::ffi::OsStr;
use std::path::Path;

use anyhow::Result;
use serde_json::json;

use crate::stdio_server::{
    session::{HandleMessage, Session},
    write_response,
};

/// Collect the output of `source_cmd` asynchronously.
async fn gather_source<S: AsRef<OsStr>, P: AsRef<Path>>(
    source_cmd: S,
    cwd: P,
) -> Result<Vec<String>> {
    // FIXME: can not use `rg --column --line-number --no-heading --color=never --smart-case ''`
    //    let output: std::process::Output = tokio::process::Command::new("rg")
    // .args(&[
    // "--column",
    // "--line-number",
    // "--no-heading",
    // "--color=never",
    // "--smart-case",
    // "''",
    // ])
    let output: std::process::Output = tokio::process::Command::new(source_cmd.as_ref())
        .current_dir(cwd)
        .output()
        .await?;
    if !output.status.success() && !output.stderr.is_empty() {
        return Err(anyhow::anyhow!(
            "an error occured for command {:?}: {:?}",
            source_cmd.as_ref(),
            output.stderr
        ));
    }
    let stdout_string = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout_string
        .split('\n')
        .map(Into::into)
        .collect::<Vec<String>>();
    if lines.last().map(|s| s.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    Ok(lines)
}

pub async fn run<T: HandleMessage>(
    msg_id: u64,
    source_cmd: String,
    session: Session<T>,
) -> Result<()> {
    let lines = gather_source(source_cmd, &session.context.cwd).await?;

    if session.is_running() {
        // Send the forerunner result to client.
        let initial_size = lines.len();
        let response_lines = lines
            .iter()
            .by_ref()
            .take(30)
            .map(|line| icon::IconPainter::File.paint(&line))
            .collect::<Vec<_>>();
        write_response(json!({
        "id": msg_id,
        "provider_id": session.context.provider_id,
        "result": {
          "event": "on_init",
          "initial_size": initial_size,
          "lines": response_lines,
        }}));

        let mut session = session;
        session.set_source_list(lines);
    }

    Ok(())
}

#[tokio::test]
async fn test_tokio_command() {
    let lines = gather_source(
        "ls",
        std::env::current_dir()
            .unwrap()
            .into_os_string()
            .into_string()
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(vec!["Cargo.toml", "src"], lines);
}
