" No usual did_ftplugin_loaded check

syntax match ClapLinNr /^.*:\zs\d\+\ze:\d\+:/hs=s+1,he=e-1 contained
syntax match ClapColumn /:\d\+:\zs\d\+\ze:/ contains=ClapLinNr contained
syntax match ClapLinNrColumn /\zs:\d\+:\d\+:\ze/ contains=ClapLinNr,ClapColumn contained

execute 'syntax match ClapFpath' '/^.*:\d\+:\d\+:/' 'contains=ClapLinNrColumn,'.join(clap#icon#add_head_hl_groups(), ',')

hi default link ClapFpath            Keyword
hi default link ClapLinNr            LineNr
hi default link ClapColumn           Comment
hi default link ClapLinNrColumn      Type
