syn match gitInfo    /^[^0-9]*\zs[0-9-]\+\s\+[a-f0-9]\+ / contains=gitDate,gitSha nextgroup=gitMessage,gitMeta
syn match gitDate    /\S\+ / contained
syn match gitSha     /[a-f0-9]\{6,}/ contained
syn match gitMessage /.* \ze(.\{-})$/ contained contains=gitTag,gitGitHub,gitJira nextgroup=gitAuthor
syn match gitAuthor  /.*$/ contained
syn match gitMeta    /([^)]\+) / contained contains=gitTag nextgroup=gitMessage
syn match gitTag     /(tag:[^)]\+)/ contained
syn match gitGitHub  /\<#[0-9]\+\>/ contained
hi def link gitDate   Number
hi def link gitSha    Identifier
hi def link gitTag    Constant
hi def link gitGitHub Label
hi def link gitJira   Label
hi def link gitMeta   Conditional
hi def link gitAuthor String

syn match gitAdded     "^\W*\zsA\t.*"
syn match gitDeleted   "^\W*\zsD\t.*"
hi def link gitAdded    diffAdded
hi def link gitDeleted  diffRemoved


syn match diffAdded   "^+.*"
syn match diffRemoved "^-.*"
syn match diffLine    "^@.*"
syn match diffFile    "^diff\>.*"
syn match diffFile    "^+++ .*"
syn match diffNewFile "^--- .*"
hi def link diffFile    Type
hi def link diffNewFile diffFile
hi def link diffAdded   Identifier
hi def link diffRemoved Special
hi def link diffFile    Type
hi def link diffLine    Statement
