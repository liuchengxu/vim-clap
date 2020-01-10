syntax match ClapCommandHistNr /^\s*\d\+/
syntax match ClapCommandHist /^.$/ contains=ClapCommandHistNr

hi default link ClapCommandHistNr Number
hi default link ClapCommandHist   LineNr
