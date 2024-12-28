pub mod text_file;
pub mod vim_help;

/// Truncates the lines that are awfully long as vim can not handle them properly.
///
/// Ref https://github.com/liuchengxu/vim-clap/issues/543
pub fn truncate_lines(
    lines: impl Iterator<Item = String>,
    max_width: usize,
) -> impl Iterator<Item = String> {
    lines.map(move |line| {
        if line.len() > max_width {
            let mut line = line;
            // https://github.com/liuchengxu/vim-clap/pull/544#discussion_r506281014
            let replace_start = (0..max_width + 1)
                .rev()
                .find(|idx| line.is_char_boundary(*idx))
                .unwrap_or_default(); // truncate to 0
            line.replace_range(replace_start.., "……");
            line
        } else {
            line
        }
    })
}
