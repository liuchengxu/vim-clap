use maple_cli::{
    cmd::{Cmd, Maple},
    Result, StructOpt,
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
        Cmd::Tags(tags) => tags.run(maple.no_cache)?,
        Cmd::RPC => {
            maple_cli::cmd::rpc::run_forever(std::io::BufReader::new(std::io::stdin()));
        }
        Cmd::Blines(blines) => {
            blines.run(maple.number, maple.winwidth)?;
        }
        Cmd::RipGrepForerunner(rip_grep_forerunner) => {
            rip_grep_forerunner.run(maple.number, maple.icon_painter, maple.no_cache)?
        }
        Cmd::Cache(cache) => cache.run()?,
        Cmd::Filter(filter) => {
            filter.run(maple.number, maple.winwidth, maple.icon_painter)?;
        }
        Cmd::Exec(exec) => {
            exec.run(maple.number, maple.icon_painter, maple.no_cache)?;
        }
        Cmd::Grep(grep) => {
            grep.run(
                maple.number,
                maple.winwidth,
                maple.icon_painter,
                maple.no_cache,
            )?;
        }
    }
    Ok(())
}

pub fn main() -> Result<()> {
    run(Maple::from_args())
}
