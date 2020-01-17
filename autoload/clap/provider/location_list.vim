" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the entries of the location list.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:location_list = {}

function! s:location_list.source() abort
  let loclist = getloclist(g:clap.start.winid)
  if empty(loclist)
    return ['Location list is empty for window '.g:clap.start.winid]
  else
    return map(loclist, 'clap#provider#quickfix#into_qf_line(v:val)')
  endif
endfunction

function! s:location_list.sink(selected) abort
  let [_, lnum, column] = clap#provider#quickfix#extract_position(a:selected)
  call cursor(lnum, column)
endfunction

let s:location_list.syntax = 'qf'
let g:clap#provider#location_list# = s:location_list

let &cpoptions = s:save_cpo
unlet s:save_cpo
