syntax match ClapMark /^\s\+[0-9a-zA-Z`'"[\]\.\^]/
syntax match ClapMarkLine /\s\+\zs\d\+\s\+\ze\d\+/
syntax match ClapMarkFileText /\d\+\s\+\zs.*$/

hi link ClapMark Function
hi link ClapMarkLine Number
hi link ClapMarkFileText String
