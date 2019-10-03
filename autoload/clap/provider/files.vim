" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the files.

let s:save_cpo = &cpo
set cpo&vim

let s:files = {}

let s:tools = {
      \ 'fd': '--type f',
      \ 'rg': '--files',
      \ 'git': 'ls-tree -r --name-only HEAD',
      \ 'find': '. -type f',
      \ }

let s:find_cmd = v:null

for exe in ['fd', 'rg', 'git', 'find']
  if executable(exe)
    let s:find_cmd = join([exe, s:tools[exe]], ' ')
    break
  endif
endfor

if s:find_cmd is v:null
  let s:find_cmd = ['No usable tools found for the files provider']
endif

let s:files.source = s:find_cmd
let s:files.sink = 'e'

function! s:files.source_async() abort
  let l:cur_input = g:clap.input.get()
  let ext_filter_cmd = clap#filter#get_external_cmd_or_default()
  let cmd = s:find_cmd.' | '.ext_filter_cmd
  return cmd
endfunction

let g:clap#provider#files# = s:files

let &cpo = s:save_cpo
unlet s:save_cpo
