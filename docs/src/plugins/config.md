# Configuration

User config file is loaded from:

- Linux: `~/.config/vimclap/config.toml`
- macOS: `~/Library/Application\ Support/org.vim.Vim-Clap/config.toml`
- Windows: `C:\Users\Alice\AppData\Roaming\Vim\Vim Clap\config\config.toml`

The default config is as follows:

```toml
## Log configuration.
[log]
# Specify the max log level.
max-level = "debug"
# Specify the log target to enable more detailed logging.
#
# Particularly useful for the debugging purpose.
#
# ```toml
# [log]
# log-target = "maple_core::stdio_server=trace,rpc=debug"
# ```
log-target = ""

## Matcher configuration.
[matcher]
# Specify how the results are sorted.
tiebreak = "score,-begin,-end,-length"
[plugin.colorizer]
# Whether to enable this plugin.
enable = false

[plugin.cursorword]
# Whether to enable this plugin.
enable = false
# Whether to ignore the comment line
ignore-comment-line = false
# Disable the plugin when the file matches this pattern.
ignore-files = "*.toml,*.json,*.yml,*.log,tmp"

[plugin.ctags]
# Whether to enable this plugin.
enable = false
# Disable this plugin if the file size exceeds the max size limit.
#
# By default the max file size limit is 4MiB.
max-file-size = 4194304

[plugin.git]
# Whether to enable this plugin.
enable = true

[plugin.linter]
# Whether to enable this plugin.
enable = false

[plugin.markdown]
# Whether to enable this plugin.
enable = false

# How the render the tree-sitter highlights.
#
# The default strategy is to render the entire buffer until the
# file size exceeds 256 KiB.
#
#
# Possible values:
# - `visual-lines`: Always render the visual lines only.
# - `entire-buffer-up-to-limit`: Render the entire buffer until
# the buffer size exceeds the size limit (in bytes).
#
# # Example
#
# ```toml
# [plugin.syntax.render-strategy]
# strategy = "visual-lines"
# ```
[plugin.syntax.render-strategy]
strategy = "entire-buffer-up-to-limit"
file-size-limit = 262144

## Provider (fuzzy picker) configuration.
[provider]
# Whether to share the input history among providers.
share-input-history = false
# Specify the syntax highlight engine for the provider preview.
#
# Possible values: `vim`, `sublime-syntax` and `tree-sitter`
preview-highlight-engine = "vim"

# Ignore configuration per project, with paths specified as
# absolute path or relative to the home directory.
[provider.project-ignores]

# Ignore configuration per provider.
#
# There are multiple ignore settings, with priorities as follows:
# `provider_ignores` > `provider_ignores` > `global_ignore`
[provider.provider-ignores]

# Delay in milliseconds before handling the the user query.
#
# When the delay is set not-zero, some intermediate inputs
# may be dropped if user types too fast.
#
# By default the debounce is set to 200ms to all providers.
#
# # Example
#
# ```toml
# [provider.debounce]
# # Set debounce to 100ms for files provider specifically.
# "files" = 100
# ```
[provider.debounce]

## Global ignore configuration.
[global-ignore]
# Whether to ignore the comment line when applicable.
ignore-comments = false
# Only include the results from the files being tracked by git if in a git repo.
git-tracked-only = false
# Ignore the results from the files whose file name matches this pattern.
#
# For instance, if you want to exclude the results whose file name matches
# `test` for dumb_jump provider:
#
# ```toml
# [provider.provider-ignores.dumb_jump]
# ignore-file-path-pattern = ["test"]
# ```
ignore-file-name-pattern = []
# Ignore the results from the files whose file path matches this pattern.
ignore-file-path-pattern = []
```