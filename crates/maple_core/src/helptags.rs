use std::borrow::Cow;
use std::collections::HashMap;
use utility::read_lines;

#[inline]
fn strip_trailing_slash(x: &str) -> Cow<str> {
    if x.ends_with('/') {
        let mut x: String = x.into();
        x.pop();
        x.into()
    } else {
        x.into()
    }
}

pub fn generate_tag_lines(
    doc_tags: impl Iterator<Item = String>,
    runtimepath: &str,
) -> Vec<String> {
    let mut lines = Vec::new();
    for doc_tag in doc_tags {
        let tags_files = runtimepath
            .split(',')
            .map(|x| format!("{}{}", strip_trailing_slash(x), doc_tag));
        let mut seen = HashMap::new();
        let mut v: Vec<String> = Vec::new();
        for tags_file in tags_files {
            if let Ok(lines) = read_lines(tags_file) {
                lines.for_each(|line| {
                    if let Ok(helptag) = line {
                        v = helptag.split('\t').map(Into::into).collect();
                        if !seen.contains_key(&v[0]) {
                            seen.insert(v[0].clone(), format!("{:<60}\t{}", v[0], v[1]));
                        }
                    }
                });
            }
        }
        let mut tag_lines = seen.into_values().collect::<Vec<String>>();
        tag_lines.sort();

        lines.extend(tag_lines);
    }

    lines
}
