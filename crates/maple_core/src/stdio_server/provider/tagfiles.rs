use anyhow::Result;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug)]
struct TagItem {
    name: String,
    path: String,
    address: String,
    kind: String,
}

impl TagItem {
    pub fn format(&self, cwd: &PathBuf, winwidth: usize) -> String {
        static HOME: OnceCell<PathBuf> = OnceCell::new();

        let name = format!("{} ", self.name);
        let taken_width = name.len() + 1;
        let path_len = self.path.len() + 2;
        let mut adjustment = 0;

        let mut home_path = PathBuf::new();
        let path = Path::new(&self.path);
        let path = path.strip_prefix(cwd).unwrap_or({
            let home = HOME.get_or_init(|| crate::dirs::BASE_DIRS.home_dir().to_path_buf());

            path.strip_prefix(home)
                .map(|path| {
                    home_path.push("~");
                    home_path = home_path.join(path);
                    home_path.as_path()
                })
                .unwrap_or(path)
        });
        let path = path.display();

        let path_label = if taken_width > winwidth {
            format!("[{}]", path)
        } else {
            let available_width = winwidth - taken_width;
            if path_len > available_width && available_width > 3 {
                let diff = path_len - available_width;
                adjustment = 2;
                let path = path.to_string();
                let start = path
                    .char_indices()
                    .nth(diff + 2)
                    .map(|x| x.0)
                    .unwrap_or(path.len());
                let path = path[start..].to_string();
                format!("[…{}]", &path)
            } else {
                format!("[{}]", path)
            }
        };

        let path_len = path_label.len();
        let text_width = if path_len < winwidth {
            winwidth - path_len
        } else {
            winwidth
        } + adjustment;

        format!(
            "{text:<text_width$}{path_label}::::{path}::::{address}",
            text = name,
            text_width = text_width,
            path_label = path_label,
            path = self.path,
            address = self.address,
        )
    }

    pub fn parse(base: &Path, input: &str) -> anyhow::Result<Self> {
        let mut field = 0;
        let mut index_last = 0;

        let mut name = String::new();
        let mut path = String::new();
        let mut address = String::new();
        let mut kind = String::new();

        let mut is_parsing_address = false;

        let mut i = 0;
        let mut iter = input.chars();
        'outer: while let Some(mut c) = iter.next() {
            /* Address parse */
            if is_parsing_address {
                /* Parse a pattern-address */
                if c == '/' {
                    address.push(c);
                    let mut escape = false;
                    loop {
                        i += c.len_utf8();
                        c = iter.next().unwrap();
                        address.push(c);
                        if c == '\\' && escape == false {
                            escape = true;
                            continue;
                        }
                        if c == '\\' && escape == true {
                            escape = false;
                            continue;
                        }
                        if c == '/' && escape == true {
                            escape = false;
                            continue;
                        }
                        if c == '/' && escape == false {
                            /* Unescaped slash is end-of-pattern */
                            break;
                        }
                        /* case: escaped non-special character */
                        if escape {
                            // Let's just let it pass
                            escape = false;
                        }
                    }
                }
                /* Parse a line-number-address */
                else if c.is_digit(10) {
                    address.push(c);
                    loop {
                        i += c.len_utf8();
                        if let Some(c_) = iter.next() {
                            c = c_;
                            if !c.is_digit(10) {
                                break;
                            }
                            address.push(c);
                        } else {
                            /* case: end of string */
                            break 'outer;
                        }
                    }
                }
                /* Nothing else is accepted */
                else {
                    return Err(anyhow::anyhow!("Invalid tag line: invalid address"));
                }

                /* Cleanup end-of-tagaddress chars: `;` and `"` usually */
                loop {
                    i += c.len_utf8();
                    if let Some(c_) = iter.next() {
                        c = c_;
                        if c == '\t' {
                            field += 1;
                            index_last = i + c.len_utf8();
                            break;
                        }
                    } else {
                        /* case: end of string */
                        break 'outer;
                    }
                }
                is_parsing_address = false;
                i += c.len_utf8();
                continue 'outer;
            }

            /* Fields other than address are parsed here, because they are easier to match */
            if c == '\t' {
                match field {
                    0 => name.push_str(&input[index_last..i]),
                    1 => {
                        let path_buf = base.join(String::from(&input[index_last..i]));
                        path.push_str(&path_buf.to_string_lossy());
                        is_parsing_address = true;
                    }
                    2 => { /* skip: already parsed above */ }
                    3 => kind.push_str(&input[index_last..i]),
                    _ => {}
                }
                field += 1;
                index_last = i + c.len_utf8();
            }

            i += c.len_utf8();
        }
        match field {
            0 => name.push_str(&input[index_last..]),
            1 => {
                let path_buf = base.join(String::from(&input[index_last..]));
                path.push_str(&path_buf.to_string_lossy());
            }
            2 => { /* skip: already parsed above */ }
            3 => kind.push_str(&input[index_last..]),
            _ => {}
        }

