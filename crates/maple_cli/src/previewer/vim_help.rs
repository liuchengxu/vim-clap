use std::path::Path;

#[derive(Debug, Clone)]
pub struct HelpTagPreview<'a> {
    /// Help tag name.
    subject: &'a str,
    /// Filename of the help text.
    doc_filename: &'a str,
    /// Output of `:echo &runtimepath`
    runtimepath: &'a str,
}

fn find_tag_line(p: &Path, subject: &str) -> Option<usize> {
    if let Ok(doc_lines) = utility::read_lines(p) {
        for (idx, doc_line) in doc_lines.enumerate() {
            if let Ok(d_line) = doc_line {
                if d_line.trim().contains(subject) {
                    return Some(idx);
                }
            }
        }
    }
    None
}

impl<'a> HelpTagPreview<'a> {
    pub fn new(subject: &'a str, doc_filename: &'a str, runtimepath: &'a str) -> Self {
        Self {
            subject,
            doc_filename,
            runtimepath,
        }
    }

    pub fn get_help_lines(&self, size: usize) -> Option<(String, Vec<String>)> {
        let target_tag = format!("*{}*", self.subject);
        for r in self.runtimepath.split(',') {
            let p = Path::new(r).join("doc").join(&self.doc_filename);
            if p.exists() {
                if let Some(line_number) = find_tag_line(&p, &target_tag) {
                    if let Ok(lines_iter) = utility::read_lines_from(&p, line_number, size) {
                        return Some((format!("{}", p.display()), lines_iter.collect()));
                    }
                }
            }
        }

        None
    }
}
