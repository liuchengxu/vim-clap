use matcher::ClapItem;
use parking_lot::Mutex;
use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::Sender;
use serde::Deserialize;
use serde_json::json;

use types::{MatchedItem, Score};

use crate::datastore::RECENT_FILES_IN_MEMORY;
use crate::stdio_server::impls::OnMoveHandler;
use crate::stdio_server::rpc::Call;
use crate::stdio_server::session::{ClapProvider, Session, SessionContext, SessionEvent};
use crate::stdio_server::{write_response, MethodCall};

async fn handle_recent_files_message(
    msg: MethodCall,
    context: Arc<SessionContext>,
    force_execute: bool,
) -> Vec<MatchedItem> {
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

        let mut cwd = cwd.clone();
        cwd.push(std::path::MAIN_SEPARATOR);

        recent_files
            .entries
            .iter()
            .map(|entry| {
                let item: Arc<dyn ClapItem> = Arc::new(entry.fpath.replacen(&cwd, "", 1));
                // frecent_score will not be larger than i32::MAX.
                MatchedItem::new(item, entry.frecent_score as Score, Default::default())
            })
            .collect::<Vec<_>>()
    } else {
        recent_files.filter_on_query(&query, cwd.clone())
    };
    let initial_size = recent_files.len();

    let total = ranked.len();

    let mut preview = None;

    let winwidth = context.display_winwidth as usize;

    if let Some(lnum) = lnum {
        // process the new preview
        if let Some(new_entry) = ranked.get(lnum as usize - 1) {
            let new_curline = new_entry.display_text().to_string();
            if let Ok((lines, fname)) = crate::previewer::preview_file(
                new_curline,
                context.sensible_preview_size(),
                winwidth,
            ) {
                preview = Some(json!({ "lines": lines, "fname": fname }));
            }
        }
    }

    // Take the first 200 entries and add an icon to each of them.
    let printer::DisplayLines {
        lines,
        indices,
        truncated_map,
        icon_added,
    } = printer::decorate_lines(
        ranked.iter().take(200).cloned().collect(),
        winwidth,
        if enable_icon.unwrap_or(true) {
            icon::Icon::Enabled(icon::IconKind::File)
        } else {
            icon::Icon::Null
        },
    );

    let mut cwd = cwd;
    cwd.push(std::path::MAIN_SEPARATOR);

    let lines = lines
        .into_iter()
        .map(|abs_path| abs_path.replacen(&cwd, "", 1))
        .collect::<Vec<_>>();

    let result = if truncated_map.is_empty() {
        json!({
            "lines": lines,
            "indices": indices,
            "total": total,
            "icon_added": icon_added,
            "initial_size": initial_size,
            "preview": preview,
        })
    } else {
        json!({
            "lines": lines,
            "indices": indices,
            "truncated_map": truncated_map,
            "total": total,
            "icon_added": icon_added,
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

#[derive(Debug)]
pub struct RecentFilesHandle {
    context: Arc<SessionContext>,
    lines: Arc<Mutex<Vec<MatchedItem>>>,
}

impl RecentFilesHandle {
    pub fn new(context: SessionContext) -> Self {
        Self {
            context: Arc::new(context),
            lines: Default::default(),
        }
    }
}

#[async_trait::async_trait]
impl ClapProvider for RecentFilesHandle {
    fn session_context(&self) -> &SessionContext {
        &self.context
    }

    async fn on_create(&mut self, call: Call) {
        let initial_lines =
            handle_recent_files_message(call.unwrap_method_call(), self.context.clone(), true)
                .await;

        let mut lines = self.lines.lock();
        *lines = initial_lines;
    }

    async fn on_move(&mut self, msg: MethodCall) -> Result<()> {
        let msg_id = msg.id;

        let lnum = msg.get_u64("lnum").expect("lnum is required");

        let maybe_curline = self
            .lines
            .lock()
            .get((lnum - 1) as usize)
            .map(|r| r.item.raw_text().to_string());

        if let Some(curline) = maybe_curline {
            let on_move_handler = OnMoveHandler::create(&msg, &self.context, Some(curline))?;
            if let Err(e) = on_move_handler.handle().await {
                tracing::error!(error = ?e, "Failed to handle OnMove event");
                write_response(json!({"error": e.to_string(), "id": msg_id }));
            }
        }
        Ok(())
    }

    async fn on_typed(&mut self, msg: MethodCall) -> Result<()> {
        let new_lines = tokio::spawn(handle_recent_files_message(
            msg,
            self.context.clone(),
            false,
        ))
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
