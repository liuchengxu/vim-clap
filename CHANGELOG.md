# CHANGELOG

## [unreleased]

- [plugin] Change the syntax of plugin actions from `plugin/foo-action` to `plugin.fooAction` for the compatibility with tools like coc.nvim.
- [plugin] Use different highlight groups for error and warn diagnostics.
- [plugin] Added diagnostics plugin in order to conveniently inspect the collected diagnostics from both the linter and lsp plugin.
  You now should use `:ClapAction diagnostics.firstError` instead of `:ClapAction linter.firstError` to jump to the position of first error.
- Various fixes and improvements.

### Internal

- Improve the robustness of the publish pipeline by migrating Bash and Python scripts to `cargo xtask`.

## [0.51] 2024-02-18

## Added

- Input history of providers are now persistent.
- Added experimental winbar support (neovim-only).
```
[winbar]
enable = true
```
- Added project-specific ignore configs for more providers. You can use

```toml
# Ignore the results from the certain files/folders.
# For example, ignore the test files when searching in the folder ~/src/github.com/bitcoin/bitcoin.
[provider.project-ignores."~/src/github.com/bitcoin/bitcoin"]
ignore-file-path-pattern = ["test"]
ignore-file-name-pattern = ["test"]
```

## Fixed

- Make the behaviour on empty query consistent across the providers.

## [0.50] 2024-01-02

## [0.49] 2023-11-19

- Added `quick_pick` to provider, which is suitable for the providers like `:Clap clap_actions` without a preview.
- Refine the tree-sitter highlighting for Rust.
- Various Fixes

## [0.48] 2023-11-18

### Added

The highlight of this release is the integration of tree-sitter highlighting, use `:ClapAction syntax/tree-sitter-highlight` to have a try.

## [0.47] 2023-10-28

## [0.46] 2023-08-29

## [0.45] 2023-07-01

### Removed

- Remove a bunch of deprecated flags: `g:clap_maple_delay`, `g:clap_dispatcher_drop_cache`, `g:clap_default_external_filter`, `g:clap_builtin_fuzzy_filter_threshold`, `g:clap_cache_threshold`, `g:clap_force_matchfuzzy`, `g:clap_force_python`. They are unused now and I believe most of them are hardly really used by users.

### Changed

- `++opt` and `+opt` have been replaced with `--opt value`/`--opt=value` and `--opt` in a consistent way. Ref to #981 for upgrade guide.

## [0.44] 2023-05-27

## [0.43] 2023-04-16

## [0.42] 2023-03-11

## [0.41] 2023-02-10

## [0.40] 2023-01-27

## [0.39] 2023-01-13

## [0.38] 2023-01-08

### Added

- Build executables for more platforms. #901
- Rework the bridge between the Rust backend and Vim, some commonly used providers such as `grep`, `files`, `blines` are reimplemented to be significantly faster and reponsive without using any caching tricks. #872

## [0.37] 2022-10-16

### Changed

- Rename the provider `grep` and `grep2`. `:Clap grep` becomes `:Clap live_grep`, `:Clap grep2` becomes `:Clap grep`. If you have some grep variables like `g:clap_provider_grep_foo` before, now you need to rename them to `g:clap_provider_live_grep_foo`. #879

### Fixed

- Fix the filer preview on backend. #863
- Make the fallback smooth if there is an error occurred while loading the Python dynamic module. #865

## [0.36] 2022-08-06

### Improved

- Support filtering in parallel with the dynamic progress update. The filtering time is reduced about 50% from the sequential to the parallel version.
  The following providers will be benefited from the parallel filtering:
  - `source` of the provider returns a String command.
  - `blines`, `grep2`, `proj_tags`.
- Speed up the cache creation signigicantly. #858

### Changed

- The spinner will be hidden if it's idle.
- `dumb_jump`: ignore the results from the files not trcked by git if the project is a git repo.

### Fixed

