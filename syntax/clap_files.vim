hi TNormal ctermfg=249 ctermbg=None guifg=#b2b2b2 guibg=None
execute 'syntax match ClapFile' '/^.*/' 'contains='.join(clap#icon#add_head_hl_groups(), ',')

hi default link ClapFile TNormal
