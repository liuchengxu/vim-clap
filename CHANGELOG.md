CHANGELOG
=========

## [unreleased]

### Added

- New provider `:Clap lines`.
- New provider `:Clap history`.
- New provider `:Clap yanks` thanks to @ratheesh.
- New external filter `fzy` and `fzf`.
- Every provider could run async if you have one of the external filters installed.
- Add the substring filter mode.
- Add the preview support for `:Clap marks` and `:Clap jumps`.
- Add the option `g:clap_provider_grep_enable_icon` for disabling the icon drawing in `:Clap grep`.
- Rework the `buffers` provider.([#71](https://github.com/liuchengxu/vim-clap/issues/71))
- Support opening the selected file via <kbd>ctrl-t</kbd>, <kbd>ctrl-x</kbd>, <kbd>ctrl-v</kbd> if the provider supports, and add `g:clap_open_action` for configuring the default keybindings.
- Support `:Clap` listing all the builtin providers, thanks to @wookayin implementing the sink of it.
- Support opening files from the git base directory. See `:h g:clap_disable_run_rooter` if you don't like this behavior.
- Support searching the hidden files via `:Clap files --hidden`.
- Add `g:clap_provider_grep_opts` for globally configuring the used command line options of rg, thanks to @Olical.
- Support using any other finder tools via `:Clap files ++finder=[YOUR FINDER] [FINDER ARGS]`.
- Add search box border symbols support.([#85](https://github.com/liuchengxu/vim-clap/pull/85))

### Improved

- Do not try using the default async filter implementation if none of the external filters are avaliable.([#61](https://github.com/liuchengxu/vim-clap/issues/61))
- Always use the sign to indicate the selected and current selection.

### Fixed

Various fixes.

### Changed

- Rename `g:clap_selected_sign_definition` to `g:clap_selected_sign`.
- Rename `g:clap_current_selection_sign_definition` to `g:clap_current_selection_sign`.
- Rename `g:clap_disable_run_from_project_root` to `g:clap_disable_run_rooter`.
- `:Clap grep <cword>` is changed to `:Clap grep ++query=<cword>`.
- Rework `g:clap.context` and `g:clap.provider.args`.

### Removed
