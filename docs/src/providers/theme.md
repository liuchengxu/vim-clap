# Theme

By default vim-clap would use the colors extracted from your current colorscheme, which is not guaranteed to suitable for all the colorschemes. You can try the built-in `material_design_dark` theme if the default theme does not work well:

```vim
let g:clap_theme = 'material_design_dark'
```

![material_design_dark-theme](https://user-images.githubusercontent.com/8850248/74818883-6cfdc380-533a-11ea-81fb-d09d90498c96.png)

You could also set `g:clap_theme` to be a `Dict` to specify the palette:

```vim
" Change the CamelCase of related highlight group name to under_score_case.
let g:clap_theme = { 'search_text': {'guifg': 'red', 'ctermfg': 'red'} }
```

`ClapDisplay` and `ClapPreview` are the most basic highlight groups for the display and preview window, which can be overridden if the provider has its own syntax highlight, then checkout the related [syntax](syntax) file for more granular highlights directly.

If you are keen to explore and even want to write your own clap theme, take [autoload/clap/themes/material_design_dark.vim](../../../autoload/clap/themes/material_design_dark.vim) as a reference.

See `:help clap-highlights` for more information.
