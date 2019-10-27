CHANGELOG
=========

## [unreleased]

## [0.1] 2019-10-27

### Added

#### Provider

- New provider `:Clap lines`.
- New provider `:Clap history`.
- New provider `:Clap yanks` thanks to @ratheesh.
- Support `:Clap` listing all the builtin providers, thanks to @wookayin implementing the sink of it.
- Add the preview support for `:Clap marks` and `:Clap jumps`.

#### Provider source

- Rework the `buffers` provider source to make it look more fancy.([#71](https://github.com/liuchengxu/vim-clap/issues/71))

#### Filter

- New built-in fzy finder implemented in python.([#92](https://github.com/liuchengxu/vim-clap/pull/92))
- New external filter `fzy` and `fzf`. Every provider could run async if you have one of the external filters installed.
- Add the substring filter mode.

#### Global options

- Support opening the selected file via <kbd>ctrl-t</kbd>, <kbd>ctrl-x</kbd>, <kbd>ctrl-v</kbd> if the provider supports, and add `g:clap_open_action` for configuring the default keybindings.
- Support opening files from the git base directory. See `:h g:clap_disable_run_rooter` if you don't like this behavior.
- Add search box border symbols support, see `:h g:clap_search_box_border_style`.([#85](https://github.com/liuchengxu/vim-clap/pull/85))
- Add the option `g:clap_provider_grep_enable_icon` for disabling the icon drawing in `:Clap grep`.
- Add `g:clap_provider_grep_opts` for globally configuring the used command line options of rg, thanks to @Olical.
- Support searching the hidden files via `:Clap files --hidden`.
- Support using any other finder tools for the files provider via `:Clap files ++finder=[YOUR FINDER] [FINDER ARGS]`.

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

## [First being published] 2019-09-28
