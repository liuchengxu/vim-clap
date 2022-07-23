use clap::{AppSettings, Parser};

use maple_cli::{Context, Params, Result, RunCmd};

mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Parser, Debug)]
pub enum Cmd {
    /// Display the current version
    #[clap(name = "version")]
    Version,
    /// This command is only invoked when user uses the prebuilt binary,
    /// more specifically, executable in vim-clap/bin/maple.
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
    Run(RunCmd),
}

#[derive(Parser, Debug)]
#[clap(name = "maple")]
#[clap(global_setting(AppSettings::NoAutoVersion))]
pub struct Maple {
    #[clap(flatten)]
    pub params: Params,

    #[clap(subcommand)]
    pub command: Cmd,
}

#[tokio::main]
async fn main() -> Result<()> {
    let maple = Maple::parse();

    match maple.command {
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
            let local_git_tag = built_info::GIT_VERSION.context("Failed to get GIT_VERSION")?;
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
