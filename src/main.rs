use maple_cli::{Cmd, Context, Maple, Result, StructOpt};

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

fn version() {
    println!(
        "version {}{}, built for {} by {}.",
        built_info::PKG_VERSION,
        built_info::GIT_VERSION.map_or_else(|| "".to_owned(), |v| format!(" (git {})", v)),
        built_info::TARGET,
        built_info::RUSTC_VERSION
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    let maple = Maple::from_args();

    match maple.command {
        Cmd::Version => version(),
        Cmd::Upgrade(upgrade) => {
            let local_git_tag = built_info::GIT_VERSION.context("Failed to get GIT_VERSION")?;
            if let Err(e) = upgrade.run(local_git_tag).await {
                eprintln!("failed to upgrade: {:?}", e);
                std::process::exit(1);
            }
        }
        _ => {
            if let Err(e) = maple.run().await {
                eprintln!("error: {:?}", e);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
