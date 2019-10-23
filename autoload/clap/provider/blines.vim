" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the buffer lines.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:blines = {}

function! s:blines.sink(selected) abort
  let lnum = matchstr(a:selected, '^\s*\(\d\+\) ')
  let lnum = str2nr(trim(lnum))
  call g:clap.start.goto_win()
  silent call cursor(lnum, 1)
  normal! ^zvzz
endfunction

function! s:blines.source() abort
  let lines = g:clap.start.get_lines()
  let linefmt = '%4d %s'
  return map(lines, 'printf(linefmt, v:key + 1, v:val)')
endfunction

function! s:blines.on_enter() abort
  call g:clap.display.setbufvar('&ft', 'clap_blines')
endfunction

let g:clap#provider#blines# = s:blines

let &cpoptions = s:save_cpo
unlet s:save_cpo