- Maple self-upgrade is broken. #847, #848
- Fix the language bonus matching of `blines` provider.

## [0.35] 2022-06-12

### Changed

- The default value of `g:clap_enable_background_shadow` is now `v:false` by default due to the side effect like #836.

### Fixed

- Fix the incompatiblity issue between vim-signature and vim-clap popup window. #817
- Escape the file name for filer sink. #822
- Seperate the vista impl for tags provider completely. #827
- Fix the regression of command `ripgrep-forerunner`.

### Improved

- Strip the current working directory prefix from the entry of `recent_files` provider, all the entries which are not in cwd still shows the absolute path. #834

## [0.34] 2022-03-25

### Added

- Add `g:clap_cache_threshold`. #806
- Add `+ignorecase` to enable case-insensitive search, e.g., `:Clap files +ignorecase`. #814

### Improved

- Tweak the final matching score using the language keyword matching. #808

## [0.33] 2022-02-20

### Added

- Support generating the source of `tags` provider using the Rust binary, remove the vista.vim dep from `tags` provider. #795
- Initial support of preview with context. #798

### Fixed

- Fix the `proj_tags` cmd list under Vim. #796
- No syntax highlight for `c` preview. #800

### Improved

- Better performance for the pre-built binary. #804

### Changed

- Set `g:clap_builtin_fuzzy_filter_threshold` to `0` to always use the async `on_typed` implementation which is full-featured using the Rust backend.

### Internal

- Update to clap v3.0. #794

## [0.32] 2022-01-21

### Improved

- Rework the truncation of long lines. #788
- Support searching the definition/declaration in the `tags` file using `readtags` for `dumb_jump` provider. #789

  Aside from the previous regex searching, the results from the tags searching will be displayed first. You can control the
  tags searching scheme by adding `*` in the end:

  - `hel`: match the tags that starts with `hel`.
  - `hel*`: match the tags that contain `hel`.

- Add `gtags` support for dumb_jump provider. #792
- Introduce debounce for user typed event. #793

### Fixed

- Fix the regression that `filer` provider is not properly initialized on the Rust backend. #790

## [0.31] 2021-12-12

### Improved

- Always update the preview for `registers` otherwise the preview content could be outdated and add a preview title.

### Fixed

