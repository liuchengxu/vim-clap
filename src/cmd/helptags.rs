use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use anyhow::Result;

// The output is wrapped in a Result to allow matching on errors
// Returns an Iterator to the Reader of the lines of the file.
fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

#[inline]
fn strip_trailing_slash(x: &str) -> String {
    if x.ends_with('/') {
        let mut x: String = x.into();
        x.pop();
        x
    } else {
        x.into()
    }
}

pub fn run(meta_path: PathBuf) -> Result<()> {
    let mut lines = read_lines(meta_path)?;
    // line 1:/doc/tags,/doc/tags-cn
    // line 2:&runtimepath
    if let Some(Ok(doc_tags)) = lines.next() {
        if let Some(Ok(runtimepath)) = lines.next() {
            for dt in doc_tags.split(',') {
                let tags_files = runtimepath
                    .split(',')
                    .map(|x| format!("{}{}", strip_trailing_slash(x), dt));
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
                let mut tag_lines = seen.values().collect::<Vec<_>>();
                tag_lines.sort();
                for line in tag_lines {
                    println!("{}", line);
                }
            }
        }
    }
    Ok(())
}
