use crate::stdio_server::plugin::PluginError;
use crate::stdio_server::vim::Vim;
use crate::tools::ctags::BufferTag;
use itertools::Itertools;
use maple_config::FilePathStyle;

fn shrink_text_to_fit(path: String, max_width: usize) -> String {
    if path.len() < max_width {
        path
    } else {
        const DOTS: char = '…';
        let to_shrink_len = path.len() - max_width;
        let left = path.chars().take(max_width / 2).collect::<String>();
        let right = path
            .chars()
            .skip(max_width / 2 + 1 + to_shrink_len)
            .collect::<String>();
        format!("{left}{DOTS}{right}")
    }
}

pub enum FunctionTag<'a> {
    /// The nearest available tag to the cursor.
    CursorTag(&'a BufferTag),
    /// No cursor tag available, but there are other tags.
    Ellipsis,
    /// Nothing to show.
    None,
}

impl<'a> FunctionTag<'a> {
    fn tag(&self) -> Option<&BufferTag> {
        match self {
            Self::CursorTag(tag) => Some(tag),
            _ => None,
        }
    }
}

pub async fn update_winbar<'a>(
    vim: &Vim,
    bufnr: usize,
    function_tag: FunctionTag<'a>,
) -> Result<(), PluginError> {
    let winid = vim.bare_call::<usize>("win_getid").await?;
    let winwidth = vim.winwidth(winid).await?;

    let path = vim.expand(format!("#{bufnr}")).await?;

    let mut winbar_items = Vec::new();

    let winbar_config = &maple_config::config().winbar;

    let separator = format!(" {} ", winbar_config.separator);
    let text_hl = winbar_config.text_highlight.as_str();

    match winbar_config.file_path_style {
        FilePathStyle::OneSegmentPerComponent => {
            // TODO: Cache the filepath section.
            let mut segments = path.split(std::path::MAIN_SEPARATOR);

            // Do not prepend the separator to the first segment.
            if let Some(seg) = segments.next() {
                // seg could be empty when path starts from the root, e.g., /Users/xuliucheng.
                if seg.is_empty() {
                    if let Some(seg) = segments.next() {
                        winbar_items.push((text_hl, seg.to_string()));
                    }
                } else {
                    winbar_items.push((text_hl, seg.to_string()));
                }
            }

            winbar_items.extend(
                segments.flat_map(|seg| [(text_hl, separator.clone()), (text_hl, seg.to_string())]),
            );

            // Add icon to the filename.
            if let Some(last) = winbar_items.pop() {
                winbar_items.extend([
                    ("Label", format!("{} ", icon::file_icon(&last.1))),
                    (text_hl, last.1),
                ]);
            }
        }
        FilePathStyle::FullPath => {
            let max_width = match function_tag.tag() {
                Some(tag) => {
                    if tag.scope.is_some() {
                        winwidth / 2
                    } else {
                        winwidth * 2 / 3
                    }
                }
                None => winwidth,
            };
            let path = if let Some(home) = dirs::Dirs::base().home_dir().to_str() {
                path.replacen(home, "~", 1)
            } else {
                path
            };
            winbar_items.push(("Label", format!(" {} ", icon::file_icon(&path))));
            winbar_items.push((text_hl, shrink_text_to_fit(path, max_width)));
        }
    }

    match function_tag {
        FunctionTag::CursorTag(tag) => {
            if vim.call::<usize>("winbufnr", [winid]).await? == bufnr {
                if let Some(scope) = &tag.scope {
                    let mut scope_kind_icon = icon::tags_kind_icon(&scope.scope_kind).to_string();
                    scope_kind_icon.push(' ');
                    let scope_max_width = winwidth / 4 - scope_kind_icon.len();
                    let scope_item = shrink_text_to_fit(scope.scope.clone(), scope_max_width);
                    winbar_items.extend([
                        (text_hl, separator.clone()),
                        ("Include", scope_kind_icon),
                        (text_hl, scope_item),
                    ]);
                }

                let tag_kind_icon = icon::tags_kind_icon(&tag.kind).to_string();
                winbar_items.extend([
                    (text_hl, separator),
                    ("Type", tag_kind_icon),
                    (text_hl, format!(" {}", &tag.name)),
                ]);
            }
        }
        FunctionTag::Ellipsis => {
            let mut winbar: String = winbar_items
                .iter()
                .map(|(highlight, value)| format!("%#{highlight}#{value}%*"))
                .join("");

            winbar.push_str(&format!("%#{text_hl}#{separator}%*"));
            winbar.push_str(&format!("%@clap#api#on_click_function_tag@...%X"));

            vim.exec("clap#api#update_winbar", (winid, winbar))?;
            return Ok(());
        }
        FunctionTag::None => {}
    }

    if winbar_items.is_empty() {
        vim.exec("clap#api#update_winbar", (winid, ""))?;
    } else {
        let winbar = winbar_items
            .iter()
            .map(|(highlight, value)| format!("%#{highlight}#{value}%*"))
            .join("");

        vim.exec("clap#api#update_winbar", (winid, winbar))?;
    }

    Ok(())
}