- Fix static binary build on ubuntu after upgrading to edition 2021(#785)

## [0.30] 2021-10-19

### Improved

- Improve the overal performance by using rayon. #754
- Parallel recursive ctags creation, 30x faster on my machine. #755
- Support expanding `~` in file path when using `preview/file`.
- Add `'AUTO'` option for `g:clap_preview_direction`. #767 @goolord

### Fixed

- Error when using `clap#preview#file()` with `g:clap_preview_direction = 'UD'`. #756
- `maps` provider: missing keybindings for neovim. #762 @ray-x
- `.cc` file preview not highlighted. #736
- Invoke `on_move` as well when initializing the display window. #768

## [0.29] 2021-08-30

### Added

- Support fzf-like search syntax. #738

### Improved

- Make the preview on enter work for `recent_files` provider. #731
- Refresh the cache when it might be outdated, detected on processing the OnMove event. #740
- Add a bonus score for the entry of `recent_files` based on cwd. #742

### Fixed

- Fix the preview of `filer` provider. #731
- Fix the installer on FreeBSD/OpenBSD. #733 Thanks to @spamwax

## [0.28] 2021-08-06

### Improved

- Rewrite the cache system to be more efficient and convenient. See #726 for details.
- Less allocations. #728

## [0.27] 2021-07-22

### Added

- Add a new provider `recent_files` for recent files history, which is persistent and can keep up to 10,000 entries ordered by [Frecency](https://en.wikipedia.org/wiki/Frecency). #724

### Fixed

- Fix rg 13.0.0 does not work for neovim. #711
- Fix grep2 does not work on Windows. #533

### Improved

- Support passing the cursor position instead of full cursor line from Vim to Rust since the performance of Vim is pretty bad when the cursor line is extremely long. #719

## [0.26] 2021-06-15

### Added

- [neovim] Add zindex option to fix the tricky floating_win overlapping, and add border for the preview window, use `let g:clap_popup_border = 'nil'` to disable the order. #693
- Impl preview for `quickfix` provider. #691
- Impl `preview/file` for easier external async preview integration. #706

### Changed

- Now `g:clap_provider_grep_enable_icon` is initialized using `g:clap_enable_icon`. #701

### Fixed

- Handle the non-utf8 line of rg's output properly. #673
- [neovim] Fix the action dialog creation using floating_win. #688
- Fix the indicator winwidth is not flexible. #687
- Fix the icon offset when restoring the full display line for grep provider. #701
- Fix the Pyo3 compilation on M1. #707
- Add icon for `*.tex`. #709

### Perf

- Use faster simdutf8. #681

## [0.25] 2021-04-25

### Added

- Add `dumb_jump` provider, which will fall back to the normal grep way when the regexp approach fails. #659

### Internal change

- Move `stdio_server` crate into a module of `maple_cli` crate for reusing the utilities in `maple_cli` easily.

### Fixed

- Force using sync impl for the providers's `source_type` that is list type. #672

## [0.24] 2021-03-13

### Added

- Add user autocmd `ClapOnInitialize`, can be used to ignore some buffers when opening clap. #653
- Add `g:clap_provider_colors_ignore_default` to ignore the default colors in `VIMRUNTIME`. #632
- Support neovim floating_win based action menu. #655

### Improved

- Truncate the lines of `grep` provider. #650
- Support unordered substring query. #652
- Add `hi default link ClapIndicator ClapInput` for the default theme.

### Fixed

- Cannot open files with pipe in file path. #643
- Fix the grep preview when `g:clap_enable_icon` is enabled and `g:clap_provider_grep_enable_icon` is disabled. #648
- Reset the old selections when the input changes. #646
- Make customize the icon easier. #392

### Changed

- Remove some colors close the white color in the default value of `g:clap_fuzzy_match_hl_groups`.

## [0.23] 2021-02-16

### Added

- Add `g:clap_force_matchfuzzy` to use the builtin `matchfuzzy()` when filtering in sync way. #607
- Add `g:clap_force_python` to always use the Python sync filter as some improvements are only implemented on the Rust side and you need the Python dynamic module to use that. #614
- Support `+name-only` for Lua sync filter. #612
- Add `g:ClapProviderHistoryCustomFilter` for customizing the source of `history` provider. #615
- Add a bonus for the match in the filename when the source item is a path, but you can only have this when you are using Python dynamic module or the Rust backend. #614.
- Add a bonus for the files you opened since you enter vim. #622
- Add async preview support for `help_tags` provider, the Rust binary is required. #630
- ~~Add `g:clap_always_open_preview` to open the preview always if the provider impls `on_move_async`~~(See `g:clap_open_preview`), it's on by the default which changes the behavior before. #625
- Add `g:clap_preview_direction` for opening the preview window on the right of the display window, and the default behavior has been changed to `LR` if your screen's `columns` is less than 80. #634
- Add `g:clap_open_preview` to control the opening of preview window, you can set it to `never` to fully disable the preview feature. #636

### Fixed

- Add `--color=never` to the default grep option. #609
- Show create new file entry when in empty directory. #624

### Internal

- Introduce `MatchText` for passing more match context easier later. #626

## [0.22] 2021-01-01

### Added

- Add `g:clap_enable_background_shadow` to render a transparent shadow (neovim-only) #546, #550
- Add `g:clap_popup_move_manager` so that Vim users can override the default mappings easily. #536
- Allow user to always download the prebuilt binary. #531
- Support smartcase fitlering for fzy algo and it's the default behavior. #541 @romgrk
- Add initial support for fzy lua, neovim-nightly or vim compiled with lua is required. #599
- Add the providers defined via global variable into the `providers` provider, which means you can see the global variable type providers when you call `:Clap` now. But you have to define `description` explicitly otherwise they won't be found. #605
  ```vim
  let g:clap_provider_tasks = {
            \ 'source': function('TaskListSource'),
            \ 'sink': function('TaskListSink'),
            \ 'description': 'List various tasks',
            \ }
  ```

### Improved

- Command provider has a better rendering and let's the user add arguments #570
- Fix the sluggish of vim when the preview lines are awfully long. #543

### Fixed

- Fix the installer on Windows. #529 @Grueslayer
- Fix the condition of vim8 job exists or not. #566
- Add the missing `ClapOnExit` for `g:clap_open_action` operation. #576

### Improved

- Keybindings for `filer`: `<CR>` now expands directory instead of editing it
- Make the grep opts work as normal in the command line. #595

## [0.21] 2020-09-27

### Added

- New shortcut for `+no-cache`, `:Clap files!` is equivalent to `:Clap!! files` and `:Clap files +no-cache`. ([#509](https://github.com/liuchengxu/vim-clap/pull/509))
- Add `g:clap_enable_debug`, useful when you find vim-clap is problematic and want to debug vim-clap.

### Improved

- The open action `ctrl-t`, `ctrl-v`, `ctrl-t` now supports the multiple files. ([#496](https://github.com/liuchengxu/vim-clap/issues/496))
- Check if the ctags has the JSON output feature. ([#491](https://github.com/liuchengxu/vim-clap/issues/491))

### Fixed

- Fix `:Clap install-binary` does not work correctly on Windows. ([#494](https://github.com/liuchengxu/vim-clap/pull/494)) @Bakudankun
- Fix [#306](https://github.com/liuchengxu/vim-clap/issues/306), note the signature of `bs_action` are different between vim and neovim now. ([#503](https://github.com/liuchengxu/vim-clap/pull/503))
- Fix filer issue on Windows [#370](https://github.com/liuchengxu/vim-clap/issues/370). @Grueslayer
- Handle the maple error in the filer provider, fix [#500](https://github.com/liuchengxu/vim-clap/issues/500), [#501](https://github.com/liuchengxu/vim-clap/issues/501).
- Fix regression #513
- Fix #515
- Fix #517
- Fix #526

## [0.20] 2020-08-06

### Added

- Python dynamic module now can be compiled using stable Rust. ([#471](https://github.com/liuchengxu/vim-clap/pull/471))
- Add `windows` preview support. ([#473](https://github.com/liuchengxu/vim-clap/pull/473))
- Impl `commits` and `bcommits` provider. ([#477](https://github.com/liuchengxu/vim-clap/pull/477)) @ray-x
- Add new provider property `on_move_async`. ([#481](https://github.com/liuchengxu/vim-clap/pull/481))
- Support expanding `%` now, e.g., `:Clap files %:p:h`.
- Build static Rust binary for Linux. [#469](https://github.com/liuchengxu/vim-clap/issues/469)

### Fixed

- Fix `history` provider `open_action` support. ([#474](https://github.com/liuchengxu/vim-clap/pull/474))

### Changed

- Remove `noautocmd` when closing neovim's floating win for clap. [#472](https://github.com/liuchengxu/vim-clap/issues/472)

## [0.19] 2020-06-28

### Added

- Add `clap#run(provider)` which is similar to `fzf#run()`. The argument `provider` is a Dict like `g:clap_provider_foo` with an optional extra field specifying the provider id. It can used for adhoc running, don't use it with a `source` that probably has a fair mount of items as it's normally undeveloped in performance. [#433](https://github.com/liuchengxu/vim-clap/issues/433)
- Impl async preview for `git_files` and `history` provider.

### Improved

- Make the indicator winwidth a bit adpative when using the `relative` layout.
- Ensure the sign always visiable when running maple via job.

### Fixed

- Fixed the win contexted `execute()` for `jumps` and `marks` provider when clap window is not yet visible.

## [0.18] 2020-06-09

### Improved

- Try loading the clap theme having a same name with the current colorscheme when `g:clap_theme` does not exist.

### Added

- Implement async preview for `blines`, `tags` and `proj_tags` provider. ([#457](https://github.com/liuchengxu/vim-clap/pull/457))
- Add icon support for `proj_tags` provider. ([#461](https://github.com/liuchengxu/vim-clap/pull/461))
- Add `g:clap_preview_size` for configuring the number of preview lines. ([#444](https://github.com/liuchengxu/vim-clap/pull/444))
- Add `g:clap_provider_buffers_cur_tab_only`. ([#439](https://github.com/liuchengxu/vim-clap/pull/439))

### Fixed

- Fix the the command of `job_start` with vanila vim. [#449](https://github.com/liuchengxu/vim-clap/issues/449)
- Implement the `VimResized` hook. [#454](https://github.com/liuchengxu/vim-clap/issues/454)

## [0.17] 2020-05-25

### Fixed

- Fix the `sink*` args in `selection.vim`, convert the truncated lines to the original full lines.

## [0.16] 2020-05-21

### Added

- Add `g:clap_provider_yanks_history`. ([#438](https://github.com/liuchengxu/vim-clap/pull/438))
- Async `on_move` impl for `filer`, `files`, `grep` and `grep2` provider in Rust binary, no delay for the preview function. ([#437](https://github.com/liuchengxu/vim-clap/pull/437))

### Changed

- Decrease the max number of candidates for running in sync from 100000 to 30000, which means once the total number of candidates is larger than 30000, the async filter will be used, otherwise use the builtin sync one.
- `filer` uses the daemon job which requires the latest binary. Download the latest binary if you uses the prebuilt binary.

### Improved

- Add cmdline completion for all the autoloaded providers. [#429](https://github.com/liuchengxu/vim-clap/issues/429)
- Run the spinner for dyn filter. [#430](https://github.com/liuchengxu/vim-clap/issues/430)

## [0.15] 2020-05-02

### Added

- Support substring matcher for dyn filter, used when the query contains space. ([#411](https://github.com/liuchengxu/vim-clap/pull/411))
- Add progress bar support for the download feature of maple. ([#419](https://github.com/liuchengxu/vim-clap/pull/419))
- Add instructions for building the Rust binary via Docker in case of some users run into the libssl error when using the prebuilt binary, see more info in [INSTALL.md](./INSTALL.md).

### Fixed

- Reset handler state. (#418)

## [0.14] 2020-04-25

### Added

- Add new `check-release` command, you can use `maple check-release --download` to download the latest release binary to `bin` directory. And `:Clap install-binary!` will run this command when possible. ([#410](https://github.com/liuchengxu/vim-clap/pull/410))

### Fixed

- When cmd in `job(cmd)` is a String, the path containing spaces could be problematic on Windows(GVim). Use List instead. ([#407](https://github.com/liuchengxu/vim-clap/pull/407))
- The positions of matched items in Rust fzy implementation `extracted_fzy` crate is incorrect. The pure Python fzy impl is consistent with the original fzy C implementation. ([#409](https://github.com/liuchengxu/vim-clap/pull/409))

## [0.13] 2020-04-20

### Added

- New provider `:Clap proj_tags` for project-wide tags.([#391](https://github.com/liuchengxu/vim-clap/pull/391))
- Allow `:Clap files +name-only` to filter the file name only instead of the full file path. Require you have built the Python dynamic module or uses in the cached mode. ([#389](https://github.com/liuchengxu/vim-clap/pull/389))
- Add provider `action` property, you can delete the buffer in `:Clap buffers` using the action dialog triggered by `<S-Tab>`. ([#396](https://github.com/liuchengxu/vim-clap/pull/396))

### Improved

- List all the autoloaded providers instead of the builtin ones in `:Clap providers`.
- Handle the icon highlight offset on Python and Rust side.

### Changed

- Now `:Clap tags` will filter the tag name column only, same with `:Clap proj_tags`.
- Change truncated dots from `...` to `..` for displaying one more useful char.

### Fixed

- Fix installer on Windows and some other job related issues. Thanks to @TissueFluid. ([#405](https://github.com/liuchengxu/vim-clap/pull/405))
- Add default value when `ClapSearchText` highlight group misses some attributes. #390
- The final result of dyn filter is not ordered, ref https://github.com/liuchengxu/vim-clap/pull/385#issuecomment-611616792 .
- Make use of command line `--winwidth` option, fix the unsuitable truncation for long matched lines.

## [0.12] 2020-04-12

### Added

- Add `--content-filtering` in maple. You can use `:Clap files +name-only ~` to filter the file name instead of full file path, but you can only use it when clap is using the cached tempfile inside vim.

### Improved

- icon highlight for truncated grep text.

### Changed

- `grep2` will not match the file path by default. ([#385](https://github.com/liuchengxu/vim-clap/pull/385))

### Fixed

- `ITEMS_TO_SHOW` is fixed at the moment, only 30 rows can be shown correctly for dyn filter. https://github.com/liuchengxu/vim-clap/pull/385#issuecomment-611601076
- Fix wrong icon for dyn filter when the text is truncated.

## [0.11] 2020-04-09

### Added

- New provider `:Clap grep2` with cache and dynamic refresh support. `grep2` is much faster than the previous `grep` provider as it'll reuse the cached contents from previous run and do the filtering with dynamic results. `grep2` is not a typical grep tool but a fuzzy filter tool, for it tries to collect all the output and then filtering on the results. `grep` is merely to dispatch the rg command and show the results returned by rg directly, no fuzzy filter actually. ([#383](https://github.com/liuchengxu/vim-clap/pull/383))

- Double bang version of `:Clap!!`, shortcut for `:Clap [provider_id_or_alias] +no-cache`, e.g., `:Clap!! files ~` is same to `:Clap files +no-cache ~`.

### Changed

- Change `ITEMS_TO_SHOW` from `100` to 30, `UPDATE_INTERVAL` from 200ms to 300ms. A normal screen can only show about 50 rows, 30 rows should look like the same to 100 rows as the default clap window size is 1/3 of the screen height, but it reduces the overhead of communication between vim and maple significantly.
- Add `using_cache` status to `g:clap_forerunner_status_sign`, the default sign is `*`, which indicates clap is using the cached file which could be outdated. Use `+no-cache` to run without cache and also rebuild the cache accordingly, e.g., `:Clap files +no-cache /`.
- The cache directory name changed to `vim.clap` from `clap_cache` in your system `temp_dir`.

### Improved

- [perf]Try using the cached file when rerunning the same command under the same directory. The cache directory uses https://doc.rust-lang.org/std/env/fn.temp_dir.html which will be purged when you restart the computer. Or you can use `maple cache --list` to list the current cached info.

### Fixed

- `has('gui_running')` does not work for neovim. [#378](https://github.com/liuchengxu/vim-clap/issues/378)
- Wrong Vim job stop API usage.([#377](https://github.com/liuchengxu/vim-clap/pull/377))
- https://github.com/liuchengxu/vim-clap/issues/371#issuecomment-610176970
- The postponed preview action can be triggered when the main window is closed. #382

## [0.10] 2020-04-04

### Added

- Add `init` for `Clap grep`, fill the content when query is empty for git repo.([#347](https://github.com/liuchengxu/vim-clap/pull/347))
- Add `g:clap_popup_border` for adding the border for the preview popup. ([#349](https://github.com/liuchengxu/vim-clap/pull/349))

### Improved

- Print a note about Rust nightly is requred for building the Python dynamic module.
- Refine the syntax of `Clap lines` with `ClapLinesBufname` and `ClapLinesNumber` group added.
- [perf] Use const table instead of `lazy_static` for the icons, [more info](https://github.com/liuchengxu/vim-clap/pull/354#discussion_r395975392). Thanks to @ImmemorConsultrixContrarie .
- [perf] Major improvement :tada: support the filter dynamic support, contribution by @ImmemorConsultrixContrarie. ([#364](https://github.com/liuchengxu/vim-clap/pull/364))

### Fixed

- `Clap filer` always selects the first entry when you narrow down and navigate the list. ([#348](https://github.com/liuchengxu/vim-clap/issues/348))

## [0.9] 2020-03-10

### Added

- Support multi-byte input for vim's popup thanks to @Bakudankun. You need patch 8.2.0329 to make it work as expected. ([#320](https://github.com/liuchengxu/vim-clap/pull/320))
- Add new option `g:clap_insert_mode_only` to disable the feature of other mode, use the insert mode only. ([#335](https://github.com/liuchengxu/vim-clap/pull/335))
- Add new option `g:clap_providers_relaunch_code`(`@@` default). You can input `@@` or use <kbd>C-L</kbd> to invoke `:Clap` to reselect another provider at any time.([#328](https://github.com/liuchengxu/vim-clap/pull/328))
- Add new keymapping <kbd>C-L</kbd>.([#328](https://github.com/liuchengxu/vim-clap/pull/328))
- Add preview support for `Clap filer`.
- Add `blines` subcommand in maple for always prepending the line number even there are 1M+ lines.

### Improved

- Now you can use `:Clap grep ++query=@visual` to search the visual selection. ([#336](https://github.com/liuchengxu/vim-clap/pull/336))
- Ensure the long matched elements from the filter always at least partially visible. ([#330](https://github.com/liuchengxu/vim-clap/pull/330))
- Use file name as the preview header for `Clap grep`, `Clap blines`, `Clap tags`, `Clap marks` and `Clap jumps`.
- Make `<Del>` work in vim's popup.

### Changed

- Change the default value of `g:clap_popup_cursor_shape` from `'|'` to `''` for using the new block-style cursor in vim's popup By @Bakudankun. ([#340](https://github.com/liuchengxu/vim-clap/pull/340))

## [0.8] 2020-02-21

### Added

- Add new clap theme `let g:clap_theme = 'atom_dark'` by @GoldsteinE.
- Add new provider `:Clap search_history` by @markwu. ([#289](https://github.com/liuchengxu/vim-clap/pull/289))
- Add new provider `:Clap maps` by @markwu. ([#293](https://github.com/liuchengxu/vim-clap/pull/293))
- Add `g:clap_project_root_markers` for specifing how vim-clap intentify a project root. Previously only the git-based project is supported, i.e., `g:clap_project_root_markers = ['.git', '.git/']`. The default value of `g:clap_project_root_markers` is `['.root', '.git', '.git/']` you can add `.root` file under the directory you want to the project root.([#290](https://github.com/liuchengxu/vim-clap/pull/290))
- Add preview support for `yanks`, `buffers`, `files`, `git_files` and `history` provider.
- Add new highlight group `ClapSelectedSign` and `ClapCurrentSelectionSign` for the sign `texthl`, they are linked to `WarningMsg` by default.
- Add multi-selection support for `:Clap blines`.
- [neovim] normal mappings: j/k, gg/G, `<C-d>`/`<C-u>` and see `ftplugin/clap_input.vim`.

### Improved

- Add `ClapDefaultPreview` for the light theme.
- Open quickfix window at the exact size of entries if there are only a few ones.

### Fixed

- The minimal requred version for neovim is v0.4.2 as v0.4.0 does not work.
- More robust fpath detection for grep preview.[#321](https://github.com/liuchengxu/vim-clap/issues/321)

### Changed

- Add `<nowait>` to neovim's open action mappinngs.
- Change the default icon for `filer` to  .
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

- Use  as the icon of markdown.
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
