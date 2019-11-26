syntax match ClapJump /^\s\+\d\+/ nextgroup=ClapJumpLineCol
syntax match ClapJumpsHeader /jump line  col file\/text/
syntax match ClapJumpLineCol /\s\+\zs\d\+\ze\s\+/

hi default link ClapJump        Function
hi default link ClapJumpsHeader Title
hi default link ClapJumpLineCol Number
