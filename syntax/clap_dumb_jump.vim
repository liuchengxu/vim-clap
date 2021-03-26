syntax match ClapDumbLinNr /^.*:\zs\d\+\ze:\d\+:/hs=s+1,he=e-1 contained
syntax match ClapDumbColumn /:\d\+:\zs\d\+\ze:/ contains=ClapDumbLinNr contained
syntax match ClapDumbLinNrColumn /\zs:\d\+:\d\+:\ze/ contains=ClapDumbLinNr,ClapDumbColumn contained

syntax match ClapDumbKind /^\[\a*\]/ contained

syntax match ClapDumbFpath /^.*:\d\+:\d\+:/ contains=ClapDumbLinNrColumn,ClapDumbKind

hi default link ClapDumbFpath            Keyword
hi default link ClapDumbKind             Title
hi default link ClapDumbLinNr            LineNr
hi default link ClapDumbColumn           Comment
hi default link ClapDumbLinNrColumn      Type
