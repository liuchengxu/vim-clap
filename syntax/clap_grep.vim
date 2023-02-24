" No usual did_ftplugin_loaded check
scriptencoding utf-8

syntax case ignore

execute 'syntax match GrepFile'       '/^\f*[^:]*/'             'nextgroup=GrepSeparator1' 'contains='.join(clap#icon#add_head_hl_groups(), ',')
syntax match GrepSeparator1 /:/    contained nextgroup=GrepLineNr
syntax match GrepLineNr     /\d\+/ contained nextgroup=GrepSeparator2
syntax match GrepSeparator2 /:/    contained nextgroup=GrepColumnNr
syntax match GrepColumnNr   /\d\+/ contained nextgroup=GrepSeparator3
syntax match GrepSeparator3 /:/    contained nextgroup=GrepPattern
syntax match GrepPattern    /.*/   contained

hi default link GrepFile            Keyword
hi default link GrepSeperator1      Label
hi default link GrepSeperator2      Label
hi default link GrepSeperator3      Label
hi default link GrepLineNr          Character
hi default link GrepColumnNr        Type
hi default link GrepPattern         Normal
