# Installation

## Requirement

- Vim: `:echo has('patch-8.1.2114')`.
- NeoVim: `:echo has('nvim-0.4.2')`.

## Installation

### [vim-plug](https://github.com/junegunn/vim-plug)

```vim
" Build the Rust binary if `cargo` exists on your system.
Plug 'liuchengxu/vim-clap', { 'do': ':Clap install-binary' }

" The bang version will try to download the prebuilt binary if `cargo` does not exist.
Plug 'liuchengxu/vim-clap', { 'do': ':Clap install-binary!' }

" `:Clap install-binary[!]` will always try to compile the binary locally.
" If you do care about the disk used for the compilation, use the way of force download,
" which will directly download the prebuilt binary even if `cargo` is available.
Plug 'liuchengxu/vim-clap', { 'do': { -> clap#installer#force_download() } }

" `:Clap install-binary[!]` will run using the terminal feature which is inherently async.
" If you don't want that and hope to run the hook synchorously:
Plug 'liuchengxu/vim-clap', { 'do': has('win32') ? 'cargo build --release' : 'make' }
```

Employing the `do` hook of the Vim plugin manager typically facilitates the automatic installation of the additional Rust binary, offering a convenient and recommended solution. However, if this process encounters any issues, manual compilation of the Rust dependency is required, as outlined in [the subsequent section](./install_rust.md).
