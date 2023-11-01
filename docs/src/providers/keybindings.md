# Keybindings

## Keybindings

### Insert mode

- [x] Use <kbd>Ctrl-j</kbd>/<kbd>Down</kbd> or <kbd>Ctrl-k</kbd>/<kbd>Up</kbd> to navigate the result list up and down linewise.
- [x] Use <kbd>PageDown</kbd>/<kbd>PageUp</kbd> to scroll the result list down and up.
- [x] Use <kbd>Ctrl-a</kbd>/<kbd>Home</kbd> to go to the start of the input.
- [x] Use <kbd>Ctrl-e</kbd>/<kbd>End</kbd> to go to the end of the input.
- [x] Use <kbd>Ctrl-c</kbd>, <kbd>Ctrl-g</kbd>, <kbd>Ctrl-[</kbd> or <kbd>Esc</kbd>(vim) to exit.
- [x] Use <kbd>Ctrl-h</kbd>/<kbd>BS</kbd> to delete previous character.
- [x] Use <kbd>Ctrl-d</kbd> to delete next character.
- [x] Use <kbd>Ctrl-b</kbd> to move cursor left one character.
- [x] Use <kbd>Ctrl-f</kbd> to move cursor right one character.
- [x] Use <kbd>Ctrl-n</kbd> for next input in the history.
- [x] Use <kbd>Ctrl-p</kbd> for previous input in the history.
- [x] Use <kbd>Enter</kbd> to select the entry and exit.
  - Use <kbd>Enter</kbd> to expand the directory or edit the file for `:Clap filer`.
- [x] By default <kbd>Alt-u</kbd> does nothing.
  - Use <kbd>Alt-u</kbd> to go up one directory in `:Clap filer`.
- [x] Use <kbd>Tab</kbd> to select multiple entries and open them using the quickfix window.(Need the provider has `sink*` support)
  - Use <kbd>Tab</kbd> to expand the directory for `:Clap filer`.
- [x] Use <kbd>Ctrl-t</kbd> or <kbd>Ctrl-x</kbd>, <kbd>Ctrl-v</kbd> to open the selected entry in a new tab or a new split.
- [x] Use <kbd>Ctrl-u</kbd> to clear inputs.
- [x] Use <kbd>Ctrl-l</kbd> to launch the whole provider list panel for invoking another provider at any time.
- [x] Use <kbd>Shift-Tab</kbd> to invoke the action dialog(vim only).
- [x] Use <kbd>Shift-up</kbd> and <kbd>Shift-down</kbd> to scroll the preview.

### NeoVim only

#### Normal mode

- [x] Use <kbd>j</kbd>/<kbd>Down</kbd> or <kbd>k</kbd>/<kbd>Up</kbd> to navigate the result list up and down linewise.
- [x] By default <kbd>Alt-u</kbd> does nothing.
  - Use <kbd>Alt-u</kbd> to go up one directory in `:Clap filer`.
- [x] Use <kbd>Ctrl-c</kbd>, <kbd>Ctrl-g</kbd> or <kbd>Esc</kbd> to exit.
- [x] Use <kbd>Ctrl-d</kbd>/<kbd>Ctrl-u</kbd>/<kbd>PageDown</kbd>/<kbd>PageUp</kbd> to scroll the result list down and up.
- [x] Use <kbd>Ctrl-l</kbd> to launch the whole provider list panel for invoking another provider at any time.
- [x] Use <kbd>Ctrl-n</kbd> for next input in the history.
- [x] Use <kbd>Ctrl-p</kbd> for previous input in the history.
- [x] Use <kbd>Shift-up</kbd> and <kbd>Shift-down</kbd> to scroll the preview.
- [x] Use <kbd>gg</kbd> and <kbd>G</kbd> to scroll to the first and last item.
- [x] Use <kbd>Enter</kbd> to select the entry and exit.
- [x] Use <kbd>Shift-Tab</kbd> to invoke the action dialog.
- [x] Actions defined by `g:clap_open_action`.

#### Cmdline mode

- [x] Use `:q` to exit.

See `:help clap-keybindings` for more information. Note that the [keybindings are not consistent](https://github.com/liuchengxu/vim-clap/issues/864) due to discrepancies between Vim/Neovim and different providers.
