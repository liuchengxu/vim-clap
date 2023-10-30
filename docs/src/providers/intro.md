# Clap Providers

## Builtin Providers

Additional requirement means the potential extra tool needed for the particular provider besides the Rust binary [`maple`](../guide/install_rust.md).

| Command                                | List                                                   | Additional Requirement                                                             |
| :------------------------------------- | :----------------------------------------------------- | :---------------------------------------------------------------------- |
| `Clap blines`                          | Lines in the current buffer                            | _none_                                                                  |
| `Clap buffers`                         | Open buffers                                           | _none_                                                                  |
| `Clap colors`                          | Colorschemes                                           | _none_                                                                  |
| `Clap command`                         | Command                                                | _none_                                                                  |
| `Clap hist:` or `Clap command_history` | Command history                                        | _none_                                                                  |
| `Clap hist/` or `Clap search_history`  | Search history                                         | _none_                                                                  |
| `Clap filetypes`                       | File types                                             | _none_                                                                  |
| `Clap help_tags`                       | Help tags                                              | _none_                                                                  |
| `Clap jumps`                           | Jumps                                                  | _none_                                                                  |
| `Clap lines`                           | Lines in the loaded buffers                            | _none_                                                                  |
| `Clap marks`                           | Marks                                                  | _none_                                                                  |
| `Clap maps`                            | Maps                                                   | _none_                                                                  |
| `Clap quickfix`                        | Entries of the quickfix list                           | _none_                                                                  |
| `Clap loclist`                         | Entries of the location list                           | _none_                                                                  |
| `Clap registers`                       | Registers                                              | _none_                                                                  |
| `Clap yanks`                           | Yank stack of the current vim session                  | _none_                                                                  |
| `Clap history`                         | Open buffers and `v:oldfiles`                          | _none_                                                                  |
| `Clap windows`                         | Windows                                                | _none_                                                                  |
| `Clap providers`                       | List the vim-clap providers                            | _none_                                                                  |
| `Clap bcommits`                        | Git commits for the current buffer                     | **[git][git]**                                                          |
| `Clap commits`                         | Git commits                                            | **[git][git]**                                                          |
| `Clap gfiles` or `Clap git_files`      | Files managed by git                                   | **[git][git]**                                                          |
| `Clap git_diff_files`                  | Files managed by git and having uncommitted changes    | **[git][git]**                                                          |
| _`Clap live_grep` (**deprecated**)_          | Grep using word-regexp matcher                         | **[rg][rg]**                                                            |
| `Clap dumb_jump`                       | Definitions/References using regexp with grep fallback | **[rg][rg]** with `--pcre2`                                             |
| `Clap files`                           | Files                                                  | _none_
| `Clap filer`                           | Ivy-like file explorer                                 | _none_
| `Clap grep`**<sup>+</sup>**            | Grep using fuzzy matcher                               | _none_
| `Clap igrep`                           | A combo of `filer` and `grep`                          | _none_
| `Clap tags`                            | Tags in the current buffer                             | _none_
| `Clap tagfiles`                        | Search existing `tagfiles`                             | _none_
| `Clap proj_tags`                       | Tags in the current project                            | **[universal-ctags][universal-ctags]** (`+json`)
| `Clap recent_files`                    | Persistent ordered history of recent files             | _none_

[rg]: https://github.com/BurntSushi/ripgrep
[git]: https://github.com/git/git
[universal-ctags]: https://github.com/universal-ctags/ctags

- The command with a superscript `!` means that it is not yet implemented or not tested.
- The command with a superscript `+` means that it supports multi-selection via <kbd>Tab</kbd>.
- `:Clap grep`
  - Use `:Clap grep --query=<cword>` to grep the word under cursor.
  - Use `:Clap grep --query=@visual` to grep the visual selection.
  - `cwd` will be searched by default, specify the extra paths in the end to search multiple directories.
    - `:Clap grep --path ~/.vim/plugged/ale` with `cwd` is `~/.vim/plugged/vim-clap` will both search vim-clap and ale.

[Send a pull request](https://github.com/liuchengxu/vim-clap/pulls) if certain provider is not listed here.

### Global variables

- `g:clap_layout`: Dict, `{ 'width': '67%', 'height': '33%', 'row': '33%', 'col': '17%' }` by default. This variable controls the size and position of vim-clap window. By default, the vim-clap window is placed relative to the currently active window. To make it relative to the whole editor modify this variable as shown below:

  ```vim
  let g:clap_layout = { 'relative': 'editor' }
  ```

- `g:clap_open_action`: Dict, `{ 'ctrl-t': 'tab split', 'ctrl-x': 'split', 'ctrl-v': 'vsplit' }`, extra key bindings for opening the selected file in a different way. NOTE: do not define a key binding which is conflicted with the other default bindings of vim-clap, and only `ctrl-*` is supported for now.

- `g:clap_provider_alias`: Dict, if you don't want to invoke some clap provider by its id(name), as it's too long or somehow, you can add an alias for that provider.

  ```vim
  " The provider name is `command_history`, with the following alias config,
  " now you can call it via both `:Clap command_history` and `:Clap hist:`.
  let g:clap_provider_alias = {'hist:': 'command_history'}
  ```

- `g:clap_selected_sign`: Dict, `{ 'text': ' >', 'texthl': "ClapSelectedSign", "linehl": "ClapSelected"}`.

- `g:clap_current_selection_sign`: Dict, `{ 'text': '>>', 'texthl': "ClapCurrentSelectionSign", "linehl": "ClapCurrentSelection"}`.

- `g:clap_no_matches_msg`: String, `'NO MATCHES FOUND'`, message to show when there is no matches found.

- `g:clap_popup_input_delay`: Number, `200ms` by default, delay for actually responsing to the input, vim only.

- `g:clap_disable_run_rooter`: Bool, `v:false`, vim-clap by default will try to run from the project root by changing `cwd` temporarily. Set it to `v:true` to run from the origin `cwd`. The project root here means the git base directory. Create an issue if you want to see more support about the project root.

The option naming convention for provider is `g:clap_provider_{provider_id}_{opt}`.

- `g:clap_provider_grep_blink`: [2, 100] by default, blink 2 times with 100ms timeout when jumping the result. Set it to [0, 0] to disable the blink.

- `g:clap_provider_grep_opts`: An empty string by default, allows you to enable flags such as `'--hidden -g "!.git/"'`.

See `:help clap-options` for more information.
