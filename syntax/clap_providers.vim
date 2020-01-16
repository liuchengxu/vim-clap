syntax match ClapProviderColon /:/ contained
syntax match ClapProviderId /^\w\+:\? \?/ contains=ClapProviderColon
syntax match ClapProviderAbout /^.*$/ contains=ClapProviderId

hi default link ClapProviderId    Function
hi default link ClapProviderColon Type
hi default link ClapProviderAbout String
