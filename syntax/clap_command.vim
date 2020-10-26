syntax match ClapCommand           /^[!"|b ]*\zs\u\w* / nextgroup=ClapCommandArgs skipwhite
syntax match ClapCommandArgs       /\[[1?*+]\]/
syntax match ClapCommandArgsNone   /\[0\]/

hi default link ClapCommand         Function
hi default link ClapCommandArgs     Keyword
hi default link ClapCommandArgsNone WarningMsg
