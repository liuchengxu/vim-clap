syntax match ClapRegistersTitle /^[A-Za-z-]*/ contained
syntax match ClapRegistersTitleColon /^[A-Za-z-]*:/ contains=ClapRegistersTitle
syntax match ClapRegistersReg /^ ./ contained
syntax match ClapRegistersRegColon /^ .:/ contains=ClapRegistersReg
highlight default link ClapRegistersTitle Title
highlight default link ClapRegistersTitleColon SpecialKey
highlight default link ClapRegistersReg Label
highlight default link ClapRegistersRegColon SpecialKey
highlight default link ClapRegistersSelected Todo
