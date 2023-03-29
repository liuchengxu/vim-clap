setlocal conceallevel=3

syntax match ClapTagfilesInfo     /\v:::.*$/            conceal

syntax match ClapTagfilesName     /\v^.*\ze\s+\[.*\]/   contains=ClapTagfilesLnum
syntax match ClapTagfilesBrackets /\[\|\]/              contained
syntax match ClapTagfilesFilename /\v\f*\/\zs[^\]]+\ze/ contained
syntax match ClapTagfilesFilename /\v\[@<=[^/]+\]@=/    contained
syntax match ClapTagfilesPath     /\[\f\+\]/            contains=ClapTagfilesBrackets,ClapTagfilesFilename

hi default link ClapTagfilesName              Special
hi default link ClapTagfilesPath              Comment
hi default link ClapTagfilesBrackets          Comment
hi default link ClapTagfilesFilename          Directory
