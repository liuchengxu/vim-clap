use std::path::{PathBuf, MAIN_SEPARATOR};

use anyhow::{anyhow, Result};
use rayon::prelude::*;
use subprocess::Exec;

use dumb_analyzer::resolve_reference_kind;

use super::Symbol;
use crate::utils::ExactOrInverseTerms;
use crate::{find_usages::AddressableUsage, tools::gtags::GTAGS_DIR};

#[derive(Clone, Debug)]
pub struct GtagsSearcher {
    pub project_root: PathBuf,
    pub db_path: PathBuf,
}

impl GtagsSearcher {
    pub fn new(project_root: PathBuf) -> Self {
        // Directory for GTAGS, GRTAGS, GPATH, e.g.,
        //
        // `~/.local/share/vimclap/gtags/project_root`
        let mut db_path = GTAGS_DIR.to_path_buf();
        db_path.push(
            project_root
                .display()
                .to_string()
                .replace(MAIN_SEPARATOR, "_"),
        );

        Self {
            project_root,
            db_path,
        }
    }

    /// Create or update the tags db.
    pub fn create_or_update_tags(&self) -> Result<()> {
        if self.db_path.exists() {
            self.update_tags()
        } else {
            self.create_tags()
        }
    }

    /// Force recreating the gtags db.
    pub fn force_recreate(&self) -> Result<()> {
        std::fs::remove_dir_all(&self.db_path)?;
        self.create_tags()
    }

    /// Constructs a `gtags` command with proper env variables.
    fn gtags(&self) -> Exec {
        Exec::cmd("gtags")
            .env("GTAGSROOT", &self.project_root)
            .env("GTAGSDBPATH", &self.db_path)
    }

    /// Constructs a `global` command with proper env variables.
    fn global(&self) -> Exec {
        Exec::cmd("global")
            .env("GTAGSROOT", &self.project_root)
            .env("GTAGSDBPATH", &self.db_path)
    }

    pub fn create_tags(&self) -> Result<()> {
        std::fs::create_dir_all(&self.db_path)?;
        let exit_status = self
            .gtags()
            .env("GTAGSLABEL", "native-pygments")
            .cwd(&self.project_root)
            .arg(&self.db_path)
            .join()?;
        if exit_status.success() {
            Ok(())
        } else {
            Err(anyhow!(
                "Creating gtags failed, exit_status: {exit_status:?}"
            ))
        }
    }

    /// Update tags files increamentally.
    pub fn update_tags(&self) -> Result<()> {
        // GTAGSLABEL=native-pygments should be enabled.
        let exit_status = self
            .global()
            .env("GTAGSLABEL", "native-pygments")
            .cwd(&self.project_root)
            .arg("--update")
            .join()?;

        if exit_status.success() {
            Ok(())
        } else {
            Err(anyhow!(
                "Updating gtags failed, exit_status: {exit_status:?}"
            ))
        }
    }

    /// Search definition tags exactly matching `keyword`.
    pub fn search_definitions(&self, keyword: &str) -> Result<impl Iterator<Item = Symbol>> {
        let cmd = self
            .global()
            .cwd(&self.project_root)
            .arg(keyword)
            .arg("--result")
            .arg("ctags-x");

        execute(cmd)
    }

    /// `search_references` and reorder the results based on the language pattern.
    pub fn search_usages(
        &self,
        keyword: &str,
        filtering_terms: &ExactOrInverseTerms,
        file_ext: &str,
    ) -> Result<Vec<AddressableUsage>> {
        let mut gtags_usages = self
            .search_references(keyword)?
            .par_bridge()
            .filter_map(|symbol| {
                let (kind, kind_weight) = resolve_reference_kind(&symbol.pattern, file_ext);
                let (line, indices) = symbol.grep_format_gtags(kind, keyword, false);
                filtering_terms
                    .check_jump_line((line, indices.unwrap_or_default()))
                    .map(|(line, indices)| GtagsUsage {
                        line,
                        indices,
                        kind_weight,
                        path: symbol.path, // TODO: perhaps path_weight? Lower the weight of path containing `test`.
                        line_number: symbol.line_number,
                    })
            })
            .collect::<Vec<_>>();

        gtags_usages.par_sort_unstable_by(|a, b| a.cmp(b));

        Ok(gtags_usages
            .into_par_iter()
            .map(GtagsUsage::into_addressable_usage)
            .collect::<Vec<_>>())
    }

    /// Search reference tags exactly matching `keyword`.
    ///
    /// Reference means the reference to a symbol which has definitions.
    pub fn search_references(&self, keyword: &str) -> Result<impl Iterator<Item = Symbol>> {
        let cmd = self
            .global()
            .cwd(&self.project_root)
            .arg(keyword)
            .arg("--reference")
            .arg("--result")
            .arg("ctags-x");

        execute(cmd)
    }

    // TODO prefix matching
    // GTAGSROOT=$(pwd) GTAGSDBPATH=/home/xlc/.local/share/vimclap/gtags/test/ global -g 'ru(.*)' --result=ctags-x
}

// Returns a stream of tag parsed from the gtags output.
fn execute(cmd: Exec) -> Result<impl Iterator<Item = Symbol>> {
    Ok(crate::utils::lines(cmd)?
        .flatten()
        .filter_map(|s| Symbol::from_gtags(&s)))
}

/// Used for sorting the usages from gtags properly.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GtagsUsage {
    line: String,
    indices: Vec<usize>,
    line_number: usize,
    path: String,
    kind_weight: usize,
}

impl GtagsUsage {
    fn into_addressable_usage(self) -> AddressableUsage {
        AddressableUsage {
            line: self.line,
            indices: self.indices,
            path: self.path,
            line_number: self.line_number,
        }
    }
}

impl PartialOrd for GtagsUsage {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some((self.kind_weight, &self.path, self.line_number).cmp(&(
            other.kind_weight,
            &other.path,
            other.line_number,
        )))
    }
}

impl Ord for GtagsUsage {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}
