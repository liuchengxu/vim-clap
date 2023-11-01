# Clap Plugins

> WARN: This is an experimental feature, use at your own risk!

Vim-Clap was originally a mere Vim fuzzy picker plugin, however, the integration of a robust Rust backend unveiled the potential to implement various additional functionalities effortlessly, for enjoyable experimentation and potential performance enhancements.

Note that the vim-Clap plugins were mainly created for the plugin author's personal uses, thus they may not be feature-complete as their alternatives. Bugs are expected as these plugins are not extensively tested, feel free to use if you are brave enough.


All the non-system plugins are disabled by default. To enable the plugins, you must create a [config file](./config.md) first and set `enable = true` explicitly for the plugins in the config file.

```toml
[plugin.git]
enable = true
```

Check out [Available Plugins](./plugins.md) for more info.
