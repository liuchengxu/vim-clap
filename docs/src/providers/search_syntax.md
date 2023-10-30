# Search Syntax

## Fzf search syntax

vim-clap adopts the almost all fzf search syntax, please refer to [the search syntax section of fzf's README](https://github.com/junegunn/fzf#search-syntax) for more details. Note that the OR operator defined by a single bar character is not yet implemented, but you can achieve that by using multiple exact matches.

### Extended search syntax

Apart from the basic fzf search syntax, more search syntax are supported:

| Token  | Match type | Description                                                  |
| ------ | ---------- | ------------------------------------------------------------ |
| `"cli` | word-match | Items that match word `cli` (`clippy` does not match `"cli`) |
