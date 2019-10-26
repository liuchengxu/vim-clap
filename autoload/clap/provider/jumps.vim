" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the jump list with the preview.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:jumps = {}

function! s:jumps.source() abort
  call g:clap.start.goto_win()
  redir => cout
  silent jumps
  redir END
  call g:clap.input.goto_win()
  let s:jumplist = split(cout, '\n')
  return s:jumplist
endfunction

function! s:jumps.sink(line) abort
  if empty(a:line)
    return
  endif
  let idx = index(s:jumplist, a:line)
  if idx == -1
    return
  endif
  let pointer = match(s:jumplist, '\v^\s*\>')
  if pointer ==# a:line
    return
  endif
  let delta = idx - pointer
  let cmd = delta < 0 ? abs(delta)."\<C-O>" : delta."\<C-I>"
  execute 'normal!' cmd
endfunction

function! s:jumps.on_move() abort
  let curline = g:clap.display.getcurline()
  let matched = matchlist(curline, '^\s\+\(\d\+\)\s\+\(\d\+\)\s\+\(\d\+\)\s\+\(.*\)$')
  if len(matched) < 5
    return
  endif
  call clap#provider#marks#preview_impl(matched[2], matched[3], matched[4])
endfunction

let g:clap#provider#jumps# = s:jumps

let &cpoptions = s:save_cpo
unlet s:save_cpo
