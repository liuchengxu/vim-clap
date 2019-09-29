" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the open buffers.

let s:save_cpo = &cpo
set cpo&vim

" TODO more fancy buffers, e.g., append icon.
function! s:buffers() abort
  redir => l:buffers
    silent buffers
  redir END
  let s:buffers_cache = split(l:buffers, "\n")
  return s:buffers_cache
endfunction

function! s:buffers_sink(selected) abort
  call win_gotoid(bufwinid(g:clap.start.bufnr))
  let b = split(a:selected)[0]
  execute 'buffer' b
endfunction

function! s:buffers_on_enter() abort
  " Although it's not the vim filetype, we merely want a highlight.
  call g:clap.display.setbufvar('&ft', 'vim')
endfunction

let s:buffers = {}
let s:buffers.sink = function('s:buffers_sink')
let s:buffers.source = function('s:buffers')
let s:buffers.on_enter = function('s:buffers_on_enter')

let g:clap#provider#buffers# = s:buffers

let &cpo = s:save_cpo
unlet s:save_cpo
