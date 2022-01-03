use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::Result;
use filter::subprocess::Exec;

use crate::tools::ctags::TagsConfig;

#[derive(Clone, Debug)]
pub enum Filtering {
    StartWith,
    Contain,
    #[allow(unused)]
    Inherit,
}

/// `readtags` powered searcher.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct TagSearcher<'a, P> {
    config: TagsConfig<'a, P>,
    tags_path: PathBuf,
}

impl<'a, P: AsRef<Path> + Hash> TagSearcher<'a, P> {
    pub fn new(config: TagsConfig<'a, P>) -> Self {
        let tags_path = config.tags_path();
        Self { config, tags_path }
    }

    /// Returns `true` if the tags file already exists.
    pub fn tags_exists(&self) -> bool {
        self.tags_path.exists()
    }

    /// Generate the `tags` file.
    pub fn generate_tags(&self) -> Result<()> {
        self.config.generate_tags()
    }

    fn build_exec(&self, query: &str, filtering_type: Filtering) -> Exec {
        // https://docs.ctags.io/en/latest/man/readtags.1.html#examples
        let cmd = Exec::cmd("readtags")
            .arg("--tag-file")
            .arg(&self.tags_path)
            .arg("-E")
            .arg("-ne");

        let cmd = if query.chars().all(char::is_lowercase) {
            cmd.arg("--icase-match")
        } else {
            cmd
        };

        match filtering_type {
            Filtering::StartWith => cmd.arg("--prefix-match").arg("-").arg(query),
            Filtering::Contain => cmd
                .arg("-Q")
                .arg(format!("(substr? (downcase $name) \"{}\")", query))
                .arg("-l"),
            Filtering::Inherit => {
                todo!("Inherit")
            }
        }
    }

    pub fn search(
        &self,
        query: &str,
        filtering: Filtering,
        force_generate: bool,
    ) -> Result<impl Iterator<Item = TagLine>> {
        use std::io::BufRead;

        if force_generate || !self.tags_exists() {
            self.generate_tags()?;
        }

        let stdout = self.build_exec(query, filtering).stream_stdout()?;

        Ok(std::io::BufReader::new(stdout)
            .lines()
            .flatten()
            .filter_map(|line| line.parse::<TagLine>().ok()))
    }
}

#[derive(Default, Debug)]
pub struct TagLine {
    pub name: String,
    pub path: String,
    pub pattern: String,
    pub language: String,
    pub kind: Option<String>,
    pub scope: Option<String>,
    pub line: u64,
}

impl FromStr for TagLine {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut items = s.split('\t');

        let mut l = TagLine {
            name: items.next().ok_or(())?.into(),
            path: items.next().ok_or(())?.into(),
            ..Default::default()
        };

        // https://docs.ctags.io/en/latest/man/ctags-client-tools.7.html#parse-readtags-output
        if let Some(p) = items
            .clone()
            .peekable()
            .peek()
            .and_then(|p| p.strip_suffix(";\""))
        {
            let search_pattern_used = (p.starts_with('/') && p.ends_with('/'))
                || (p.len() > 1 && p.starts_with('$') && p.ends_with('$'));
            if search_pattern_used {
                let pat = items.next().ok_or(())?;
                let pat_len = pat.len();
                // forward search: `/^foo$/`
                // backward search: `?^foo$?`
                if p.starts_with("/^") || p.starts_with("?^") {
                    if p.ends_with("$/") || p.ends_with("$?") {
                        l.pattern = String::from(&pat[2..pat_len - 4]);
                    } else {
                        l.pattern = String::from(&pat[2..pat_len - 2]);
                    }
                } else {
                    l.pattern = String::from(&pat[2..pat_len]);
                }
            } else {
                return Err(());
            }
        } else {
            return Err(());
        }

        for item in items {
            if let Some((k, v)) = item.split_once(':') {
                if v.is_empty() {
                    continue;
                }
                match k {
                    "kind" => l.kind = Some(v.into()),
                    "language" => l.language = v.into(),
                    "scope" => l.scope = Some(v.into()),
                    "line" => l.line = v.parse().expect("line is an integer"),
                    "roles" | "access" | "signature" => {}
                    unknown => {
                        tracing::debug!(line = %s, "Unknown field: {}", unknown);
                    }
                }
            }
        }

        Ok(l)
    }
}

impl TagLine {
    pub fn grep_format(&self, query: &str, ignorecase: bool) -> (String, Option<Vec<usize>>) {
        let mut formatted = format!(
            "[{}]{}:{}:1:",
            self.kind.as_ref().map(|s| s.as_ref()).unwrap_or("tags"),
            self.path,
            self.line
        );

        let found = if ignorecase {
            self.pattern.to_lowercase().find(&query.to_lowercase())
        } else {
            self.pattern.find(query)
        };

        let indices = if let Some(idx) = found {
            let start = formatted.len() + idx;
            let end = start + query.len();
            Some((start..end).into_iter().collect())
        } else {
            None
        };

        formatted.push_str(&self.pattern);

        (formatted, indices)
    }
}
