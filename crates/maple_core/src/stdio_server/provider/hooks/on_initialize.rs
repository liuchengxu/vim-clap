use crate::process::ShellCommand;
use crate::stdio_server::provider::{Context, ProviderResult as Result, ProviderSource};
use crate::tools::ctags::ProjectCtagsCommand;
use filter::SourceItem;
use printer::{DisplayLines, Printer};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use types::ClapItem;
use utils::io::line_count;

async fn execute_and_write_cache(
    cmd: &str,
    cache_file: std::path::PathBuf,
) -> std::io::Result<ProviderSource> {
    // Can not use subprocess::Exec::shell here.
    //
    // Must use TokioCommand otherwise the timeout may not work.

    let mut tokio_cmd = crate::process::tokio::shell_command(cmd);
    crate::process::tokio::write_stdout_to_file(&mut tokio_cmd, &cache_file).await?;
    let total = line_count(&cache_file)?;
    Ok(ProviderSource::CachedFile {
        total,
        path: cache_file,
        refreshed: true,
    })
}

fn to_small_provider_source(lines: Vec<String>) -> ProviderSource {
    let total = lines.len();
    let items = lines
        .into_iter()
        .map(|line| Arc::new(SourceItem::from(line)) as Arc<dyn ClapItem>)
        .collect::<Vec<_>>();
    ProviderSource::Small { total, items }
}

#[allow(unused)]
async fn init_proj_tags(ctx: &Context) -> std::io::Result<ProviderSource> {
    let ctags_cmd = ProjectCtagsCommand::with_cwd(ctx.cwd.to_path_buf());
    let provider_source = if true {
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
    Ok(provider_source)
}

/// Performs the initialization like collecting the source and total number of source items.
async fn initialize_provider_source(ctx: &Context) -> Result<ProviderSource> {
    // Known providers.
    match ctx.provider_id() {
        "tags" => {
            let items = crate::tools::ctags::buffer_tag_items(&ctx.env.start_buffer_path, false)?;
            let total = items.len();
            return Ok(ProviderSource::Small { total, items });
        }
        "help_tags" => {
            let helplang: String = ctx.vim.eval("&helplang").await?;
            let runtimepath: String = ctx.vim.eval("&runtimepath").await?;
            let doc_tags = std::iter::once("/doc/tags".to_string()).chain(
                helplang
                    .split(',')
                    .filter(|&lang| lang != "en")
                    .map(|lang| format!("/doc/tags-{lang}")),
            );
            let lines = crate::helptags::generate_tag_lines(doc_tags, &runtimepath);
            return Ok(to_small_provider_source(lines));
        }
        _ => {}
    }

    let source_cmd: Vec<Value> = ctx.vim.bare_call("provider_source").await?;
    if let Some(value) = source_cmd.into_iter().next() {
        match value {
            // Source is a String: g:__t_string, g:__t_func_string
            Value::String(command) => {
                let shell_cmd = ShellCommand::new(command, ctx.cwd.to_path_buf());
                let cache_file = shell_cmd.cache_file_path()?;

                // Deprecated as now files provider has no `source` property, which is
                // handled by vim-clap internally.
                const DIRECT_CREATE_NEW_SOURCE: &[&str] = &["files"];

                let create_new_source_directly =
                    DIRECT_CREATE_NEW_SOURCE.contains(&ctx.provider_id());

                let provider_source = if create_new_source_directly || ctx.env.no_cache {
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
                    ctx.vim.set_var("g:__clap_forerunner_tempfile", path)?;
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

    Ok(ProviderSource::Uninitialized)
}

fn on_initialized_source(
    provider_source: ProviderSource,
    ctx: &Context,
    init_display: bool,
) -> Result<()> {
    if let Some(total) = provider_source.total() {
        ctx.vim.set_var("g:clap.display.initial_size", total)?;
    }

    if init_display {
        if let Some(items) = provider_source.try_skim(ctx.provider_id(), 100) {
            let printer = Printer::new(ctx.env.display_winwidth, ctx.env.icon);
            let DisplayLines {
                lines,
                icon_added,
                truncated_map,
                ..
            } = printer.to_display_lines(items);

            let using_cache = provider_source.using_cache();

            ctx.vim.exec(
                "clap#picker#init",
                json!([lines, truncated_map, icon_added, using_cache]),
            )?;
        }
        if ctx.initializing_prompt_echoed.load(Ordering::SeqCst) {
            ctx.vim.bare_exec("clap#helper#echo_clear")?;
        }
    }

    ctx.set_provider_source(provider_source);

    Ok(())
}

async fn initialize_list_source(ctx: Context, init_display: bool) -> Result<()> {
    let source_cmd: Vec<Value> = ctx.vim.bare_call("provider_source").await?;
    // Source must be initialized when it is a List: g:__t_list, g:__t_func_list
    if let Some(Value::Array(arr)) = source_cmd.into_iter().next() {
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
        on_initialized_source(to_small_provider_source(lines), &ctx, init_display)?;
    }

    Ok(())
}

pub async fn initialize_provider(ctx: &Context, init_display: bool) -> Result<()> {
    // Skip the initialization.
    match ctx.provider_id() {
        "grep" | "live_grep" => return Ok(()),
        "proj_tags" => {
            ctx.set_provider_source(ProviderSource::Initializing);
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let mut ctags_cmd = ProjectCtagsCommand::with_cwd(ctx.cwd.to_path_buf());
                match ctags_cmd.par_formatted_lines() {
                    Ok(lines) => {
                        let provider_source = to_small_provider_source(lines);
                        ctx.set_provider_source(provider_source);
                    }
                    Err(e) => {
                        ctx.set_provider_source(ProviderSource::InitializationFailed(
                            e.to_string(),
                        ));
                    }
                }

                Ok::<_, std::io::Error>(())
            });
            return Ok(());
        }
        _ => {}
    }

    if ctx.env.source_is_list {
        let ctx = ctx.clone();
        ctx.set_provider_source(ProviderSource::Initializing);
        // Initialize the list-style providers in another task so that the further
        // messages won't be blocked by the initialization in case it takes too long.
        tokio::spawn(initialize_list_source(ctx, init_display));
        return Ok(());
    }

    const TIMEOUT: Duration = Duration::from_millis(300);

    match tokio::time::timeout(TIMEOUT, initialize_provider_source(ctx)).await {
        Ok(Ok(provider_source)) => on_initialized_source(provider_source, ctx, init_display)?,
        Ok(Err(e)) => tracing::error!(?e, "Error occurred while initializing the provider source"),
        Err(_) => {
            // The initialization was not finished quickly.
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
                    let job_id = utils::calculate_hash(&rg_cmd);
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
