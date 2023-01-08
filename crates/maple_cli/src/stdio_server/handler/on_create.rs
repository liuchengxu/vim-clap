#![allow(unused)]
use crate::command::ctags::recursive_tags::build_recursive_ctags_cmd;
use crate::command::grep::{rg_command, rg_shell_command, RgTokioCommand, RG_EXEC_CMD};
use crate::process::{CacheableCommand, ShellCommand};
use crate::stdio_server::job;
use crate::stdio_server::provider::{Context, ProviderSource};
use anyhow::Result;
use filter::SourceItem;
use matcher::ClapItem;
use printer::DisplayLines;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

async fn execute_and_write_cache(
    cmd: &str,
    cache_file: std::path::PathBuf,
) -> Result<ProviderSource> {
    // Can not use subprocess::Exec::shell here.
    //
    // Must use TokioCommand otherwise the timeout may not work.

    let mut tokio_cmd = crate::process::tokio::shell_command(cmd);
    crate::process::tokio::write_stdout_to_file(&mut tokio_cmd, &cache_file).await?;
    let total = crate::utils::count_lines(std::fs::File::open(&cache_file)?)?;
    Ok(ProviderSource::CachedFile {
        total,
        path: cache_file,
        refreshed: true,
    })
}

/// Performs the initialization like collecting the source and total number of source items.
async fn initialize_provider_source(context: &Context) -> Result<ProviderSource> {
    let to_small_provider_source = |lines: Vec<String>| {
        let total = lines.len();
        let items = lines
            .into_iter()
            .map(|line| Arc::new(SourceItem::from(line)) as Arc<dyn ClapItem>)
            .collect::<Vec<_>>();
        ProviderSource::Small { total, items }
    };

    // Known providers.
    match context.provider_id() {
        "blines" => {
            let total =
                crate::utils::count_lines(std::fs::File::open(&context.env.start_buffer_path)?)?;
            let path = context.env.start_buffer_path.clone();
            return Ok(ProviderSource::File { total, path });
        }
        "tags" => {
            let items =
                crate::tools::ctags::buffer_tag_items(&context.env.start_buffer_path, false)?;
            let total = items.len();
            return Ok(ProviderSource::Small { total, items });
        }
        "proj_tags" => {
            let ctags_cmd = build_recursive_ctags_cmd(context.cwd.to_path_buf());
            let provider_source = if context.env.no_cache {
                let lines = ctags_cmd.execute_and_write_cache().await?;
                to_small_provider_source(lines)
            } else {
                match ctags_cmd.ctags_cache() {
                    Some((total, path)) => ProviderSource::CachedFile {
                        total,
                        path,
                        refreshed: false,
                    },
                    None => {
                        let lines = ctags_cmd.execute_and_write_cache().await?;
                        to_small_provider_source(lines)
                    }
                }
            };
            return Ok(provider_source);
        }
        "help_tags" => {
            let helplang: String = context.vim.eval("&helplang").await?;
            let runtimepath: String = context.vim.eval("&runtimepath").await?;
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

    let source_cmd: Vec<Value> = context.vim.bare_call("provider_source").await?;
    if let Some(value) = source_cmd.into_iter().next() {
        match value {
            // Source is a String: g:__t_string, g:__t_func_string
            Value::String(command) => {
                // Always try recreating the source.
                if context.provider_id() == "files" {
                    let mut tokio_cmd = crate::process::tokio::TokioCommand::new(command);
                    tokio_cmd.current_dir(&context.cwd);
                    let lines = tokio_cmd.lines().await?;
                    return Ok(to_small_provider_source(lines));
                }

                let shell_cmd = ShellCommand::new(command, context.cwd.to_path_buf());
                let cache_file = shell_cmd.cache_file_path()?;

                const DIRECT_CREATE_NEW_SOURCE: &[&str] = &["files"];

                let direct_create_new_source =
                    DIRECT_CREATE_NEW_SOURCE.contains(&context.provider_id());

                let provider_source = if direct_create_new_source || context.env.no_cache {
                    execute_and_write_cache(&shell_cmd.command, cache_file).await?
                } else {
                    match shell_cmd.cache_digest() {
                        Some(digest) => ProviderSource::CachedFile {
                            total: digest.total,
                            path: digest.cached_path,
                            refreshed: false,
                        },
                        None => execute_and_write_cache(&shell_cmd.command, cache_file).await?,
                    }
                };

                if let ProviderSource::CachedFile { path, .. } = &provider_source {
                    context.vim.set_var("g:__clap_forerunner_tempfile", path)?;
                }

                return Ok(provider_source);
            }
            // Source is a List: g:__t_list, g:__t_func_list
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

    Ok(ProviderSource::Unactionable)
}

pub async fn initialize_provider(ctx: &Context) -> Result<()> {
    const TIMEOUT: Duration = Duration::from_millis(300);

    // Skip the initialization.
    match ctx.provider_id() {
        "grep" | "live_grep" => return Ok(()),
        _ => {}
    }

    match tokio::time::timeout(TIMEOUT, initialize_provider_source(ctx)).await {
        Ok(provider_source_result) => match provider_source_result {
            Ok(provider_source) => {
                if let Some(total) = provider_source.total() {
                    ctx.vim.set_var("g:clap.display.initial_size", total)?;
                }

                if let Some(items) = provider_source.initial_items(100) {
                    let DisplayLines {
                        lines,
                        icon_added,
                        truncated_map,
                        ..
                    } = printer::decorate_lines(items, ctx.env.display_winwidth, ctx.env.icon);

                    let using_cache = provider_source.using_cache();
                    ctx.vim.exec(
                        "clap#state#init_display",
                        json!([lines, truncated_map, icon_added, using_cache]),
                    )?;
                }

                ctx.set_provider_source(provider_source);
            }
            Err(e) => tracing::error!(?e, "Error occurred on creating session"),
        },
        Err(_) => {
            // The initialization was not super fast.
            tracing::debug!(timeout = ?TIMEOUT, "Did not receive value in time");

            let source_cmd: Vec<String> = ctx.vim.bare_call("provider_source_cmd").await?;
            let maybe_source_cmd = source_cmd.into_iter().next();
            if let Some(source_cmd) = maybe_source_cmd {
                ctx.set_provider_source(ProviderSource::Command(source_cmd));
            }

            /* no longer necessary for grep provider.
            // Try creating cache for some potential heavy providers.
            match context.provider_id() {
                "grep" | "live_grep" => {
                    context.set_provider_source(ProviderSource::Command(RG_EXEC_CMD.to_string()));

                    let context = context.clone();
                    let rg_cmd = RgTokioCommand::new(context.cwd.to_path_buf());
                    let job_id = utility::calculate_hash(&rg_cmd);
                    job::try_start(
                        async move {
                            if let Ok(digest) = rg_cmd.create_cache().await {
                                let new = ProviderSource::CachedFile {
                                    total: digest.total,
                                    path: digest.cached_path,
                                    refreshed: true,
                                };
                                if !context.terminated.load(Ordering::SeqCst) {
                                    context.set_provider_source(new);
                                }
                            }
                        },
                        job_id,
                    );
                }
                _ => {}
            }
            */
        }
    }

    Ok(())
}
