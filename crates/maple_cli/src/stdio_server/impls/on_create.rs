use std::sync::Arc;

use anyhow::Result;
use serde_json::{json, Value};

use filter::SourceItem;
use matcher::ClapItem;

use crate::command::ctags::recursive_tags::build_recursive_ctags_cmd;
use crate::command::grep::{rg_command, rg_shell_command, RgTokioCommand};
use crate::process::{CacheableCommand, ShellCommand};
use crate::stdio_server::provider::ProviderSource;
use crate::stdio_server::session::SessionContext;
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
        "grep" => {
            let mut std_cmd = rg_command(&context.cwd);
            let exec_info = CacheableCommand::new(
                &mut std_cmd,
                rg_shell_command(&context.cwd),
                None,
                context.icon,
                Some(100_000),
            )
            .execute()?;
            vim.exec(
                "clap#state#process_grep_forerunner_result",
                json!({ "exec_info": exec_info }),
            )?;
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
            vim.set_var("g:__clap_forerunner_tempfile", &path)?;
            return Ok(ProviderSource::CachedFile { total, path });
        }
        "help_tags" => {
            let helplang: String = vim.eval("&helplang").await?;
            let runtimepath: String = vim.eval("&runtimepath").await?;
            let doc_tags = std::iter::once("/doc/tags".to_string()).chain(
                helplang
                    .split(',')
                    .filter(|&lang| lang != "en")
                    .map(|lang| format!("/doc/tags-{lang}")),
            );
            let lines = crate::command::helptags::generate_tag_lines(doc_tags, &runtimepath);
            return Ok(to_small_provider_source(lines));
        }
        _ => {}
    }

    let source_cmd: Vec<Value> = vim.call("provider_source", json!([])).await?;
    if let Some(value) = source_cmd.into_iter().next() {
        match value {
            Value::String(command) => {
                async fn create_new_source(
                    cmd: &str,
                    cache_file: std::path::PathBuf,
                ) -> Result<ProviderSource> {
                    // Can not use subprocess::Exec::shell here.
                    //
                    // Must use TokioCommand otherwise the timeout may not work.

                    let mut tokio_cmd = crate::process::tokio::shell_command(cmd);
                    crate::process::tokio::write_stdout_to_file(&mut tokio_cmd, &cache_file)
                        .await?;
                    let total = crate::utils::count_lines(std::fs::File::open(&cache_file)?)?;
                    Ok(ProviderSource::CachedFile {
                        total,
                        path: cache_file,
                    })
                }

                // Try always recreating the source.
                if context.provider_id.as_str() == "files" {
                    let mut tokio_cmd = crate::process::tokio::TokioCommand::new(command);
                    tokio_cmd.current_dir(&context.cwd);
                    let lines = tokio_cmd.lines().await?;
                    return Ok(to_small_provider_source(lines));
                }

                let shell_cmd = ShellCommand::new(command, context.cwd.to_path_buf());
                let cache_file = shell_cmd.cache_file_path()?;

                const DIRECT_CREATE_NEW_SOURCE: &[&str] = &["files"];

                let provider_source =
                    if DIRECT_CREATE_NEW_SOURCE.contains(&context.provider_id.as_str()) {
                        create_new_source(&shell_cmd.command, cache_file).await?
                    } else if context.no_cache {
                        create_new_source(&shell_cmd.command, cache_file).await?
                    } else {
                        match shell_cmd.cache_digest() {
                            Some(digest) => ProviderSource::CachedFile {
                                total: digest.total,
                                path: digest.cached_path,
                            },
                            None => create_new_source(&shell_cmd.command, cache_file).await?,
                        }
                    };

                if let ProviderSource::CachedFile { total: _, path } = &provider_source {
                    vim.set_var("g:__clap_forerunner_tempfile", &path)?;
                }

                return Ok(provider_source);
            }
            Value::Array(arr) => {
                let lines = arr
                    .into_iter()
                    .filter_map(|v| {
                        if let Value::String(s) = v {
                            Some(s)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                return Ok(to_small_provider_source(lines));
            }
            _ => {}
        }
    }

    Ok(ProviderSource::Unknown)
}
