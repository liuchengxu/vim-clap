syntax match ClapDir '^.*/$'
hi default link ClapDir Directory

hi TNormal ctermfg=249 ctermbg=NONE guifg=#b2b2b2 guibg=NONE
execute 'syntax match ClapFile' '/.*[^\/]$/' 'contains='.join(clap#icon#add_head_hl_groups(), ',')

hi default link ClapFile TNormal

call clap#provider#filer#hi_empty_dir()
