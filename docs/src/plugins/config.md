# Configuration

User config file is loaded from:

- Linux: `~/.config/vimclap/config.toml`
- macOS: `~/Library/Application\ Support/org.vim.Vim-Clap/config.toml`
- Windows: `C:\Users\Alice\AppData\Roaming\Vim\Vim Clap\config\config.toml`

```toml
[log]
# Note that the log file path must be an absolute path.
log-file = "/tmp/clap.log"
max-level = "debug"

[matcher]
# There are four sort keys for results: score, begin, end, length,
# you can specify how the records are sorted using `tiebreak`.
tiebreak = "score,-begin,-end,-length"

[provider]
preview-highlight-engine = "tree-sitter"
```
