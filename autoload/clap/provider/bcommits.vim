" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the buffer commits.

let s:bcommits = {}

function! s:bcommits.source() abort
  call g:clap.abort("Not implemented yet")
endfunction

function! s:bcommits.sink(line) abort
  call g:clap.abort("Not implemented yet")
endfunction

let g:clap#provider#bcommits# = s:bcommits
