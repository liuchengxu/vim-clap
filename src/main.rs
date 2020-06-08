use maple_cli::{Cmd, Context, Maple, Result, StructOpt};

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
        Cmd::Upgrade(upgrade) => {
            let local_git_tag = built_info::GIT_VERSION.context("Failed to get git tag info")?;
            upgrade.run(local_git_tag)?;
        }
        _ => maple.run()?,
    }
    Ok(())
}

pub fn main() -> Result<()> {
    run(Maple::from_args())
}
