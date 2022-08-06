use std::sync::Arc;

use anyhow::Result;
use filter::{ParSource, SourceItem};
use matcher::ClapItem;
use parking_lot::Mutex;
use serde_json::json;

use crate::command::ctags::recursive_tags::build_recursive_ctags_cmd;
use crate::command::grep::RgTokioCommand;
use crate::process::tokio::TokioCommand;
use crate::stdio_server::session::{SessionContext, SourceScale};

/// Threshold for large scale.
const LARGE_SCALE: usize = 200_000;

/// Performs the initialization like collecting the source and total number of source items.
pub async fn initialize(context: Arc<SessionContext>) -> Result<SourceScale> {
    let to_scale = |lines: Vec<String>| {
        let total = lines.len();

        if total > LARGE_SCALE {
            SourceScale::Large(total)
        } else {
            let items = lines
                .into_iter()
                .map(|line| Arc::new(SourceItem::from(line)) as Arc<dyn ClapItem>)
                .collect::<Vec<_>>();
            SourceScale::Small { total, items }
        }
    };

    // Known providers.
    match context.provider_id.as_str() {
        "blines" => {
            let total =
                crate::utils::count_lines(std::fs::File::open(&context.start_buffer_path)?)?;
            return Ok(SourceScale::Cache {
                total,
                path: context.start_buffer_path.to_path_buf(),
            });
        }
        "tags" => {
            let items = crate::tools::ctags::buffer_tag_items(&context.start_buffer_path, false)?;
            return Ok(SourceScale::Small {
                total: items.len(),
                items,
            });
        }
        "proj_tags" => {
            let ctags_cmd = build_recursive_ctags_cmd(context.cwd.to_path_buf());
            let scale = if context.no_cache {
                let lines = ctags_cmd.par_formatted_lines()?;
                ctags_cmd.create_cache_async(lines.clone()).await?;
                to_scale(lines)
            } else {
                match ctags_cmd.ctags_cache() {
                    Some((total, path)) => SourceScale::Cache { total, path },
                    None => {
                        let lines = ctags_cmd.par_formatted_lines()?;
                        ctags_cmd.create_cache_async(lines.clone()).await?;
                        to_scale(lines)
                    }
                }
            };
            return Ok(scale);
        }
        "grep2" => {
            let rg_cmd = RgTokioCommand::new(context.cwd.to_path_buf());
            let (total, path) = if context.no_cache {
                rg_cmd.create_cache().await?
            } else {
                match rg_cmd.cache_info() {
                    Some(cache) => cache,
                    None => rg_cmd.create_cache().await?,
                }
            };
            let method = "clap#state#set_variable_string";
            let name = "g:__clap_forerunner_tempfile";
            let value = &path;
            utility::println_json_with_length!(method, name, value);
            return Ok(SourceScale::Cache { total, path });
        }
        _ => {}
    }

    if let Some(ref source_cmd) = context.source_cmd {
        // TODO: check cache

        // Can not use subprocess::Exec::shell here.
        //
        // Must use TokioCommand otherwise the timeout may not work.
        let lines = TokioCommand::new(source_cmd)
            .current_dir(&context.cwd)
            .lines()
            .await?;

        return Ok(to_scale(lines));
    }

    Ok(SourceScale::Indefinite)
}
