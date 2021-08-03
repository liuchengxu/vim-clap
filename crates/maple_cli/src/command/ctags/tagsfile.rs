use std::ops::Deref;
use std::path::{Path, PathBuf};

use anyhow::Result;
use once_cell::sync::Lazy;
use structopt::StructOpt;

use super::SharedParams;

use crate::app::Params;

#[derive(StructOpt, Debug, Clone)]
struct TagsFileParams {
    /// Same with the `--kinds-all` option of ctags.
    #[structopt(long, default_value = "*")]
    kinds_all: String,

    /// Same with the `--fields` option of ctags.
    #[structopt(long, default_value = "*")]
    fields: String,

    /// Same with the `--extras` option of ctags.
    #[structopt(long, default_value = "*")]
    extras: String,
}

/// Manipulate the tags file.
#[derive(StructOpt, Debug, Clone)]
pub struct TagsFile {
    /// Params for creating tags file.
    #[structopt(flatten)]
    inner: TagsFileParams,

    /// Shared parameters arouns ctags.
    #[structopt(flatten)]
    shared: SharedParams,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct CreateTagsConfig<'a, P> {
    languages: Option<&'a String>,
    kinds_all: &'a str,
    fields: &'a str,
    extras: &'a str,
    exclude_opt: String,
    dir: P,
}

pub static TAGS_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let proj_dirs = directories::ProjectDirs::from("org", "vim", "Vim Clap")
        .expect("Couldn't create project directory for vim-clap");

    let mut tags_dir = proj_dirs.data_dir().to_path_buf();
    tags_dir.push("tags");
    std::fs::create_dir_all(&tags_dir).expect("Couldn't create data directory for vim-clap");

    tags_dir
});

impl<'a, P: AsRef<Path> + std::hash::Hash> CreateTagsConfig<'a, P> {
    pub fn new(
        languages: Option<&'a String>,
        kinds_all: &'a str,
        fields: &'a str,
        extras: &'a str,
        dir: P,
        exclude_opt: String,
    ) -> Self {
        Self {
            languages,
            kinds_all,
            fields,
            extras,
            dir,
            exclude_opt,
        }
    }

    fn build_command(&self) -> String {
        let mut tags_filepath = TAGS_DIR.deref().clone();
        tags_filepath.push(utility::calculate_hash(self).to_string());

        // TODO: detect the languages by dir if not explicitly specified?
        let languages_opt = self
            .languages
            .map(|v| format!("--languages={}", v))
            .unwrap_or_default();

        format!(
            "ctags {} --kinds-all='{}' --fields='{}' --extras='{}' {} -f '{}' -R",
            languages_opt,
            self.kinds_all,
            self.fields,
            self.extras,
            self.exclude_opt,
            tags_filepath.display()
        )
    }

    fn create_tags(&self) -> Result<()> {
        let command = self.build_command();
        let exit_status = filter::subprocess::Exec::shell(&command)
            .cwd(self.dir.as_ref())
            .join()?;

        if !exit_status.success() {
            return Err(anyhow::anyhow!("Error occured when creating tags file"));
        }

        Ok(())
    }
}

impl TagsFile {
    pub fn run(&self, _params: Params) -> Result<()> {
        let create_tags_config = CreateTagsConfig::new(
            self.shared.languages.as_ref(),
            &self.inner.kinds_all,
            &self.inner.fields,
            &self.inner.extras,
            &self.shared.dir,
            self.shared.exclude_opt(),
        );

        create_tags_config.create_tags()?;

        Ok(())
    }
}
