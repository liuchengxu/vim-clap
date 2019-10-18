CHANGELOG
=========

## [unreleased]

### Added

- New provider `:Clap lines`.
- Add the preview support for `:Clap marks`.
- Add the option `g:clap_provider_grep_enable_icon` for disabling the icon drawing in `:Clap grep`.

### Improved

- Do not try using the default async filter implementation if none of the external filters are avaliable.([#61](https://github.com/liuchengxu/vim-clap/issues/61))

### Changed

- Rename `g:clap_selected_sign_definition` to `g:clap_selected_sign`.
- Rename `g:clap_current_selection_sign_definition` to `g:clap_current_selection_sign`.
- Rename `g:clap_disable_run_from_project_root` to `g:clap_disable_run_rooter`.

### Removed
