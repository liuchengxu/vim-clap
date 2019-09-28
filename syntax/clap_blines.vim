syntax match ClapNoMathcesFound /^NO MATCHES FOUND/
syntax match ClapBlinesLineNr /^\s*\d\+ /
syntax match ClapBlines  /^.*$/ contains=ClapBlinesLineNr

hi default link ClapNoMathcesFound ErrorMsg
hi default link ClapBlinesLineNr Number
hi default link ClapBlines SpecialComment
