use crate::cmd::cache::{cache_exists, send_response_from_cache, CacheEntry, SendResponse};
use anyhow::Result;
use filter::{matcher::LineSplitter, Source};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use anyhow::anyhow;

#[derive(Serialize, Deserialize, Debug)]
struct TagInfo {
    name: String,
    path: String,
    pattern: String,
    kind: String,
}

impl TagInfo {
    pub fn format(&self, cwd: &PathBuf, winwidth: usize) -> String {
        static HOME: OnceCell<Option<PathBuf>> = OnceCell::new();

        let name = format!("{} ", self.name);
        let taken_width = name.len() + 1;
        let path_len = self.path.len() + 2;
        let mut adjustment = 0;

        let mut home_path = PathBuf::new();
        let path = Path::new(&self.path);
        let path = path.strip_prefix(cwd).unwrap_or(
            HOME.get_or_init(|| dirs::home_dir())
                .as_deref()
                .map(|home| {
                    path.strip_prefix(home)
                        .map(|path| {
                            home_path.push("~");
                            home_path = home_path.join(path);
                            home_path.as_path()
                        })
                        .unwrap_or(path)
                })
                .unwrap_or(path),
        );
        let path = path.display();

        let path_label = if taken_width > winwidth {
            format!("[{}]", path)
        } else {
            let available_width = winwidth - taken_width;
            if path_len > available_width && available_width > 3 {
                let diff = path_len - available_width;
                adjustment = 2;
                let path = path.to_string();
                let start = path.char_indices().nth(diff + 2).map(|x| x.0).unwrap_or(path.len());
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
            "{text:<text_width$}{path_label}:::{path}:::{pattern}",
            text = name,
            text_width = text_width,
            path_label = path_label,
            path = self.path,
            pattern = self.pattern,
        )
    }

    pub fn parse(base: &PathBuf, input: &str) -> anyhow::Result<Self> {
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
                            continue
                        }
                        if c == '\\' && escape == true {
                            escape = false;
                            continue
                        }
                        if c == '/' && escape == true {
                            escape = false;
                            continue
                        }
                        if c == '/' && escape == false {
                            /* Unescaped slash is end-of-pattern */
                            break
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
                                break
                            }
                            address.push(c);
                        }
                        else {
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
                            break
                        }
                    }
                    else {
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
                        write!(&mut path, "{}", path_buf.display()).unwrap();
                        is_parsing_address = true;
                    }
                    2 => { /* skip: already parsed above */ },
                    3 => {
                        kind.push_str(&input[index_last..i])
                    },
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
                write!(&mut path, "{}", path_buf.display()).unwrap();
            }
            2 => { /* skip: already parsed above */ },
            3 => kind.push_str(&input[index_last..]),
            _ => {}
        }

        /* Not enough fields for us */
        if field <= 1 {
            Err(anyhow::anyhow!{"Invalid tag line: not enough fields"})
        }
        else {
            Ok(TagInfo {
                name,
                path,
                address,
                kind,
            })
        }
    }
}

/// Generate ctags recursively given the directory.
#[derive(StructOpt, Debug, Clone)]
pub struct TagFiles {
    /// Initial query string
    #[structopt(index = 1, short, long)]
    query: String,

    /// The directory to generate recursive ctags.
    #[structopt(long, parse(from_os_str))]
    files: Vec<PathBuf>,

    /// Specify the language.
    #[structopt(long = "languages")]
    languages: Option<String>,

    /// Read input from a cached grep tempfile, only absolute file path is supported.
    #[structopt(long = "input", parse(from_os_str))]
    input: Option<PathBuf>,
}

fn read_tag_files<'a>(
    cwd: &'a PathBuf,
    winwidth: usize,
    files: &'a [[PathBuf; 2]],
) -> Result<impl Iterator<Item = String> + 'a> {
    Ok(files
        .into_iter()
        .filter_map(move |path| read_tag_file(path, &cwd, winwidth).ok())
        .flatten())
}

