syntax match ClapFileName /\v(\/|\s)\zs(\w|[-.])+(\.\w+)?$/ contained
execute 'syntax region ClapFilePath' 'start=/^\s*\S/ end=/(\w|[-.])+(\.\w\+)?$/' 'contains='.join(clap#icon#add_head_hl_groups(), ',').',ClapFileName'

highlight default link ClapFilePath Normal
highlight default link ClapFileName Special
