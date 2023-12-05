# Available Plugins

The following plugins may only implement a subset of features of their alternatives.

TODO: elaborate on plugins' usage.

<!-- clap-markdown-toc -->

* [colorizer](#colorizer)
* [ctags](#ctags)
* [cursorword](#cursorword)
* [git](#git)
* [linter](#linter)
* [markdown](#markdown)
* [syntax](#syntax)

<!-- /clap-markdown-toc -->

## colorizer

```toml
[plugin.colorizer]
enable = true
```

| Features                               | Alternatives                                                                                                |
| :------------------------------------- | :-----------------------------------------------------                                                      |
| Highlight color name                   | [colorizer](https://github.com/chrisbra/colorizer)</br>[vim-css-color](https://github.com/ap/vim-css-color) |

## ctags

| Features                               | Alternatives                                            |
| :------------------------------------- | :-----------------------------------------------------  |
| statusline integration(current symbol) | [vista.vim](https://github.com/liuchengxu/vista.vim)    |

## cursorword

```toml
[plugin.cursorword]
enable = true
```

| Features                               | Alternatives                                                                                                                                                                                                                                                   |
| :------------------------------------- | :-----------------------------------------------------                                                                                                                                                                                                         |
| Highlight the word under the cursor    | [nvim-blame-line](https://github.com/tveskag/nvim-blame-line)</br>[vim-illuminate](https://github.com/RRethy/vim-illuminate)</br> [vim-cursorword](https://github.com/itchyny/vim-cursorword)</br>[vim-brightest](https://github.com/osyo-manga/vim-brightest) |

By default this plugin utilizes `Normal` guibg as the primary color. It then lighten this base color for `ClapCursorWord` and darkens it for `ClapCursorWordTwins`. You can manually adjust them in case the default highlights does not meet your expectations.

## git

```toml
[plugin.git]
enable = true
```

| Features                                       | Alternatives                                                  |
| :-------------------------------------         | :-----------------------------------------------------        |
| Show blame info at the end of line             | [nvim-blame-line](https://github.com/tveskag/nvim-blame-line) |
| Show git diff in sign column                   | [vim-gitgutter](https://github.com/airblade/vim-gitgutter)    |
| Open the permalink of current line in browser  | [repolink.nvim](https://github.com/9seconds/repolink.nvim)    |

The signs are updated when you save the buffer and are rendered lazily, i.e., the signs are only displayed when they are visually in the range of screen.

## linter

```toml
[plugin.linter]
enable = true
```

Although [ALE](https://github.com/dense-analysis/ale) has been an incredible and feature-rich linter plugin that served me well for an extended
period, I began to notice a persistent lagging issue over time. There were noticeable delays in refreshing
the latest diagnostics whenever I made changes to the source file (especially the Rust file). This prompted
me to develop this linter plugin in Rust and the results have been remarkable. The diagnostics update is now
considerably faster from what I see on Rust project.

## markdown

```toml
[plugin.markdown]
enable = true
```

- Features
    - Generate/Update/Delete toc

## syntax

```toml
[plugin.syntax]
enable = true
```

This plugin implements the sublime-syntax and tree-sitter highlighting. The sublime-syntax is feature-limited and pretty experimental, this plugin primarily focues on the tree-sitter highlighting support.

Officially tree-sitter supported languages:

- Bash, C, Cpp, Go, Javascript, Json, Markdown, Python, Rust, Toml, Viml
