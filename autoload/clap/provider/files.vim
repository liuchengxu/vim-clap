" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the files.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:files = {}

let s:default_opts = {
      \ 'fd': '--type f',
      \ 'rg': '--files',
      \ 'git': 'ls-tree -r --name-only HEAD',
      \ 'find': '. -type f',
      \ }

let s:default_finder = v:null

for exe in ['fd', 'rg', 'git', 'find']
  if executable(exe)
    let s:default_finder = exe
    break
  endif
endfor

if s:default_finder is v:null
  let s:default_source = ['No usable tools found for the files provider']
else
  let s:default_source = join([s:default_finder, s:default_opts[s:default_finder]], ' ')
endif

function! s:files.source() abort
  if has_key(g:clap.context, 'finder')
    let finder = g:clap.context.finder
    return finder.' '.join(g:clap.provider.args, ' ')
  elseif g:clap.provider.args == ['--hidden']
    if s:default_finder ==# 'fd' || s:default_finder ==# 'rg'
      return join([s:default_finder, s:default_opts[s:default_finder], '--hidden'], ' ')
    else
      return s:default_source
    endif
  else
    return s:default_source
  endif
endfunction

let s:files.sink = 'e'

let s:files.enable_rooter = v:true

let g:clap#provider#files# = s:files

let &cpoptions = s:save_cpo
unlet s:save_cpo
