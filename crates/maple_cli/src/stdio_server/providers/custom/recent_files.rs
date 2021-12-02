use parking_lot::Mutex;
use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::Sender;
use serde::Deserialize;
use serde_json::json;

use filter::FilteredItem;

use crate::datastore::RECENT_FILES_IN_MEMORY;
use crate::stdio_server::{
    providers::builtin::OnMoveHandler,
    rpc::Call,
    session::{EventHandler, NewSession, Session, SessionContext, SessionEvent},
    write_response, MethodCall,
};

pub async fn handle_recent_files_message(
    msg: MethodCall,
    context: Arc<SessionContext>,
    force_execute: bool,
) -> Vec<FilteredItem> {
    let msg_id = msg.id;

    let cwd = context.cwd.to_string_lossy().to_string();

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
    } = msg.parse_unsafe();

    let mut recent_files = RECENT_FILES_IN_MEMORY.lock();

    let ranked = if query.is_empty() || force_execute {
        // Sort the initial list according to the cwd.
        //
        // This changes the order of existing recent file entries.
        recent_files.sort_by_cwd(&cwd);
        recent_files
            .entries
            .iter()
            .map(|entry| {
                FilteredItem::new(
                    entry.fpath.clone(),
                    entry.frecent_score as i64,
                    Default::default(),
                )
            })
            .collect::<Vec<_>>()
    } else {
        recent_files.filter_on_query(&query, cwd)
    };
    let initial_size = recent_files.len();

    let total = ranked.len();

    let mut preview = None;

    let winwidth = context.display_winwidth as usize;

    if let Some(lnum) = lnum {
        // process the new preview
        if let Some(new_entry) = ranked.get(lnum as usize - 1) {
            let new_curline = new_entry.display_text();
            if let Ok((lines, fname)) = crate::previewer::preview_file(
                new_curline,
                context.sensible_preview_size(),
                winwidth,
            ) {
                preview = Some(json!({
                  "lines": lines,
                  "fname": fname
                }));
            }
        }
    }

    // Take the first 200 entries and add an icon to each of them.
    let printer::DecoratedLines {
        lines,
        indices,
        truncated_map,
    } = printer::decorate_lines(
        ranked.iter().take(200).cloned().collect(),
        winwidth,
        if enable_icon.unwrap_or(true) {
            icon::Icon::Enabled(icon::IconKind::File)
        } else {
            icon::Icon::Null
        },
    );

    let result = if truncated_map.is_empty() {
        json!({
        "lines": lines,
        "indices": indices,
        "total": total,
        "initial_size": initial_size,
        "preview": preview,
        })
    } else {
        json!({
        "lines": lines,
        "indices": indices,
        "truncated_map": truncated_map,
        "total": total,
        "initial_size": initial_size,
        "preview": preview,
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
    lines: Arc<Mutex<Vec<FilteredItem>>>,
}

#[async_trait::async_trait]
impl EventHandler for RecentFilesMessageHandler {
    async fn handle_on_move(
        &mut self,
        msg: MethodCall,
        context: Arc<SessionContext>,
    ) -> Result<()> {
        let msg_id = msg.id;

        let lnum = msg.get_u64("lnum").expect("lnum is required");

        if let Some(curline) = self
            .lines
            .lock()
            .get((lnum - 1) as usize)
            .map(|r| r.source_item.raw.as_str())
        {
            if let Err(e) =
                OnMoveHandler::create(&msg, &context, Some(curline.into())).map(|x| x.handle())
            {
                tracing::error!(error = ?e, "Failed to handle OnMove event");
                write_response(json!({"error": e.to_string(), "id": msg_id }));
            }
        }
        Ok(())
    }

    async fn handle_on_typed(
        &mut self,
        msg: MethodCall,
        context: Arc<SessionContext>,
    ) -> Result<()> {
        let new_lines = tokio::spawn(handle_recent_files_message(msg, context, false))
            .await
            .unwrap_or_else(|e| {
                tracing::error!(error = ?e, "Failed to spawn task handle_recent_files_message");
                Default::default()
            });

        let mut lines = self.lines.lock();
        *lines = new_lines;

        Ok(())
    }
}

pub struct RecentFilesSession;

impl NewSession for RecentFilesSession {
    fn spawn(call: Call) -> Result<Sender<SessionEvent>> {
        let handler = RecentFilesMessageHandler::default();
        let lines_clone = handler.lines.clone();

        let (session, session_sender) = Session::new(call.clone(), handler);

        let context_clone = session.context.clone();

        session.start_event_loop();

        tokio::spawn(async move {
            let initial_lines =
                handle_recent_files_message(call.unwrap_method_call(), context_clone, true).await;

            let mut lines = lines_clone.lock();
            *lines = initial_lines;
        });

        Ok(session_sender)
    }
}
