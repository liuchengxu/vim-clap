" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the register list with the preview.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:registers = {}

function! s:registers.source() abort
  call g:clap.abort('Not implemented yet')
endfunction

function! s:registers.sink(line) abort
  call g:clap.abort('Not implemented yet')
endfunction

let g:clap#provider#registers# = s:registers

let &cpoptions = s:save_cpo
unlet s:save_cpo
