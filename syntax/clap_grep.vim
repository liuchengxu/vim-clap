" No usual did_ftplugin_loaded check
scriptencoding utf-8

syntax match ClapLinNr /^.*:\zs\d\+\ze:\d\+:/hs=s+1,he=e-1 contained
syntax match ClapColumn /:\d\+:\zs\d\+\ze:/ contains=ClapLinNr contained
syntax match ClapLinNrColumn /\zs:\d\+:\d\+:\ze/ contains=ClapLinNr,ClapColumn contained

" Not sure why this icon somehow are unable to be highlighted in clap#icon#add_head_hl_groups()
syntax match ClapIconUnknown /^\s*/

execute 'syntax match ClapFpath' '/^.*:\d\+:\d\+:/' 'contains=ClapLinNrColumn,'.join(clap#icon#add_head_hl_groups(), ',')
execute 'syntax match ClapFpathTruncated' '/^.*\.\./' 'contains='.join(clap#icon#add_head_hl_groups(), ',').',ClapFpathDots'
syntax match ClapFpathDots '\.\.' contained

hi default link ClapFpath            Keyword
hi default link ClapFpathTruncated   Keyword
hi default link ClapLinNr            LineNr
hi default link ClapColumn           Comment
hi default link ClapLinNrColumn      Type
hi default link ClapIconUnknown      Character
