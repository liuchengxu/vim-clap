use maple_cli::{
    cmd::{Cmd, Maple},
    Result, Source, StructOpt,
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
        Cmd::RPC => {
            maple_cli::cmd::rpc::run_forever(std::io::BufReader::new(std::io::stdin()));
        }
        Cmd::Filter { query, input, algo } => {
            let source = input
                .map(Into::into)
                .unwrap_or(Source::<std::iter::Empty<_>>::Stdin);
            maple_cli::cmd::filter::dynamic::dyn_fuzzy_filter_and_rank(
                &query,
                source,
                algo,
                maple.number,
                maple.enable_icon,
                maple.winwidth,
            )?;
        }
        Cmd::Blines { query, input } => {
            maple_cli::cmd::filter::blines(&query, &input, maple.number, maple.winwidth)?;
        }
        Cmd::Exec {
            cmd,
            output,
            cmd_dir,
            output_threshold,
        } => {
            maple_cli::cmd::exec::run(
                cmd,
                output,
                output_threshold,
                cmd_dir,
                maple.number,
                maple.enable_icon,
            )?;
        }
        Cmd::Grep {
            grep_cmd,
            grep_query,
            glob,
            cmd_dir,
        } => {
            let g = match &glob {
                Some(s) => Some(s.as_str()),
                None => None,
            };

            maple_cli::cmd::grep::run(
                grep_cmd,
                &grep_query,
                g,
                cmd_dir,
                maple.number,
                maple.enable_icon,
            )?;
        }
        Cmd::Helptags { meta_info } => maple_cli::cmd::helptags::run(meta_info)?,
        Cmd::RipgrepForerunner { cmd_dir } => {
            maple_cli::cmd::grep::run_forerunner(cmd_dir, maple.number, maple.enable_icon)?
        }
    }
    Ok(())
}

pub fn main() -> Result<()> {
    run(Maple::from_args())
}
