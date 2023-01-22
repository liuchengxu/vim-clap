" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Utilities for sink function.

let s:save_cpo = &cpoptions
set cpoptions&vim

function! clap#sink#edit_with_open_action(fpath) abort
  if has_key(g:clap, 'open_action')
    execute g:clap.open_action a:fpath
  else
    " Cannot use noautocmd here as it would lose syntax, and ...
    execute 'edit' fnameescape(a:fpath)
  endif
endfunction

function! clap#sink#open_file(fpath, lnum, col) abort
  normal! m'
  call clap#sink#edit_with_open_action(a:fpath)
  noautocmd call cursor(a:lnum, a:col)
  normal! zz
endfunction

function! clap#sink#open_quickfix(qf_entries) abort
  let entries_len = len(a:qf_entries)
  call setqflist(a:qf_entries)
  " If there are only a few items, open the qf window at exact size.
  if entries_len < 15
    execute 'copen' entries_len
  else
    copen
  endif
  cc
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
