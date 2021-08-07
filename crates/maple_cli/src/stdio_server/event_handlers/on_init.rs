use anyhow::Result;
use serde_json::json;

use crate::process::AsyncCommand;
use crate::stdio_server::{
    session::{EventHandler, Session},
    write_response,
};

pub async fn run<T: EventHandler>(
    msg_id: u64,
    source_cmd: String,
    session: Session<T>,
) -> Result<()> {
    let lines = AsyncCommand::new(source_cmd)
        .current_dir(&session.context.cwd)
        .lines()
        .await?;

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
