use anyhow::Result;
use clap::Parser;
use maple_core::helptags::generate_tag_lines;
use maple_core::paths::AbsPathBuf;
use std::io::Write;
use utils::io::read_lines;

/// Parse and display Vim helptags.
#[derive(Parser, Debug, Clone)]
pub struct Helptags {
    /// Tempfile containing the info of vim helptags.
    #[clap(index = 1)]
    meta_info: AbsPathBuf,
}

impl Helptags {
    pub fn run(self) -> Result<()> {
        let mut lines = read_lines(self.meta_info.as_ref())?;
        // line 1:/doc/tags,/doc/tags-cn
        // line 2:&runtimepath
        if let Some(Ok(doc_tags)) = lines.next() {
            if let Some(Ok(runtimepath)) = lines.next() {
                let lines =
                    generate_tag_lines(doc_tags.split(',').map(|s| s.to_string()), &runtimepath);
                let stdout = std::io::stdout();
                let mut lock = stdout.lock();
                for line in lines {
                    writeln!(lock, "{line}")?;
                }
            }
        }
        Ok(())
    }
}
