mod on_move;

use super::filer::read_dir_entries;
use super::*;
use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

pub type SessionId = u64;
pub type ProviderId = String;

#[derive(Debug)]
pub struct SessionEventSender(crossbeam_channel::Sender<SessionEvent>);

impl From<crossbeam_channel::Sender<SessionEvent>> for SessionEventSender {
    fn from(sender: crossbeam_channel::Sender<SessionEvent>) -> Self {
        Self(sender)
    }
}

impl SessionEventSender {
    pub fn send(&self, event: SessionEvent) {
        if let Err(e) = self.0.send(event) {
            error!("Failed to send session event, error: {:?}", e);
        }
    }
}

fn spawn_new_session(msg: Message) -> anyhow::Result<crossbeam_channel::Sender<SessionEvent>> {
    let (session_sender, session_receiver) = crossbeam_channel::unbounded();
    let msg_id = msg.id;

    let session = Session {
        session_id: msg.session_id,
        context: msg.into(),
        event_recv: session_receiver,
    };

    if session.context.source_cmd.is_some() {
        let session_cloned = session.clone();
        // TODO: choose different fitler strategy according to the time forerunner job spent.
        spawn_forerunner(msg_id, session_cloned)?;
    }

    session.start_event_loop()?;

    Ok(session_sender)
}

#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: HashMap<SessionId, SessionEventSender>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Default::default(),
        }
    }

    /// Start a session in a new thread given the session id and init message.
    pub fn new_session(&mut self, session_id: SessionId, msg: Message) {
        if self.has(session_id) {
            error!("Session {} already exists", msg.session_id);
        } else {
            match spawn_new_session(msg) {
                Ok(sender) => {
                    self.sessions.insert(session_id, sender.into());
                }
                Err(e) => {
                    error!("Couldn't spawn new session, error:{:?}", e);
                }
            }
        }
    }

    /// Returns true if the sessoion exists given the session_id.
    pub fn has(&self, session_id: SessionId) -> bool {
        self.sessions.contains_key(&session_id)
    }

    /// Send Terminate event to stop the thread of session.
    pub fn terminate(&mut self, session_id: SessionId) {
        if let Some(sender) = self.sessions.remove(&session_id) {
            sender.send(SessionEvent::Terminate);
        }
    }

    /// Dispatch the session event to the session thread accordingly.
    pub fn send(&self, session_id: SessionId, event: SessionEvent) {
        if let Some(sender) = self.sessions.get(&session_id) {
            sender.send(event);
        } else {
            error!(
                "Can't find session_id: {} in SessionManager: {:?}",
                session_id, self
            );
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub cwd: String,
    pub source_cmd: Option<String>,
    pub winwidth: Option<u64>,
    pub provider_id: ProviderId,
    pub start_buffer_path: Option<String>,
    pub is_running: Arc<Mutex<AtomicBool>>,
    pub source_list: Arc<Mutex<Option<Vec<String>>>>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: u64,
    pub context: SessionContext,
    pub event_recv: crossbeam_channel::Receiver<SessionEvent>,
}

impl From<Message> for SessionContext {
    fn from(msg: Message) -> Self {
        let provider_id = msg.get_provider_id();

        let cwd = String::from(
            msg.params
                .get("cwd")
                .and_then(|x| x.as_str())
                .unwrap_or("Missing cwd when deserializing into FilerParams"),
        );

        let source_cmd = msg
            .params
            .get("source_cmd")
            .and_then(|x| x.as_str().map(Into::into));

        let winwidth = msg.params.get("winwidth").and_then(|x| x.as_u64());

        let start_buffer_path = msg
            .params
            .get("source_fpath")
            .and_then(|x| x.as_str().map(Into::into));

        Self {
            provider_id,
            cwd,
            source_cmd,
            winwidth,
            start_buffer_path,
            is_running: Arc::new(Mutex::new(true.into())),
            source_list: Arc::new(Mutex::new(None)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    OnTyped(Message),
    OnMove(Message),
    Terminate,
}

fn spawn_forerunner_impl(msg_id: u64, session: Session) -> anyhow::Result<()> {
    let stdout_stream = filter::subprocess::Exec::shell(session.context.source_cmd.unwrap())
        .cwd(&session.context.cwd)
        .stream_stdout()?;

    let lines = std::io::BufReader::new(stdout_stream)
        .lines()
        .filter_map(|x| x.ok())
        .collect::<Vec<String>>();

    let is_running = session.context.is_running.lock().unwrap();

    if is_running.load(std::sync::atomic::Ordering::Relaxed) {
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

fn spawn_forerunner(msg_id: u64, session: Session) -> anyhow::Result<()> {
    thread::Builder::new()
        .name(format!("session-forerunner-{}", session.session_id))
        .spawn(move || spawn_forerunner_impl(msg_id, session))?;
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

    fn _handle_filer_impl(&self, msg: Message) -> anyhow::Result<()> {
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

            if truncated_map.is_empty() {
                write_response(json!({
                "id": msg_id,
                "provider_id": self.context.provider_id,
                "result": {
                  "event": "on_typed",
                  "total": total,
                  "lines": lines,
                  "indices": indices,
                }}));
            } else {
                write_response(json!({
                "id": msg_id,
                "provider_id": self.context.provider_id,
                "result": {
                  "event": "on_typed",
                  "total": total,
                  "lines": lines,
                  "indices": indices,
                  "truncated_map": truncated_map,
                }}));
            }
        }
    }

    fn handle_on_move(&self, msg: Message) {
        let msg_id = msg.id;
        if let Err(e) = on_move::OnMoveHandler::try_new(msg, &self.context).map(|x| x.handle()) {
            write_response(json!({ "error": format!("{}",e), "id": msg_id }));
        }
    }

    pub fn start_event_loop(mut self) -> anyhow::Result<()> {
        thread::Builder::new()
            .name(format!("session-{}", self.session_id))
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
