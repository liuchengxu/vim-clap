syntax match ClapProjTagLnum /:\zs\d\+/ contained
syntax match ClapProjTagName /^\(.*\):\d\+.*\ze\[.*@.*\]/ contains=ClapProjTagLnum
syntax match ClapProjTagKindPathSeperator /@/ contained
syntax match ClapProjTagBrackets /\[\|\]/ contained
syntax match ClapProjTagKind   /\[\zs.*\ze@\f*\]/ contained
syntax match ClapProjTagPath /\[.*@\f*\]/ contains=ClapProjTagKind,ClapProjTagKindPathSeperator
syntax match ClapProjTagPattern /^.*$/ contains=ClapTagName,ClapProjTagKind,ClapProjTagPath,ClapProjTagLnum

hi default link ClapProjTagName Type
hi default link ClapProjTagKind Function
hi default link ClapProjTagPath Directory
hi default link ClapProjTagPattern Identifier
hi default link ClapProjTagKindPathSeperator String
hi default link ClapProjTagLnum Number
hi default link ClapProjTagBrackets Comment
