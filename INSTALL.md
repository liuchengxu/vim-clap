# Install the extra dependency for vim-clap

<!-- TOC GFM -->

* [Introduction](#introduction)
* [Build the dependency locally](#build-the-dependency-locally)
  * [`python`(deprecated)](#pythondeprecated)
  * [`Rust`](#rust)
    * [`maple` binary](#maple-binary)
    * [Python dynamic module(deprecated)](#python-dynamic-moduledeprecated)
* [Download the prebuilt binary from GitHub release](#download-the-prebuilt-binary-from-github-release)
  * [Quick installer](#quick-installer)
    * [Unix](#unix)
    * [Windows](#windows)
  * [Manual](#manual)
* [Build the Rust binary via Docker](#build-the-rust-binary-via-docker)
  * [Linux](#linux)

<!-- /TOC -->

## Introduction

vim-clap can work without any other extra dependencies in theory. However, there are some unavoidable performance issues for some providers, see the details at [#140](https://github.com/liuchengxu/vim-clap/issues/140), for you can never expect a Vim plugin written in pure VimL to be fast everywhere even vim9 can make the VimL faster significantly. Pin to some ancient version of vim-clap if you do want one implemented in pure VimL.

Now, only `maple` binary is mandatory for getting a fast and quite responsive vim-clap. The `+python` feature and Python dynamic module have been totally retired.

## Build the dependency locally

### `python`(deprecated)

<details>
  <summary>`python` dependency is totally unneeded since v0.37</summary>

If you want to use the advanced built-in fuzzy match filter which uses the [fzy algorithm](https://github.com/jhawthorn/fzy/blob/master/ALGORITHM.md) implemented in python, then the `python` support is required:

- Vim: `:pyx print("Hello")` should be `Hello`.
- NeoVim:

  ```bash
  # ensure you have installed pynvim
  $ python3 -m pip install pynvim
  ```

</details>

### `Rust`

#### `maple` binary

If you have installed Rust on your system, specifically, `cargo` executable exists, use this single command `:call clap#installer#build_maple()` from Vim.

If you are using macOS or Linux, building the Rust binary is very convenient, just go to the clap plugin directory and run `make`. Or you can run the `cargo` command on your own:

```bash
cd path/to/vim-clap

# Compile the release build
#
# Try running `rustup update` if the follow command runs into an error.
cargo build --release
```

#### Python dynamic module(deprecated)

<details>
  <summary>Python dynamic module has been retired since v0.37, please update the `maple` binary to the latest version</summary>

If you don't have `+python`, you can safely skip this section, it's totally fine, vim-clap can still work very well with only `maple` binary installed. This Python dynamic module is mainly for saving the async job when the data set is small.

Now PyO3(v0.11+) supports stable Rust, therefore the nightly Rust is no longer required. Simply use `:call clap#installer#build_python_dynamic_module()` to install the Python dynamic module written in Rust for 10x faster fuzzy filter than the Python version. Refer to the post [Make Vim Python plugin 10x faster using Rust](http://liuchengxu.org/posts/speed-up-vim-python-plugin-using-rust/) for the whole story.

~~[Python dynamic module](https://github.com/liuchengxu/vim-clap#python-dynamic-module) needs to be compiled using Rust nightly, ensure you have installed it if you want to run the installer function successfully:~~

```bash
# You do not have to install Rust nightly since #471
$ rustup toolchain install nightly
```

</details>

## Download the prebuilt binary from GitHub release

You can call `:call clap#installer#download_binary()` in Vim/NeoVim, or do it manually as follows.

### Quick installer

#### Unix

```bash
$ bash install.sh
```

#### Windows

Run `install.ps1` in the powershell.

### Manual

1. Download the binary from the latest release https://github.com/liuchengxu/vim-clap/releases/ according to your system.
2. Rename the downloaded binary to:
   - Unix: `maple`
   - Windows: `maple.exe`
3. Move `maple`/`maple.exe` to `bin` directory. Don't forget to assign execute permission to `maple` via `chmod a+x bin/maple` if you are using the Unix system.

## Build the Rust binary via Docker

### Linux

If you run into the libssl error when using the prebuilt binary from GitHub release, you can try building a static Rust binary:

```bash
$ cd path/to/vim-clap
$ docker run --rm -it -v "$(pwd)":/volume clux/muslrust cargo build --profile production --locked
$ cp target/x86_64-unknown-linux-musl/production/maple bin/maple
# See if it really works
$ ./bin/maple version
```
