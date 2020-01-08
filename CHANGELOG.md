CHANGELOG
=========

## [unreleased]

### Improved

- Make the display window compact when there are too few results for grep provider.

### Fixed

- Do not apply the offset for matched items when using substring filter.

## [0.4] 2019-01-06

### Added

- Add icon for files provider.([#195](https://github.com/liuchengxu/vim-clap/pull/195))
- Add `syntax` property for provider to make enable the syntax highlight easier.([#217](https://github.com/liuchengxu/vim-clap/pull/217))
- Add an option `g:clap_disable_bottom_top`( `0` by default) for disabling wrapping top-to-bottom when pressing ctrl-j/ctrl-k at the bottom/top of the results.
- Add open action support for `:Clap buffers`.
- Add open action support for `:Clap git_files`.
- Add `<C-U>` mapping for clearning the input.

### Improved

- Make the helper function for building the extra Rust tools more friendly and smarter. ([#202](https://github.com/liuchengxu/vim-clap/pull/202))
- Optimize for `Clap blines` provider in case of the buffer has 1 million lines.([#210](https://github.com/liuchengxu/vim-clap/pull/210))

### Fixed

- :tada: Fix the flicker of running asynchronously using `job`.([#185](https://github.com/liuchengxu/vim-clap/issues/185))

## [0.3] 2019-12-30

The major feature of 0.3 is the performance problem has been soloved, see [#140](https://github.com/liuchengxu/vim-clap/issues/140).

### Added

- Add Python version subscorer fuzzy filter.([#159](https://github.com/liuchengxu/vim-clap/pull/159))
- Add Rust version subscorer fuzzy filter.([#176](https://github.com/liuchengxu/vim-clap/pull/176))
- New provider `:Clap quickfix` by @kit494way.
- New provider `:Clap git_diff_files` by @kit494way.
- Add the preview support for `:Clap registers`. If the content of some register is too much to fit on one line, then it will be shown in the preview window, otherwise do nothing.
- Add the preview support for `:Clap tags`.
- Add the helper function for building Rust extension easily. Now you can use `:call clap#helper#build_all()` to build the optional Rust dependency.
- Make the built-in fuzzy filter 10x faster using Rust.([#147](https://github.com/liuchengxu/vim-clap/pull/147))

### Improved

- Cache the result of forerunner job into a temp file if it's larger than the threshold of built-in sync filter can handle.([#177](https://github.com/liuchengxu/vim-clap/pull/177))
- Decrease the overhead of async job significantly.([#181](https://github.com/liuchengxu/vim-clap/pull/181))
- Set `syntax` instead of `filetype` for the highlight as setting `filetype` can start some unexpected filetype related services.

### Fixed

- Fix vim popup sign not showing.([#141](https://github.com/liuchengxu/vim-clap/pull/141))
- Fix performance issue of async job.([#140](https://github.com/liuchengxu/vim-clap/issues/140))
- Fix rff can't work on Windows thanks to @ImmemorConsultrixContrarie.([#180](https://github.com/liuchengxu/vim-clap/pull/180))

## [0.2] 2019-12-10

### Added

- New provider `:Clap registers`.
- New provider `:Clap command`.
- Add a brief description for each provider used in `:Clap`.
- Add syntax for `:Clap jumps`.
- Add the option `g:clap_spinner_frames`.
- Add the option `g:clap_prompt_format`.
- Add the option `g:clap_enable_icon` for configuring the icon functionality globally.
- Add the option `g:clap_popup_cursor_shape` for configuring the mocked cursor shape.
- Add the options `g:clap_fuzzy_match_hl_groups` for configuring the color of fuzzy matched items easier.
- Add an utility function `clap#helper#build_maple()` for building maple easily in vim. Use `:call clap#helper#build_maple()` to install maple inside vim.
- Add the preview support for `:Clap grep`.
- Add the preview support for `:Clap blines`.
- Support running from any specified directory by passing it via the last argument for `:Clap files` and `:Clap grep`.
- Add limited fzf like search syntax([#127](https://github.com/liuchengxu/vim-clap/issues/127)) for `:Clap grep`.([#150](https://github.com/liuchengxu/vim-clap/issues/150))

### Changed

- Put `call g:clap.provider.on_exit()` just before `silent doautocmd <nomodeline> User ClapOnExit` in `clap#_exit()`.

### Improved

- Reverse the original order of `jumps` to make the newer jump appear first.

### Fixed

- sink of `:Clap command_history`.([#109](https://github.com/liuchengxu/vim-clap/issues/109))
- Apply `redraw` when navigating and selecting via tab in vim's popup.
- Fix `bg` of icon highlight([#132](https://github.com/liuchengxu/vim-clap/issues/132))
- Use absolute directory for `g:__clap_provider_cwd` ([#137](https://github.com/liuchengxu/vim-clap/issues/137)).

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
