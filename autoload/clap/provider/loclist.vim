" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the entries of the location list.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:loclist = {}

function! s:loclist.source() abort
  let loclist = getloclist(g:clap.start.winid)
  if empty(loclist)
    return ['Location list is empty for window '.g:clap.start.winid]
  else
    return map(loclist, 'clap#provider#quickfix#into_qf_line(v:val)')
  endif
endfunction

function! s:loclist.sink(selected) abort
  let [_, lnum, column] = clap#provider#quickfix#extract_position(a:selected)
  call cursor(lnum, column)
endfunction

let s:loclist.syntax = 'qf'
let g:clap#provider#loclist# = s:loclist

let &cpoptions = s:save_cpo
unlet s:save_cpo
