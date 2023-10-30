# Install Rust Dependency

You can download the prebuilt binary from GitHub or compile the binary locally on your own.

### Compile Rust Binary Locally

Refer to [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install) if you haven't installed Rust on your system.

Assuming Rust has already been installed on your system, specifically, `cargo` executable exists, you can have several ways to compile the binary:

- Use this helper function `:call clap#installer#build_maple()` within Vim/NeoVim.

- Run `make` under the clap plugin directory (macOS and Linux).

- Run the `cargo` command on your own:

  ```bash
  cd path/to/vim-clap

  # Compile the release build, you can find the compiled executable at target/release/maple.
  cargo build --release
  ```

### Compile Rust binary via Docker (Linux Only)

If you run into the libssl error when using the prebuilt binary from GitHub release, you can try building a static Rust binary:

```bash
$ cd path/to/vim-clap
$ docker run --rm -it -v "$(pwd)":/volume clux/muslrust cargo build --profile production --locked
$ cp target/x86_64-unknown-linux-musl/production/maple bin/maple
# See if it really works
$ ./bin/maple version
```

### Download Prebuilt binary

The prebuilt binary is available from GitHub release. You can call `:call clap#installer#download_binary()` in Vim/NeoVim, or do it manually as follows.

#### Quick Downloader

The scripts to download the prebuilt binary quickly are provided out of the box. The downloaded executable can be found at `bin/maple` on success.

- Unix: `$ bash install.sh`
- Windows: Run `install.ps1` in the powershell.

#### Download Prebuilt Binary By Hand

1. Download the binary from the latest release [https://github.com/liuchengxu/vim-clap/releases](https://github.com/liuchengxu/vim-clap/releases) according to your system.
2. Rename the downloaded binary to:
   - Unix: `maple`
   - Windows: `maple.exe`
3. Move `maple`/`maple.exe` to `bin` directory. Don't forget to assign execute permission to `maple` via `chmod a+x bin/maple` if you are using the Unix system.
