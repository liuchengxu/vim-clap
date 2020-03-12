syntax match ClapLinesBufnr /^\[\d\+\]/ nextgroup=ClapLinesBufname

syntax match ClapLinesBufname / \f*\.\f* / nextgroup=ClapLinesNumber

syntax match ClapLinesNumber / \d\+ /

hi default link ClapLinesBufnr   Function
hi default link ClapLinesBufname Type
hi default link ClapLinesNumber  Number
