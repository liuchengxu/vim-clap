# Clap Plugins

> WARN: This is an experimental feature, use at your own risk!

Vim-Clap was originally a mere Vim fuzzy picker plugin, however, the integration of a robust Rust backend unveiled the potential to implement various additional functionalities effortlessly, for enjoyable experimentation and potential performance enhancements.

Note that the vim-Clap plugins were mainly created for the plugin author's personal uses, thus they may not be feature-complete as their alternatives. Bugs are expected as these plugins are not extensively tested, feel free to use if you are brave enough.

You must enable the `g:clap_plugin_experimental` flag in your `.vimrc` and create a [config file](./config.md) beforehand.

```vim
" Specify this variable to enable the plugin feature.
let g:clap_plugin_experimental = v:true
```

All the non-system plugins are disabled by default. To enable the plugins, you need to set `enable = true` explicitly for the plugins in the config file.

```toml
[plugin.git]
enable = true
```

Check out [Available Plugins](./plugins.md) for detailed introduction to the plugins. Try `:Clap clap_actions` to take a look at the existing actions.
