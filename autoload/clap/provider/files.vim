" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the files.

let s:save_cpo = &cpo
set cpo&vim

let s:files = {}

let s:find_cmd = v:null

let s:tools = [
      \ ['fd', '--type f'],
      \ ['rg', '--files'],
      \ ['git', 'ls-tree -r --name-only HEAD'],
      \ ['find', '. -type f'],
      \ ]

let s:find_cmd = v:null

for [exe, opt] in s:tools
  if executable(exe)
    let s:find_cmd = join([exe, opt], ' ')
    break
  endif
endfor

if s:find_cmd is v:null
  let s:find_cmd = ['No usable tools found for the files provider']
endif

function! s:files.source() abort
  if has_key(g:clap.context, 'finder')
    let finder = g:clap.context.finder
    return finder. join(g:clap.provider.args, ' ')
  elseif g:clap.provider.args == ['--hidden']
    return 'fd --hidden --type f'
  else
    return a:find_cmd
  endif
endfunction

" let s:files.source = s:find_cmd
let s:files.sink = 'e'

let s:files.enable_rooter = v:true

let g:clap#provider#files# = s:files

let &cpo = s:save_cpo
unlet s:save_cpo
