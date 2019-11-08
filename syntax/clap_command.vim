syntax match ClapCommand /^[!b ]*\zs\u\w* /
syntax match ClapCommandHeader /Name        Args       Address   Complete  Definition/

hi default link ClapCommand Function
hi default link ClapCommandHeader Title
