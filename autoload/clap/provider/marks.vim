" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the marks.

let s:save_cpo = &cpo
set cpo&vim

let s:marks = {}

function! s:format_mark(line)
  return substitute(a:line, '\S', '\=submatch(0)', '')
endfunction

function! s:marks.source() abort
  redir => cout
  silent marks
  redir END
  let list = split(cout, "\n")
  return extend(list[0:0], map(list[1:], 's:format_mark(v:val)'))
endfunction

function! s:marks.sink(line)
  execute 'normal! `'.matchstr(a:line, '\S').'zz'
endfunction

let g:clap#provider#marks# = s:marks

let &cpo = s:save_cpo
unlet s:save_cpo
