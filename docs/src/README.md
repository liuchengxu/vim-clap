# Introduction

[![CI](https://github.com/liuchengxu/vim-clap/workflows/ci/badge.svg)](https://github.com/liuchengxu/vim-clap/actions?workflow=ci)

[g1]: https://badges.gitter.im/liuchengxu/vim-clap.svg

Vim-clap stands as a comprehensive and efficient solution, providing powerful fuzzy pickers and replacements for various established Vim plugins, designed to support both Vim and NeoVim.

<p align="center">
  <img width="400px" src="https://user-images.githubusercontent.com/8850248/73323347-24467380-4282-11ea-8dac-5ef5a1ee63bb.gif">
</p>

[More screenshots](https://github.com/liuchengxu/vim-clap/issues/1)

## Features

Vim-clap was initially written in pure VimScript, but later incorporated a Rust dependency to enhance performance. Presently, the Rust binary `maple` is a must-have for ensuring smooth and optimal functionality. The principle of Vim-Clap in this regard is to offload all the heavy computation to the Rust backend and make Vim/NeoVim a super lightweight layer focusing on UI.

- [x] Blazingly fast thanks to the powerful Rust backend
- [x] Consistent command interface with [clap-rs/clap](https://github.com/clap-rs/clap)
- [x] Tons of builtin providers
- [x] Support writing new providers in both Vimscript and Rust
- [x] Support [the search syntax borrowed from fzf](https://github.com/junegunn/fzf#search-syntax) and more

## Caveats

- While Vim-clap is intended to be compatible with Windows, comprehensive testing on this platform has not been conducted to the same extent as macOS and Linux (specifically Ubuntu), as the plugin author primarily utilizes these operating systems. Consequently, there may be Windows-specific issues yet to be identified. If you encounter any problems on Windows, please [create an issue](https://github.com/liuchengxu/vim-clap/issues/new?assignees=&labels=&template=bug_report.md&title=), and any assistance in addressing these issues would be highly appreciated.

- While Vim-Clap strives to offer equal support for both Vim and NeoVim, certain nuances arise from the differing implementation details between the two. For example, the focusability of Vim's `popup` differs from NeoVim's `floating_win`.

## Credit

- Vim-clap is initially enlightened by [snails](https://github.com/manateelazycat/snails).
- Some providers' idea and code are borrowed from [fzf.vim](https://github.com/junegunn/fzf.vim).
- The built-in fzy python implementation is based on [sweep.py](https://github.com/aslpavel/sweep.py).
