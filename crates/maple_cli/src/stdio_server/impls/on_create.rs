use std::sync::Arc;

use anyhow::Result;
use serde_json::json;

use filter::SourceItem;
use matcher::ClapItem;

use crate::command::ctags::recursive_tags::build_recursive_ctags_cmd;
use crate::command::grep::RgTokioCommand;
use crate::process::ShellCommand;
use crate::stdio_server::session::{ProviderSource, SessionContext};
use crate::stdio_server::vim::Vim;

/// Performs the initialization like collecting the source and total number of source items.
pub async fn initialize_provider_source(
    context: &SessionContext,
    vim: &Vim,
) -> Result<ProviderSource> {
    let to_small_provider_source = |lines: Vec<String>| {
        let total = lines.len();
        let items = lines
            .into_iter()
            .map(|line| Arc::new(SourceItem::from(line)) as Arc<dyn ClapItem>)
            .collect::<Vec<_>>();
        ProviderSource::Small { total, items }
    };

    // Known providers.
    match context.provider_id.as_str() {
        "blines" => {
            let total =
                crate::utils::count_lines(std::fs::File::open(&context.start_buffer_path)?)?;
            let path = context.start_buffer_path.to_path_buf();
            return Ok(ProviderSource::CachedFile { total, path });
        }
        "tags" => {
            let items = crate::tools::ctags::buffer_tag_items(&context.start_buffer_path, false)?;
            let total = items.len();
            return Ok(ProviderSource::Small { total, items });
        }
        "proj_tags" => {
            let ctags_cmd = build_recursive_ctags_cmd(context.cwd.to_path_buf());
            let provider_source = if context.no_cache {
                let lines = ctags_cmd.execute_and_write_cache().await?;
                to_small_provider_source(lines)
            } else {
                match ctags_cmd.ctags_cache() {
                    Some((total, path)) => ProviderSource::CachedFile { total, path },
                    None => {
                        let lines = ctags_cmd.execute_and_write_cache().await?;
                        to_small_provider_source(lines)
                    }
                }
            };
            return Ok(provider_source);
        }
        "grep2" => {
            let rg_cmd = RgTokioCommand::new(context.cwd.to_path_buf());
            let digest = if context.no_cache {
                rg_cmd.create_cache().await?
            } else {
                match rg_cmd.cache_digest() {
                    Some(digest) => digest,
                    None => rg_cmd.create_cache().await?,
                }
            };
            let (total, path) = (digest.total, digest.cached_path);
            vim.exec("set_var", json!(["g:__clap_forerunner_tempfile", &path]))?;
            return Ok(ProviderSource::CachedFile { total, path });
        }
        _ => {}
    }

    let source_cmd: Vec<String> = vim.call("provider_source_cmd", json!([])).await?;
    if let Some(source_cmd) = source_cmd.into_iter().next() {
        let mut tokio_cmd = crate::process::tokio::shell_command(&source_cmd);

        let shell_cmd = ShellCommand::new(source_cmd, context.cwd.to_path_buf());

        let cache_file = shell_cmd.cache_file_path()?;

        let provider_source = if context.no_cache {
            // Can not use subprocess::Exec::shell here.
            //
            // Must use TokioCommand otherwise the timeout may not work.

            crate::process::tokio::write_stdout_to_file(&mut tokio_cmd, &cache_file).await?;
            let total = crate::utils::count_lines(std::fs::File::open(&cache_file)?)?;
            ProviderSource::CachedFile {
                total,
                path: cache_file,
            }
        } else {
            match shell_cmd.cache_digest() {
                Some(digest) => ProviderSource::CachedFile {
                    total: digest.total,
                    path: digest.cached_path,
                },
                None => {
                    crate::process::tokio::write_stdout_to_file(&mut tokio_cmd, &cache_file)
                        .await?;
                    let total = crate::utils::count_lines(std::fs::File::open(&cache_file)?)?;

                    ProviderSource::CachedFile {
                        total,
                        path: cache_file,
                    }
                }
            }
        };

        return Ok(provider_source);
    }

    Ok(ProviderSource::Unknown)
}
