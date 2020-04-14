" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Utilities for sink function.

let s:save_cpo = &cpoptions
set cpoptions&vim

function! clap#sink#open_file(fpath, lnum, col) abort
  normal! m'

  if has_key(g:clap, 'open_action')
    execute g:clap.open_action a:fpath
  else
    " Cannot use noautocmd here as it would lose syntax, and ...
    execute 'edit' a:fpath
  endif

  noautocmd call cursor(a:lnum, a:col)
  normal! zz
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
