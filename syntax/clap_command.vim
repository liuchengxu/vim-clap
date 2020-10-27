
syntax match  ClapCommand                /\v^\s+\S+\s+\S+\s+/   contains=ClapCommandArgs,ClapCommandArgsNone
syntax match  ClapCommandArgs            /\v\[[1?*+]\]\s+/      contained nextgroup=ClapCommandName
syntax match  ClapCommandArgsNone        /\v\[0\]\s+/           contained nextgroup=ClapCommandNameI
syntax match  ClapCommandName            /\v\u\w*/              contained skipwhite nextgroup=ClapCommandRest
syntax match  ClapCommandNameI           /\v\u\w*/              contained skipwhite nextgroup=ClapCommandRest
syntax match  ClapCommandRest            /\v.+$/                contained skipwhite

hi default link ClapCommandArgs          Keyword
hi default link ClapCommandArgsNone      Keyword
hi default link ClapCommandName          ModeMsg
hi default link ClapCommandNameI         WarningMsg
hi default link ClapCommandRest          Comment
