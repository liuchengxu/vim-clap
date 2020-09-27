use super::*;

pub(super) fn run_forerunner<T: super::handler::HandleMessage>(
    msg_id: u64,
    source_cmd: String,
    session: Session<T>,
) -> Result<()> {
    let stdout_stream = filter::subprocess::Exec::shell(source_cmd)
        .cwd(&session.context.cwd)
        .stream_stdout()?;

    let lines = std::io::BufReader::new(stdout_stream)
        .lines()
        .filter_map(|x| x.ok())
        .collect::<Vec<String>>();

    // Send the forerunner results to the client if the session is still running.
    if session.is_running() {
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
