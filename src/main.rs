use maple_cli::{
    cmd::{Cmd, Maple},
    subprocess, ContentFiltering, IconPainter, Result, Source, StructOpt,
};

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

fn version() {
    println!(
        "{}",
        format!(
            "version {}{}, built for {} by {}.",
            built_info::PKG_VERSION,
            built_info::GIT_VERSION.map_or_else(|| "".to_owned(), |v| format!(" (git {})", v)),
            built_info::TARGET,
            built_info::RUSTC_VERSION
        )
    );
}

fn run(maple: Maple) -> Result<()> {
    match maple.command {
        Cmd::Version => {
            version();
        }
        Cmd::Helptags(helptags) => helptags.run()?,
        Cmd::RPC => {
            maple_cli::cmd::rpc::run_forever(std::io::BufReader::new(std::io::stdin()));
        }
        Cmd::Blines(blines) => {
            blines.run(maple.number, maple.winwidth)?;
        }
        Cmd::RipgrepForerunner { cmd_dir } => maple_cli::cmd::grep::run_forerunner(
            cmd_dir,
            maple.number,
            maple.enable_icon,
            maple.no_cache,
        )?,
        Cmd::Cache(cache) => cache.run()?,
        Cmd::Filter {
            query,
            input,
            algo,
            cmd,
            cmd_dir,
            sync,
        } => {
            let source = if let Some(cmd_str) = cmd {
                if let Some(dir) = cmd_dir {
                    subprocess::Exec::shell(cmd_str).cwd(dir).into()
                } else {
                    subprocess::Exec::shell(cmd_str).into()
                }
            } else {
                input
                    .map(Into::into)
                    .unwrap_or(Source::<std::iter::Empty<_>>::Stdin)
            };
            if sync {
                maple_cli::cmd::filter::run(
                    &query,
                    source,
                    algo,
                    maple.number,
                    maple.enable_icon,
                    maple.winwidth,
                )?;
            } else {
                maple_cli::cmd::filter::dyn_run(
                    &query,
                    source,
                    algo,
                    maple.number,
                    maple.winwidth,
                    if maple.enable_icon {
                        Some(IconPainter::File)
                    } else {
                        None
                    },
                    ContentFiltering::Full,
                )?;
            }
        }
        Cmd::Exec(exec) => {
            exec.run(maple.number, maple.enable_icon, maple.no_cache)?;
        }
        Cmd::Grep {
            grep_cmd,
            grep_query,
            glob,
            cmd_dir,
            sync,
            input,
        } => {
            let g = match &glob {
                Some(s) => Some(s.as_str()),
                None => None,
            };

            if sync {
                maple_cli::cmd::grep::run(
                    grep_cmd,
                    &grep_query,
                    g,
                    cmd_dir,
                    maple.number,
                    maple.enable_icon,
                )?;
            } else {
                maple_cli::cmd::grep::dyn_grep(
                    &grep_query,
                    cmd_dir,
                    input,
                    maple.number,
                    maple.enable_icon,
                    maple.no_cache,
                )?;
            }
        }
    }
    Ok(())
}

pub fn main() -> Result<()> {
    run(Maple::from_args())
}
