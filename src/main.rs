use maple_cli::{
    cmd::{Cmd, Maple},
    Context, Result, StructOpt,
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
        Cmd::CheckRelease => {
            let local_git_tag = built_info::GIT_VERSION.context("Failed to get git tag info")?;
            let remote_release = maple_cli::cmd::check_release::latest_remote_release()?;
            let remote_tag = remote_release.tag_name;
            if remote_tag != local_git_tag {
                println!("New maple release {} is avaliable, please download it from https://github.com/liuchengxu/vim-clap/releases/tag/{}", remote_tag, remote_tag);
            } else {
                println!("No newer release, current maple version: {}", remote_tag);
            }
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
