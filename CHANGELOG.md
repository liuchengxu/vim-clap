CHANGELOG
=========

## [unreleased]

### Added

- New provider `:Clap lines`.
- Add the substring filter mode.
- New external filter `fzy` and `fzf`. Every provider could run async if you have one of the external filters installed.
- Add the preview support for `:Clap marks`.
- Add the option `g:clap_provider_grep_enable_icon` for disabling the icon drawing in `:Clap grep`.
- Support opening the selected file via <kbd>ctrl-t</kbd>, <kbd>ctrl-x</kbd>, <kbd>ctrl-v</kbd> if the provider supports, and add `g:clap_open_action` for configuring the default keybindings.
- Support opening files from the git base directory. See `:h g:clap_disable_run_rooter` if you don't like this behavior.

### Improved

- Do not try using the default async filter implementation if none of the external filters are avaliable.([#61](https://github.com/liuchengxu/vim-clap/issues/61))

### Changed

- Rename `g:clap_selected_sign_definition` to `g:clap_selected_sign`.
- Rename `g:clap_current_selection_sign_definition` to `g:clap_current_selection_sign`.
- Rename `g:clap_disable_run_from_project_root` to `g:clap_disable_run_rooter`.

### Removed
