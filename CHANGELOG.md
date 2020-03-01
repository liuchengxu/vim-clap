CHANGELOG
=========

## [unreleased]

### Added

- Support multi-byte input for vim's popup thanks to @Bakudankun. You need patch 8.2.0329 to make it work as expected. ([#320](https://github.com/liuchengxu/vim-clap/pull/320))
- Add new option `g:clap_insert_mode_only` to disable the feature of other mode, use the insert mode only. ([#335](https://github.com/liuchengxu/vim-clap/pull/335))
- Add new option `g:clap_providers_relaunch_code`(`@@` default). You can input `@@` or use <kbd>C-L</kbd> to invoke `:Clap` to reselect another provider at any time.([#328](https://github.com/liuchengxu/vim-clap/pull/328))
- Add new keymapping <kbd>C-L</kbd>.([#328](https://github.com/liuchengxu/vim-clap/pull/328))

### Improved

- Now you can use `:Clap grep ++query=@visual` to search the visual selection. ([#336](https://github.com/liuchengxu/vim-clap/pull/336))
- Ensure the long matched elements from the filter always at least partially visible. ([#330](https://github.com/liuchengxu/vim-clap/pull/330))
- Use file name as the preview header for `Clap grep`, `Clap marks` and `Clap jumps`.

## [0.8] 2020-02-21

### Added

- Add new clap theme `let g:clap_theme = 'atom_dark'` by @GoldsteinE.
- Add new provider `:Clap search_history` by @markwu. ([#289](https://github.com/liuchengxu/vim-clap/pull/289))
- Add new provider `:Clap maps` by @markwu. ([#293](https://github.com/liuchengxu/vim-clap/pull/293))
- Add `g:clap_project_root_markers` for specifing how vim-clap intentify a project root. Previously only the git-based project is supported, i.e., `g:clap_project_root_markers = ['.git', '.git/']`. The default value of `g:clap_project_root_markers` is `['.root', '.git', '.git/']` you can add `.root` file under the directory you want to the project root.([#290](https://github.com/liuchengxu/vim-clap/pull/290))
- Add preview support for `yanks`, `buffers`, `files`, `git_files` and `history` provider.
- Add new highlight group `ClapSelectedSign` and `ClapCurrentSelectionSign` for the sign `texthl`, they are linked to `WarningMsg` by default.
-  Add multi-selection support for `:Clap blines`.
- [neovim] normal mappings: j/k, gg/G, `<C-d>`/`<C-u>` and see `ftplugin/clap_input.vim`.

### Improved

- Add `ClapDefaultPreview` for the light theme.
- Open quickfix window at the exact size of entries if there are only a few ones.

### Fixed

- The minimal requred version for neovim is v0.4.2 as v0.4.0 does not work.
- More robust fpath detection for grep preview.[#321](https://github.com/liuchengxu/vim-clap/issues/321)

### Changed

- Add `<nowait>` to neovim's open action mappinngs.
- Change the default icon for `filer` to   .
- Set `&foldcoloumn` to 0 for neovim by default.
- Decrease the default `g:clap_popup_input_delay` from 200ms to 100ms, use the Rust binary.
- Update `clap_tags` syntax due to https://github.com/liuchengxu/vista.vim/pull/231.
- Use a standalone floating win instead of virtual text for the matches count.([#315](https://github.com/liuchengxu/vim-clap/pull/315))
- [neovim] `<Esc>` won't exit clap but enter the normal mode.[#322](https://github.com/liuchengxu/vim-clap/issues/322)

## [0.7] 2020-01-31

### Added

- Add new provider `:Clap filer` for ivy-like file explorer, this also introduces a new type of clap provider: stdio-based RPC provider. ([#272](https://github.com/liuchengxu/vim-clap/pull/272))
- Add new provider `:Clap help_tags` by @markwu. ([#248](https://github.com/liuchengxu/vim-clap/pull/248))
- Add `maple version` to get the detailed maple info and include it in `:Clap debug`.([#262](https://github.com/liuchengxu/vim-clap/pull/262))
- Add `g:clap_forerunner_status_sign` and deprecate `g:clap_forerunner_status_sign_done` and `g:clap_forerunner_status_sign_running`.
- Support skim as the external filter, ref https://github.com/lotabout/skim/issues/225 . ([#269](https://github.com/liuchengxu/vim-clap/pull/269))
- Add a new property `source_type` for non-pure-async provider.([#270](https://github.com/liuchengxu/vim-clap/pull/270))
- Add `g:ClapPrompt` which is Funcref to give more control of the prompt of clap, please see https://github.com/liuchengxu/vim-clap/issues/134#issuecomment-578503522 for the usage.([#265](https://github.com/liuchengxu/vim-clap/pull/265))
- Add `init` property for each provider, which will be invoked when initializing the display window.([#280](https://github.com/liuchengxu/vim-clap/pull/280))

### Internal

- Split out the native VimScript filter implementation in favor of the potential vim9 improvement.([#267](https://github.com/liuchengxu/vim-clap/pull/267))

### Changed

- Use    as the icon of markdown.
- Change the default spinner frames to `['⠋', '⠙', '⠚', '⠞', '⠖', '⠦', '⠴', '⠲', '⠳', '⠓']`.
- Change the default prompt format to `' %spinner%%forerunner_status%%provider_id%:'`.
- Disable `coc_pairs`.

## [0.6] 2020-01-24

### Added

- New provider `:Clap loclist` for listing the entries of current window's location list.([#244](https://github.com/liuchengxu/vim-clap/pull/244))
- New provider `:Clap providers` for listing all the providers by splitting out the previous anonymous `_` provider.([#242](https://github.com/liuchengxu/vim-clap/pull/242))
- Add `g:clap_layout` to control the size and position of clap window. Now the default behaviour has been changed to window relative. If you prefer the previous behaviour, use `let g:clap_layout = { 'relative': 'editor' }`.
- Add multi-select support for `Clap files` and `Clap git_files`.([#258](https://github.com/liuchengxu/vim-clap/pull/258))
- Add `g:clap_theme` for changing the clap theme easily, the theme `material_design_dark` is shipped by default.[#259](https://github.com/liuchengxu/vim-clap/pull/259)

### Changed

- Now `maple` use subcommand instead of option for the various function, this refactor also makes adding new features easier.([#255](https://github.com/liuchengxu/vim-clap/pull/255))

### Improved

- Refine `:Clap debug` and require it in the bug report. ([#241](https://github.com/liuchengxu/vim-clap/pull/241))

### Fixed

- Wrong async threshold in impl.vim.(https://github.com/liuchengxu/vim-clap/pull/248#issuecomment-576108100)

## [0.5] 2020-01-15

### Added

- Add icon support for `history` provider.
- Provide the prebuilt binary support since [Release v0.4](https://github.com/liuchengxu/vim-clap/releases/tag/v0.4).
- Add script for downloading the prebuilt binary easily and support downloading via plugin manager directly.([#222](https://github.com/github.com/liuchengxu/vim-clap/pull/222))
- Push the current position to the jumplist for `blines` provider so that you can jump back using `<C-O>`.([#227](https://github.com/github.com/liuchengxu/vim-clap/pull/2277))
- Add `<PageDown>` and `<PageUp>` keybindings. ([#232](https://github.com/liuchengxu/vim-clap/pull/232))
- Add icon for exact matched file name and more file-extension based icons.([#233](https://github.com/liuchengxu/vim-clap/pull/233))

### Improved

- Make the display window compact when there are too few results for grep provider.

### Fixed

- Do not apply the offset for matched items when using substring filter.
- Git submodule detection.([#175](https://github.com/liuchengxu/vim-clap/pull/175))
- Regression of using neovim job without maple.([#234](https://github.com/liuchengxu/vim-clap/pull/234))

## [0.4] 2020-01-06

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