        /* Not enough fields for us */
        if field <= 1 {
            Err(anyhow::anyhow!("Invalid tag line: not enough fields"))
        } else {
            Ok(TagItem {
                name,
                path,
                address,
                kind,
            })
        }
    }
}

/// Generate ctags recursively given the directory.
#[derive(Debug, Clone)]
pub struct TagFiles {
    /// The directory to find tagfiles.
    tagfiles: Vec<PathBuf>,
}

impl TagFiles {
    pub fn run(&self) -> Result<()> {
        let _tag_item_stream = self
            .tagfiles
            .iter()
            .filter_map(|tagfile| read_tag_file(tagfile).ok())
            .flatten();

        // TODO: tagfiles searcher

        Ok(())
    }
}

fn read_tag_file(tagfile: &Path) -> std::io::Result<impl Iterator<Item = TagItem> + '_> {
    let parent_dir = tagfile.parent().unwrap_or(tagfile);

    Ok(BufReader::new(File::open(tagfile)?)
        .lines()
        .filter_map(move |line| {
            line.ok().and_then(|input| {
                if input.starts_with("!_TAG") {
                    None
                } else if let Ok(tag) = TagItem::parse(parent_dir, &input) {
                    Some(tag)
                } else {
                    None
                }
            })
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tag_from_tagfile() {
        let empty_path = PathBuf::new();

        // With escaped characters
        let data = r#"run	crates/maple_cli/src/app.rs	/^	pub \/* \\\/ *\/ fn	run(self) -> Result<()> {$/;"	P	implementation:Maple"#;
        let tag = TagItem::parse(&empty_path, data).unwrap();
        assert_eq!(tag.name, "run");
        assert_eq!(tag.path, "crates/maple_cli/src/app.rs");
        assert_eq!(
            tag.address,
            r#"/^	pub \/* \\\/ *\/ fn	run(self) -> Result<()> {$/"#
        );
        assert_eq!(tag.kind, "P");

        // With invalid escaped characters
        let data = r#"tag_name	filepath_here.py	/def ta\g_name/	f	"#;
        let tag = TagItem::parse(&empty_path, data).unwrap();
        assert_eq!(tag.name, "tag_name");
        assert_eq!(tag.path, "filepath_here.py");
        assert_eq!(tag.address, r#"/def ta\g_name/"#);
        assert_eq!(tag.kind, "f");

        // with different characters after pattern
        let data = r#"tagname	filename	/pattern/[;"	f	"#;
        let tag = TagItem::parse(&empty_path, data).unwrap();
        assert_eq!(tag.address, "/pattern/");

        // Without kind
        let data = r#"tag_with_no_kind	filepath_here.py	/tag_with_no_kind/"#;
        let tag = TagItem::parse(&empty_path, data).unwrap();
        assert_eq!(tag.name, "tag_with_no_kind");
        assert_eq!(tag.path, "filepath_here.py");
        assert_eq!(tag.address, r#"/tag_with_no_kind/"#);
        assert_eq!(tag.kind, "");

        // Invalid: pattern
        let data = r#"tag_name	filename.py	invalid/pattern/;""	f	"#;
        assert_eq!(TagItem::parse(&empty_path, data).is_err(), true);

        // With line-number address
        let data = r#".Button.--icon .Button__icon	client/src/styles/Button.scss	86;"	r"#;
        let tag = TagItem::parse(&empty_path, data).unwrap();
        assert_eq!(tag.name, ".Button.--icon .Button__icon");
        assert_eq!(tag.path, "client/src/styles/Button.scss");
        assert_eq!(tag.address, r#"86"#);
        assert_eq!(tag.kind, "r");

        // With line-number address (hasktags style)
        let data = r#"EndlessList	example.hs	3"#;
        let tag = TagItem::parse(&empty_path, data).unwrap();
        assert_eq!(tag.name, "EndlessList");
        assert_eq!(tag.path, "example.hs");
        assert_eq!(tag.address, r#"3"#);
    }
}