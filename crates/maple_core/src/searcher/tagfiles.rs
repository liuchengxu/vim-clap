use crate::searcher::SearchContext;
use crate::stdio_server::SearchProgressor;
use dirs::Dirs;
use filter::BestItems;
use matcher::Matcher;
use printer::Printer;
use std::borrow::Cow;
use std::fs::File;
use std::io::{BufRead, BufReader, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use types::{ClapItem, MatchedItem, SearchProgressUpdate};

#[allow(dead_code)]
#[derive(Debug)]
struct TagItem {
    name: String,
    path: String,
    address: String,
    // TODO: display kind?
    kind: String,
    display: Option<String>,
}

impl TagItem {
    pub fn format(&self, cwd: &Path, winwidth: usize) -> String {
        let taken_width = self.name.len() + 1;
        let path_len = self.path.len() + 2;
        let mut adjustment = 0;

        let mut home_path = PathBuf::new();
        let path = Path::new(&self.path);
        let path = path.strip_prefix(cwd).unwrap_or({
            path.strip_prefix(Dirs::base().home_dir())
                .map(|path| {
                    home_path.push("~");
                    home_path = home_path.join(path);
                    home_path.as_path()
                })
                .unwrap_or(path)
        });
        let path = path.display();

        let path_label = if taken_width > winwidth {
            format!("[{path}]")
        } else {
            let available_width = winwidth - taken_width;
            if path_len > available_width && available_width > 3 {
                let diff = path_len - available_width;
                adjustment = 2;
                let start = self
                    .path
                    .char_indices()
                    .nth(diff + 2)
                    .map(|x| x.0)
                    .unwrap_or(self.path.len());
                format!("[…{}]", &self.path[start..])
            } else {
                format!("[{path}]")
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
            text = self.name,
            text_width = text_width,
            path_label = path_label,
            path = self.path,
            address = self.address,
        )
    }

    pub fn parse(base: &Path, input: &str) -> std::io::Result<Self> {
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
                        if c == '\\' && !escape {
                            escape = true;
                            continue;
                        }
                        if c == '\\' && escape {
                            escape = false;
                            continue;
                        }
                        if c == '/' && escape {
                            escape = false;
                            continue;
                        }
                        if c == '/' && !escape {
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
                else if c.is_ascii_digit() {
                    address.push(c);
                    loop {
                        i += c.len_utf8();
                        if let Some(c_) = iter.next() {
                            c = c_;
                            if !c.is_ascii_digit() {
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
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Invalid tag line: invalid address",
                    ));
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
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Invalid tag line: not enough fields",
            ))
        } else {
            Ok(TagItem {
                name,
                path,
                address,
                kind,
                display: None,
            })
        }
    }
}

impl ClapItem for TagItem {
    fn raw_text(&self) -> &str {
        self.name.as_str()
    }

    fn output_text(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            self.display
                .as_ref()
                .expect("must be initialized when parsing"),
        )
    }
}

fn read_tag_file<'a>(
    tagfile: &'a Path,
    cwd: &'a Path,
    winwidth: usize,
) -> std::io::Result<impl Iterator<Item = TagItem> + 'a> {
    let parent_dir = tagfile.parent().unwrap_or(tagfile);

    Ok(BufReader::new(File::open(tagfile)?)
        .lines()
        .filter_map(move |line| {
            line.ok().and_then(|input| {
                if input.starts_with("!_TAG") {
                    None
                } else if let Ok(mut tag) = TagItem::parse(parent_dir, &input) {
                    // TODO: display text can be lazy evalulated.
                    tag.display.replace(tag.format(cwd, winwidth));
                    Some(tag)
                } else {
                    None
                }
            })
        }))
}

fn search_tagfiles(
    tagfiles: Vec<PathBuf>,
    cwd: PathBuf,
    winwidth: usize,
    matcher: Matcher,
    stop_signal: Arc<AtomicBool>,
    item_sender: UnboundedSender<MatchedItem>,
    total_processed: Arc<AtomicUsize>,
) -> Result<()> {
    let _ = tagfiles
        .iter()
        .filter_map(|tagfile| read_tag_file(tagfile, &cwd, winwidth).ok())
        .flatten()
        .try_for_each(|tag_item| {
            if stop_signal.load(Ordering::SeqCst) {
                return Err(());
            }

            total_processed.fetch_add(1, Ordering::Relaxed);

            if let Some(matched_item) = matcher.match_item(Arc::new(tag_item)) {
                item_sender.send(matched_item).map_err(|_| ())?;
            }

            Ok(())
        });

    Ok(())
}

pub async fn search(query: String, cwd: PathBuf, matcher: Matcher, search_context: SearchContext) {
    let SearchContext {
        icon,
        line_width,
        paths,
        vim,
        stop_signal,
        item_pool_size,
    } = search_context;

    let printer = Printer {
        line_width,
        icon,
        truncate_text: false,
    };
    let number = item_pool_size;
    let progressor = SearchProgressor::new(vim, stop_signal.clone());

    let mut best_items = BestItems::new(printer, number, progressor, Duration::from_millis(200));

    let (sender, mut receiver) = unbounded_channel();

    let total_processed = Arc::new(AtomicUsize::new(0));

    {
        let total_processed = total_processed.clone();

        std::thread::Builder::new()
            .name("tagfiles-worker".into())
            .spawn({
                let stop_signal = stop_signal.clone();
                let tagfiles = paths.into_iter().map(|p| p.join("tags")).collect();
                move || {
                    search_tagfiles(
                        tagfiles,
                        cwd,
                        line_width,
                        matcher,
                        stop_signal,
                        sender,
                        total_processed,
                    )
                }
            })
            .expect("Failed to spawn tagfiles worker thread");
    }

    let mut total_matched = 0usize;

    let now = std::time::Instant::now();

    while let Some(matched_item) = receiver.recv().await {
        if stop_signal.load(Ordering::SeqCst) {
            return;
        }
        total_matched += 1;
        let total_processed = total_processed.load(Ordering::Relaxed);
        best_items.on_new_match(matched_item, total_matched, total_processed);
    }

    let elapsed = now.elapsed().as_millis();

    let BestItems {
        items,
        progressor,
        printer,
        ..
    } = best_items;

    let display_lines = printer.to_display_lines(items);
    let total_processed = total_processed.load(Ordering::SeqCst);

    progressor.on_finished(display_lines, total_matched, total_processed);

    tracing::debug!(
        total_processed,
        total_matched,
        ?query,
        "Searching completed in {elapsed:?}ms"
    );
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
            r"/^	pub \/* \\\/ *\/ fn	run(self) -> Result<()> {$/"
        );
        assert_eq!(tag.kind, "P");

        // With invalid escaped characters
        let data = r"tag_name	filepath_here.py	/def ta\g_name/	f	";
        let tag = TagItem::parse(&empty_path, data).unwrap();
        assert_eq!(tag.name, "tag_name");
        assert_eq!(tag.path, "filepath_here.py");
        assert_eq!(tag.address, r"/def ta\g_name/");
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
        assert!(TagItem::parse(&empty_path, data).is_err());

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

    #[test]
    #[ignore]
    fn test_read_tags() {
        let cur_dir = std::env::current_dir().unwrap();
        let cwd = cur_dir.parent().unwrap().parent().unwrap();
        let tags_file = cwd.join("tags");
        let tagitems = read_tag_file(&tags_file, cwd, 60)
            .unwrap()
            .collect::<Vec<_>>();
        println!("{tagitems:#?}");
    }
}