fn read_tag_file<'a>(
    paths: &'a [PathBuf; 2],
    cwd: &'a PathBuf,
    winwidth: usize,
) -> Result<impl Iterator<Item = String> + 'a> {
    let file = File::open(&paths[0]);
    let file = if let Ok(file) = file {
        file
    } else {
        return Err(anyhow!{"File does not exists"});
    };

    Ok(BufReader::new(file).lines().filter_map(move |line| {
        line.ok().and_then(|input| {
            if input.starts_with("!_TAG") {
                None
            } else if let Ok(tag) = TagInfo::parse(&paths[1], &input) {
                Some(tag.format(&cwd, winwidth))
            } else {
                None
            }
        })
    }))
}

fn get_paths_from_files<'a>(files: &'a Vec<PathBuf>) -> Vec<[PathBuf; 2]> {
    files
        .iter()
        .map(|path| {
            let mut dirname = path.clone();
            dirname.pop();
            [path.to_owned(), dirname]
        })
        .collect::<Vec<_>>()
}

impl TagFiles {
    pub fn run(&self, options: &crate::Maple) -> Result<()> {
        /* let icon_painter = options
         *     .icon_painter
         *     .clone()
         *     .map(|_| icon::IconPainter::ProjTags); */

        let cwd = options.cwd.clone().unwrap();
        let winwidth = options.winwidth.unwrap_or(120);

        let files = &self.files.clone();
        let tag_paths = get_paths_from_files(files);

        filter::dyn_run(
            &self.query,
            Source::List(read_tag_files(&cwd, winwidth, &tag_paths)?),
            None,
            Some(30),
            None,
            None,
            LineSplitter::TagNameOnly,
        )?;

        Ok(())
    }
}

#[test]
fn test_parse_ctags_line() {
    let empty_path = PathBuf::new();

    // With escaped characters
    let data =
        r#"run	crates/maple_cli/src/app.rs	/^	pub \/* \\\/ *\/ fn	run(self) -> Result<()> {$/;"	P	implementation:Maple"#;
    let tag = TagInfo::parse(&empty_path, data).unwrap();
    assert_eq!(tag.name, "run");
    assert_eq!(tag.path, "crates/maple_cli/src/app.rs");
    assert_eq!(tag.address, r#"/^	pub \/* \\\/ *\/ fn	run(self) -> Result<()> {$/"#);
    assert_eq!(tag.kind, "P");

    // With invalid escaped characters
    let data =
        r#"tag_name	filepath_here.py	/def ta\g_name/	f	"#;
    let tag = TagInfo::parse(&empty_path, data).unwrap();
    assert_eq!(tag.name, "tag_name");
    assert_eq!(tag.path, "filepath_here.py");
    assert_eq!(tag.address, r#"/def ta\g_name/"#);
    assert_eq!(tag.kind, "f");

    // Without kind
    let data =
        r#"tag_with_no_kind	filepath_here.py	/tag_with_no_kind/"#;
    let tag = TagInfo::parse(&empty_path, data).unwrap();
    assert_eq!(tag.name, "tag_with_no_kind");
    assert_eq!(tag.path, "filepath_here.py");
    assert_eq!(tag.address, r#"/tag_with_no_kind/"#);
    assert_eq!(tag.kind, "");

    // Invalid: pattern
    let data =
        r#"tag_name	filename.py	invalid/pattern/;""	f	"#;
    let failed = TagInfo::parse(&empty_path, data).is_err();
    assert_eq!(failed, true);

    // With line-number address
    let data = r#".Button.--icon .Button__icon	client/src/styles/Button.scss	86;"	r"#;
    let tag = TagInfo::parse(&empty_path, data).unwrap();
    assert_eq!(tag.name, ".Button.--icon .Button__icon");
    assert_eq!(tag.path, "client/src/styles/Button.scss");
    assert_eq!(tag.address, r#"86"#);
    assert_eq!(tag.kind, "r");

    // With line-number address (hasktags style)
    let data = r#"EndlessList	example.hs	3"#;
    let tag = TagInfo::parse(&empty_path, data).unwrap();
    assert_eq!(tag.name, "EndlessList");
    assert_eq!(tag.path, "example.hs");
    assert_eq!(tag.address, r#"3"#);
    assert_eq!(tag.kind, "");
}

