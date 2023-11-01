# Available Plugins

TODO: elaborate on plugins' usage.

<!-- clap-markdown-toc -->

* [ctags](#ctags)
* [cursorword](#cursorword)
* [git](#git)
* [linter](#linter)
* [markdown](#markdown)

<!-- /clap-markdown-toc -->

## ctags

<!-- - Alternatives -->
    <!-- - vista.vim -->

## cursorword

```toml
[plugin.cursorword]
enable = true
```

| Features                               | Alternatives                                                                                                                                                                                                                                                   |
| :------------------------------------- | :-----------------------------------------------------                                                                                                                                                                                                         |
| Highlight the word under the cursor    | [nvim-blame-line](https://github.com/tveskag/nvim-blame-line)</br>[vim-illuminate](https://github.com/RRethy/vim-illuminate)</br> [vim-cursorword](https://github.com/itchyny/vim-cursorword)</br>[vim-brightest](https://github.com/osyo-manga/vim-brightest) |

## git

```toml
[plugin.git]
enable = true
```

| Features                                       | Alternatives                                                  |
| :-------------------------------------         | :-----------------------------------------------------        |
| Show blame info at the end of line             | [nvim-blame-line](https://github.com/tveskag/nvim-blame-line) |
| Open the permalink of current line in browser | _none_                                                        |

## linter

```toml
[plugin.linter]
enable = true
```

- Features
  - Lint files asynchronously

- Alternatives
  - [ale](https://github.com/dense-analysis/ale)

## markdown

```toml
[plugin.markdown]
enable = true
```

- Features
    - Generate/Update/Delete toc
