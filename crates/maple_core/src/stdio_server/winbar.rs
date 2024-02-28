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

pub async fn update_winbar(
    vim: &Vim,
    bufnr: usize,
    tag: Option<&BufferTag>,
) -> Result<(), PluginError> {
    const SEP: char = '';

    let winid = vim.bare_call::<usize>("win_getid").await?;
    let winwidth = vim.winwidth(winid).await?;

    let path = vim.expand(format!("#{bufnr}")).await?;

    let mut winbar_items = Vec::new();

    let path_style = &maple_config::config().winbar.file_path_style;

    match path_style {
        FilePathStyle::OneSegmentPerComponent => {
            // TODO: Cache the filepath section.
            let mut segments = path.split(std::path::MAIN_SEPARATOR);

            if let Some(seg) = segments.next() {
                winbar_items.push(("Normal", seg.to_string()));
            }

            winbar_items.extend(
                segments
                    .flat_map(|seg| [("LineNr", format!(" {SEP} ")), ("Normal", seg.to_string())]),
            );

            // Add icon to the filename.
            if let Some(last) = winbar_items.pop() {
                winbar_items.extend([
                    ("Label", format!("{} ", icon::file_icon(&last.1))),
                    ("Normal", last.1),
                ]);
            }
        }
        FilePathStyle::FullPath => {
            let max_width = match tag {
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
            winbar_items.push(("LineNr", shrink_text_to_fit(path, max_width)));
        }
    }

    if let Some(tag) = tag {
        if vim.call::<usize>("winbufnr", [winid]).await? == bufnr {
            if let Some(scope) = &tag.scope {
                let mut scope_kind_icon = icon::tags_kind_icon(&scope.scope_kind).to_string();
                scope_kind_icon.push(' ');
                let scope_max_width = winwidth / 4 - scope_kind_icon.len();
                winbar_items.extend([
                    ("LineNr", format!(" {SEP} ")),
                    ("Include", scope_kind_icon),
                    (
                        "LineNr",
                        shrink_text_to_fit(scope.scope.clone(), scope_max_width),
                    ),
                ]);
            }

            let tag_kind_icon = icon::tags_kind_icon(&tag.kind).to_string();
            winbar_items.extend([
                ("LineNr", format!(" {SEP} ")),
                ("Type", tag_kind_icon),
                ("LineNr", format!(" {}", &tag.name)),
            ]);
        }
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
