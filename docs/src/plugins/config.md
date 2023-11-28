# Configuration

User config file is loaded from:

- Linux: `~/.config/vimclap/config.toml`
- macOS: `~/Library/Application\ Support/org.vim.Vim-Clap/config.toml`
- Windows: `C:\Users\Alice\AppData\Roaming\Vim\Vim Clap\config\config.toml`

```toml
## Log configuration.
[log]
# Specify the max log level.
max-level = "debug"
# Specify the log target.
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
# Disable the ctags plugin if the size of file exceeds the max size limit.
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

## Provider configuration.
[provider]
# Whether to share the input history among providers.
share-input-history = false
# Specify the syntax highlight engine for the provider preview.
preview-highlight-engine = "vim"

# Ignore configuration per project, with paths specified as absoluate path
# or relative to the home directory.
[provider.project-ignores]

# Ignore configuration per provider, with priorities as follows:
# `provider_ignores` > `provider_ignores` > `global_ignore`
[provider.provider-ignores]

# Delay in milliseconds before handling the the user query.
#
# When enabled and not-zero, some intermediate inputs may be dropped if user types too fast.
#
# # Config example
#
# ```toml
# [provider.debounce]
# # Set debounce to 200ms for all providers by default.
# "*" = 200
#
# # Set debounce to 100ms for files provider specifically.
# "files" = 100
# ```
[provider.debounce]

## Global ignore configuration.
[global-ignore]
# Whether to ignore the comment line when it's possible.
ignore-comments = false
# Only include the results from the files being tracked by git if in a git repo.
git-tracked-only = false
# Ignore the results from the files whose file name matches this pattern.
ignore-file-name-pattern = []
# Ignore the results from the files whose file path matches this pattern.
ignore-file-path-pattern = []
```