syntax match ClapNoMathcesFound /^NO MATCHES FOUND/
syntax match ClapCommandHistNr /^\d\+/
syntax match ClapCommandHist /^.$/ contains=ClapCommandHistNr

hi default link ClapNoMathcesFound ErrorMsg
hi default link ClapCommandHistNr Number
hi default link ClapCommandHist   LineNr
