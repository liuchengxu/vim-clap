mod context;
mod manager;
mod on_move;

use super::filer::read_dir_entries;
use super::*;
use anyhow::Result;
use context::SessionContext;
pub use manager::SessionManager;

pub type SessionId = u64;
pub type ProviderId = String;

#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: u64,
    pub context: SessionContext,
    pub event_recv: crossbeam_channel::Receiver<SessionEvent>,
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    OnTyped(Message),
    OnMove(Message),
    Terminate,
}

fn spawn_forerunner_impl(msg_id: u64, source_cmd: String, session: Session) -> Result<()> {
    let stdout_stream = filter::subprocess::Exec::shell(source_cmd)
        .cwd(&session.context.cwd)
        .stream_stdout()?;

    let lines = std::io::BufReader::new(stdout_stream)
        .lines()
        .filter_map(|x| x.ok())
        .collect::<Vec<String>>();

    if session.is_running() {
        let initial_size = lines.len();
        let response_lines = lines
            .iter()
            .by_ref()
            .take(30)
            .map(|line| icon::IconPainter::File.paint(&line))
            .collect::<Vec<_>>();

        let mut source_list = session.context.source_list.lock().unwrap();
        *source_list = Some(lines);

        write_response(json!({
        "id": msg_id,
        "provider_id": session.context.provider_id,
        "result": {
          "event": "on_init",
          "initial_size": initial_size,
          "lines": response_lines,
        }}));
    }

    Ok(())
}

fn spawn_forerunner(msg_id: u64, source_cmd: String, session: Session) -> Result<()> {
    thread::Builder::new()
        .name(format!("session-forerunner-{}", session.session_id))
        .spawn(move || spawn_forerunner_impl(msg_id, source_cmd, session))?;
    Ok(())
}

impl Session {
    pub fn handle_terminate(&mut self) {
        let mut val = self.context.is_running.lock().unwrap();
        *val.get_mut() = false;
    }

    /// This session is still running, hasn't received Terminate event.
    pub fn is_running(&self) -> bool {
        self.context
            .is_running
            .lock()
            .unwrap()
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn provider_id(&self) -> &str {
        &self.context.provider_id
    }

    fn _handle_filer_impl(&self, msg: Message) -> Result<()> {
        let enable_icon = super::env::global().enable_icon;
        let result = match read_dir_entries(&self.context.cwd, enable_icon, None) {
            Ok(entries) => json!({
            "id": msg.id,
            "provider_id": self.context.provider_id,
            "result": {
              "entries": entries,
              "dir": self.context.cwd,
              "total": entries.len(),
              "event": "on_typed",
            }}),
            Err(err) => json!({
            "id": msg.id,
            "provider_id": self.context.provider_id,
            "error": {
              "message": format!("{}", err),
              "dir": self.context.cwd
            }}),
        };

        write_response(result);

        Ok(())
    }

    pub fn handle_on_typed(&self, msg: Message) {
        debug!("recv OnTyped event: {:?}", msg);

        if msg.get_provider_id() == "filer" {
            let _ = self._handle_filer_impl(msg);
            return;
        }

        let msg_id = msg.id;
        let query = msg.get_query();

        let source_list = self.context.source_list.lock().unwrap();

        // TODO: sync for 100000, dyn for 100000+
        if let Some(ref source_list) = *source_list {
            let source = filter::Source::List(source_list.iter().map(Into::into));

            let lines_info = filter::sync_run(&query, source, filter::matcher::Algo::Fzy).unwrap();

            let total = lines_info.len();

            let (lines, indices, truncated_map) = printer::process_top_items(
                30,
                lines_info.into_iter().take(30),
                self.context.winwidth.map(|x| x as usize),
                Some(icon::IconPainter::File),
            );

            debug!(
                "indices size: {:?}, lines size: {:?}",
                indices.len(),
                lines.len()
            );

            let send_response = |result: serde_json::value::Value| {
                write_response(json!({
                "id": msg_id,
                "provider_id": self.context.provider_id,
                "result": result
                }));
            };

            if truncated_map.is_empty() {
                send_response(json!({
                  "event": "on_typed",
                  "total": total,
                  "lines": lines,
                  "indices": indices,
                }));
            } else {
                send_response(json!({
                  "event": "on_typed",
                  "total": total,
                  "lines": lines,
                  "indices": indices,
                  "truncated_map": truncated_map,
                }));
            }
        }
    }

    fn handle_on_move(&self, msg: Message) {
        let msg_id = msg.id;
        if let Err(e) = on_move::OnMoveHandler::try_new(msg, &self.context).map(|x| x.handle()) {
            write_response(json!({ "error": format!("{}",e), "id": msg_id }));
        }
    }

    pub fn start_event_loop(mut self) -> Result<()> {
        thread::Builder::new()
            .name(format!(
                "session-{}-{}",
                self.session_id,
                self.provider_id()
            ))
            .spawn(move || loop {
                match self.event_recv.recv() {
                    Ok(event) => {
                        debug!("session recv: {:?}", event);
                        match event {
                            SessionEvent::Terminate => {
                                self.handle_terminate();
                                debug!("session {} terminated", self.session_id);
                                return;
                            }
                            SessionEvent::OnMove(msg) => self.handle_on_move(msg),
                            SessionEvent::OnTyped(msg) => self.handle_on_typed(msg),
                        }
                    }
                    Err(err) => debug!("session recv error: {:?}", err),
                }
            })?;
        Ok(())
    }
}
