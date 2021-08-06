use parking_lot::Mutex;
use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::Sender;
use serde::Deserialize;
use serde_json::json;

use crate::datastore::RECENT_FILES_IN_MEMORY;
use crate::stdio_server::event_handlers::OnMoveHandler;
use crate::stdio_server::{
    session::{Event, EventHandler, NewSession, Session, SessionContext, SessionEvent},
    write_response, Message,
};

pub async fn handle_recent_files_message(
    msg: Message,
    winwidth: u64,
    force_execute: bool,
) -> Vec<filter::FilteredItem> {
    let msg_id = msg.id;

    #[derive(Deserialize)]
    struct Params {
        query: String,
        enable_icon: Option<bool>,
        lnum: Option<u64>,
    }

    let Params {
        query,
        enable_icon,
        lnum,
    } = msg.deserialize_params_unsafe();

    let recent_files = RECENT_FILES_IN_MEMORY.lock();
    let ranked = recent_files.filter_on_query(&query);
    let initial_size = recent_files.len();

    let total = ranked.len();

    let mut preview_content = None;

    if let Some(lnum) = lnum {
        // process the new preview
        if let Some(new_entry) = ranked.get(lnum as usize - 1) {
            let new_curline = new_entry.display_text();
            if let Ok((preview_lines, preview_fname)) =
                crate::previewer::preview_file(new_curline, 100, 80)
            {
                preview_content = Some(json!({
                  "lines": preview_lines,
                  "fname": preview_fname
                }));
            }
        }
    }

    // Take the first 200 entries and add an icon to each of them.
    let (lines, indices, truncated_map) = printer::process_top_items(
        ranked.iter().take(200).cloned().collect(),
        winwidth as usize,
        if enable_icon.unwrap_or(true) {
            Some(icon::IconPainter::File)
        } else {
            None
        },
    );

    let result = if truncated_map.is_empty() {
        json!({
        "lines": lines,
        "indices": indices,
        "total": total,
        "initial_size": initial_size,
        "preview_content": preview_content,
        })
    } else {
        json!({
        "lines": lines,
        "indices": indices,
        "truncated_map": truncated_map,
        "total": total,
        "initial_size": initial_size,
        "preview_content": preview_content,
        })
    };

    let result = json!({
    "id": msg_id,
    "force_execute": force_execute,
    "provider_id": "recent_files",
    "result": result,
    });

    write_response(result);

    ranked
}

#[derive(Debug, Clone, Default)]
pub struct RecentFilesMessageHandler {
    lines: Arc<Mutex<Vec<filter::FilteredItem>>>,
}

#[async_trait::async_trait]
impl EventHandler for RecentFilesMessageHandler {
    async fn handle(&mut self, event: Event, context: SessionContext) -> Result<()> {
        match event {
            Event::OnMove(msg) => {
                let msg_id = msg.id;

                let lnum = msg.get_u64("lnum").expect("lnum is required");

                if let Some(curline) = self
                    .lines
                    .lock()
                    .get((lnum - 1) as usize)
                    .map(|r| r.source_item.raw.as_str())
                {
                    if let Err(e) = OnMoveHandler::try_new(&msg, &context, Some(curline.into()))
                        .map(|x| x.handle())
                    {
                        log::error!("Failed to handle OnMove event: {:?}", e);
                        write_response(json!({"error": e.to_string(), "id": msg_id }));
                    }
                }
            }
            Event::OnTyped(msg) => {
                let winwidth = context.display_winwidth;
                let new_lines = tokio::spawn(handle_recent_files_message(msg, winwidth, false))
                    .await
                    .unwrap_or_else(|e| {
                        log::error!(
                            "Failed to spawn a task for handle_dumb_jump_message: {:?}",
                            e
                        );
                        Default::default()
                    });

                let mut lines = self.lines.lock();
                *lines = new_lines;
            }
        }

        Ok(())
    }
}

pub struct RecentFilesSession;

impl NewSession for RecentFilesSession {
    fn spawn(msg: Message) -> Result<Sender<SessionEvent>> {
        let handler = RecentFilesMessageHandler::default();
        let lines_clone = handler.lines.clone();

        let (session, session_sender) = Session::new(msg.clone(), handler);

        let winwidth = session.context.display_winwidth;

        session.start_event_loop()?;

        tokio::spawn(async move {
            let initial_lines = handle_recent_files_message(msg, winwidth, true).await;

            let mut lines = lines_clone.lock();
            *lines = initial_lines;
        });

        Ok(session_sender)
    }
}
