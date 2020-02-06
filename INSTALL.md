# Install the extra dependency for vim-clap

<!-- TOC GFM -->

* [Introduction](#introduction)
* [Build the dependency locally](#build-the-dependency-locally)
  * [`python`](#python)
  * [`Rust`](#rust)
    * [`maple` binary](#maple-binary)
    * [Python dynamic module](#python-dynamic-module)
* [Download the prebuilt binary](#download-the-prebuilt-binary)
  * [`maple`](#maple)

<!-- /TOC -->

## Introduction

vim-clap can work without any other extra dependencies. However, there are some unavoidable performance issues for some providers, see the details at [#140](https://github.com/liuchengxu/vim-clap/issues/140), for you can never expect a Vim plugin written in pure VimL to be fast everywhere even vim9 can make the VimL faster significantly.

There are two optional dependencies for boosting the performance of vim-clap:

1. `maple` binary.
2. Python dynamic module.

Now, only `maple` binary is mandatory for getting a fast and quite responsive vim-clap. If you do not have the `+python` support, that's no problem.

## Build the dependency locally

### `python`

  If you want to use the advanced built-in fuzzy match filter which uses the [fzy algorithm](https://github.com/jhawthorn/fzy/blob/master/ALGORITHM.md) implemented in python, then the `python` support is required:

- Vim: `:pyx print("Hello")` should be `Hello`.
- NeoVim:

  ```bash
  # ensure you have installed pynvim
  $ python3 -m pip install pynvim
  ```

### `Rust`

If you have installed Rust on your system, specifically, `cargo` executable exists, you can build the extra tools for a performant and nicer vim-clap using this single command `:call clap#helper#build_all()`.

#### `maple` binary

`maple` mainly serves two functions:

1. Expose the fuzzy matched indices so that the matched elements can be highlighted in vim-clap, being a tiny wrapper of external fuzzy filter [fzf](https://github.com/junegunn/fzf) and [fzy](https://github.com/jhawthorn/fzy). Once you installed `maple`, fzy/skim binary are unneeded as `maple` does not rely the binary directly but reuses their filter algorithm internally.

2. Reduce the overhead of async job of Vim/NeoVim dramastically.

To install `maple` you can use the helper function and run `:call clap#helper#build_maple()`, or install it manually:

  ```bash
  cd path/to/vim-clap

  # Compile the release build
  cargo build --release

  # Or use cargo install globally
  cargo install --path . --force
  ```

#### Python dynamic module

If you don't have `+python`, you can safely skip this section, it's totally fine, vim-clap can still work very well with only `maple` binary installed. This Python dynamic module is mainly for saving the async job when the data set is small.

[Python dynamic module](https://github.com/liuchengxu/vim-clap#python-dynamic-module) needs to be compiled using Rust nightly, ensure you have installed it if you want to run the helper function successfully:

```bash
$ rustup toolchain install nightly
```

Then use `:call clap#helper#build_python_dynamic_module()` to install the Python dynamic module written in Rust for 10x faster fuzzy filter than the Python one. Refer to the post [Make Vim Python plugin 10x faster using Rust](http://liuchengxu.org/posts/speed-up-vim-python-plugin-using-rust/) for the whole story.

## Download the prebuilt binary

You can call `:call clap#installer#download_binary()` in Vim/NeoVim, or do it manually as follows.

### `maple`

1. Download the binary from the latest release https://github.com/liuchengxu/vim-clap/releases/ according to your system.
2. Rename the downloaded binary to:
    - Unix: `maple`
    - Windows: `maple.exe`
3. Move `maple`/`maple.exe` to `bin` directory. Don't forget to assign execute permission to `maple` via `chmod a+x bin/maple` if you are using a Unix system.
