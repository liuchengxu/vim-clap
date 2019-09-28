" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the jump list with the preview.

let s:jumps = {}

function! s:jumps.source() abort
  call g:clap.abort("Not implemented yet")
endfunction

function! s:jumps.sink(line) abort
  call g:clap.abort("Not implemented yet")
endfunction

let g:clap#provider#jumps# = s:jumps
