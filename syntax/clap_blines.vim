syntax match ClapBlinesLineNr /^\s*\d\+ /
syntax match ClapBlines  /^.*$/ contains=ClapBlinesLineNr

hi default link ClapBlinesLineNr Number
hi default link ClapBlines SpecialComment
