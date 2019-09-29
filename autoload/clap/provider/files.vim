" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the files.

let s:save_cpo = &cpo
set cpo&vim

let s:files = {}

function! s:chain_try() abort
  for exe in keys(s:tools)
    if executable(exe)
      return join([exe, s:tools[exe]], ' ')
    endif
  endfor
  return ['No usable tools found for the files provider']
endfunction

if has('win32')
  let s:tools = {
        \ 'fd': '',
        \ 'rg': '--files',
        \ 'git': 'ls-tree -r --name-only HEAD',
        \ 'find': '.',
        \ }
  let s:files.source = s:chain_try()
else
  let s:files.source = 'fd || git ls-tree -r --name-only HEAD || rg --files || find .'
endif

let s:files.sink = 'e'

let g:clap#provider#files# = s:files

let &cpo = s:save_cpo
unlet s:save_cpo
