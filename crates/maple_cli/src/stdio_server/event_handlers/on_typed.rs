use log::debug;
use serde_json::json;

use filter::matcher::{Bonus, FuzzyAlgorithm, MatchType};

use crate::stdio_server::{session::SessionContext, write_response, Message};

pub fn handle_on_typed(msg: Message, context: &SessionContext) {
    if msg.get_provider_id().as_str() == "filer" {
        // let _ = self._handle_filer_impl(msg);
        return;
    }

    let msg_id = msg.id;
    let query = msg.get_query();

    let source_list = context.source_list.lock().unwrap();

    // TODO: sync for 100000, dyn for 100000+
    if let Some(ref source_list) = *source_list {
        let source = filter::Source::List(source_list.iter().map(|s| s.to_string().into()));

        let match_type = MatchType::Full;
        let bonus = match msg.get_provider_id().as_str() {
            "files" | "git_files" => Bonus::FileName,
            _ => Bonus::None,
        };
        let lines_info =
            filter::sync_run(&query, source, FuzzyAlgorithm::Fzy, match_type, vec![bonus]).unwrap();

        let total = lines_info.len();

        let (lines, indices, truncated_map) = printer::process_top_items(
            lines_info.into_iter().take(30).collect(),
            context.display_winwidth as usize,
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
            "provider_id": context.provider_id,
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
