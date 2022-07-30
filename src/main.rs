use clap::{AppSettings, Parser};

use maple_cli::{Params, RunCmd};

mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Parser, Debug)]
pub enum Cmd {
    /// Display the current version.
    #[clap(name = "version")]
    Version,
    /// Upgrade the prebuilt binary to the latest GitHub release if any.
    ///
    /// Only available for the executable in `vim-clap/bin/maple`.
    #[clap(name = "upgrade")]
    Upgrade {
        /// Download if the local version mismatches the latest remote version.
        #[clap(long)]
        download: bool,
        /// Disable the downloading progress_bar
        #[clap(long)]
        no_progress_bar: bool,
    },
    /// Run the maple.
    #[clap(flatten)]
    Run(Box<RunCmd>),
}

#[derive(Parser, Debug)]
#[clap(name = "maple", global_setting(AppSettings::NoAutoVersion))]
pub struct Maple {
    #[clap(flatten)]
    pub params: Params,

    #[clap(subcommand)]
    pub cmd: Cmd,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let maple = Maple::parse();

    match maple.cmd {
        Cmd::Version => {
            println!(
                "version {}{}, built for {} by {}.",
                built_info::PKG_VERSION,
                built_info::GIT_VERSION.map_or_else(|| "".to_owned(), |v| format!(" (git {})", v)),
                built_info::TARGET,
                built_info::RUSTC_VERSION
            );
        }
        Cmd::Upgrade {
            download,
            no_progress_bar,
        } => {
            let local_git_tag = built_info::GIT_VERSION.expect("GIT_VERSION does not exist");
            if let Err(e) = upgrade::Upgrade::new(download, no_progress_bar)
                .run(local_git_tag)
                .await
            {
                eprintln!("failed to upgrade: {:?}", e);
                std::process::exit(1);
            }
        }
        Cmd::Run(run_cmd) => {
            if let Err(e) = run_cmd.run(maple.params).await {
                eprintln!("error: {:?}", e);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
