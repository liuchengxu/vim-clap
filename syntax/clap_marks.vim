syntax match ClapMark /^\s\+[0-9a-zA-Z`'"[\]\.\^]/
syntax match ClapMarkLine /\s\+\zs\d\+\s\+\ze\d\+/
syntax match ClapMarkFileText /\d\+\s\+\zs.*$/
syntax match ClapMarkHeader /mark line  col file\/text/

hi default link ClapMark Function
hi default link ClapMarkLine Number
hi default link ClapMarkFileText String
hi default link ClapMarkHeader Title
