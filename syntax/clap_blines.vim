syntax match ClapBlinesLineNr /^\s*\d\+/ contained
syntax match ClapBlines  /^.*$/ contains=ClapBlinesLineNr

hi default link ClapBlinesLineNr Number
hi default link ClapBlines SpecialComment
