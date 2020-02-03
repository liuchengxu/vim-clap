" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Common utilities for command/search history.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Derived from fzf.vim
function! s:get_history_list(type) abort
  let max  = histnr(a:type)
  let fmt  = ' %'.len(string(max)).'d '
  let list = filter(map(range(1, max), 'histget("'. a:type .'", - v:val)'), '!empty(v:val)')
  return list
endfunction

function! clap#common_history#source(type) abort
  let hist_list = s:get_history_list(a:type)
  let hist_len = len(hist_list)
  return map(hist_list, 'printf("%4d", hist_len - v:key)."  ".v:val')
endfunction

function! clap#common_history#sink(type, selected) abort
  let item = matchstr(a:selected, '\d\+\s\+\zs\(.*\)')
  if a:type ==# ':'
    call histadd(':', item)
    call execute(item, '')
  elseif a:type ==# '/'
    let @/ = item
    normal! n
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
