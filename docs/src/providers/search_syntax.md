# Search Syntax

## Fzf search syntax

vim-clap adopts most of [fzf search syntax](https://github.com/junegunn/fzf#search-syntax). Note that the OR operator defined by a single bar character is unsupported, but you can achieve that by using multiple exact matches.

| Token     | Match type                 | Description                        |
| ------    | ----------                 | ---------------------------------- |
| `sbtrkt`  | fuzzy-match                | Items that match sbtrkt            |
| `'wild`   | exact-match (quoted)       | Items that include wild            |
| `^music`  | prefix-exact-match         | Items that start with music        |
| `.mp3$`   | suffix-exact-match         | Items that end with .mp3           |
| `!fire`   | inverse-exact-match        | Items that do not include fire     |
| `!^music` | inverse-prefix-exact-match | Items that do not start with music |
| `!.mp3$`  | inverse-suffix-exact-match | Items that do not end with .mp3    |


### Extended search syntax

Apart from the basic fzf search syntax, more search syntax are supported:

| Token  | Match type | Description                                                  |
| ------ | ---------- | ------------------------------------------------------------ |
| `"cli` | word-match | Items that match word `cli` (`clippy` does not match `"cli`) |
