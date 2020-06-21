use super::*;
use log::debug;

pub fn handle_on_typed(msg: Message, context: &SessionContext) {
    debug!("recv OnTyped event: {:?}", msg);

    if msg.get_provider_id().as_str() == "filer" {
        // let _ = self._handle_filer_impl(msg);
        return;
    }

    let msg_id = msg.id;
    let query = msg.get_query();

    let source_list = context.source_list.lock().unwrap();

    // TODO: sync for 100000, dyn for 100000+
    if let Some(ref source_list) = *source_list {
        let source = filter::Source::List(source_list.iter().map(Into::into));

        let lines_info = filter::sync_run(&query, source, filter::matcher::Algo::Fzy).unwrap();

        let total = lines_info.len();

        let (lines, indices, truncated_map) = printer::process_top_items(
            30,
            lines_info.into_iter().take(30),
            context.winwidth.map(|x| x as usize),
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
