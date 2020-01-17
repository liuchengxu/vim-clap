" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the buffer lines.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:blines = {}

function! s:blines.sink(selected) abort
  let lnum = matchstr(a:selected, '^\s*\(\d\+\) ')
  let lnum = str2nr(trim(lnum))
  call g:clap.start.goto_win()
  " Push the current position to the jumplist
  normal! m'
  silent call cursor(lnum, 1)
  normal! ^zvzz
endfunction

function! clap#provider#blines#format(lines) abort
  let linefmt = '%4d %s'
  return map(a:lines, 'printf(linefmt, v:key + 1, v:val)')
endfunction

function! s:blines.source() abort
  return clap#provider#blines#format(g:clap.start.get_lines())
endfunction

function! s:blines.on_move() abort
  let curline = g:clap.display.getcurline()
  let lnum = str2nr(split(curline)[0])
  let [start, end, hi_lnum] = clap#util#get_preview_line_range(lnum, 5)
  let lines = getbufline(g:clap.start.bufnr, start, end)
  call g:clap.preview.show(lines)
  call g:clap.preview.set_syntax(s:origin_syntax)
  call g:clap.preview.add_highlight(hi_lnum+1)
endfunction

function! s:blines.on_enter() abort
  let s:origin_syntax = getbufvar(g:clap.start.bufnr, '&syntax')
  call g:clap.display.setbufvar('&syntax', 'clap_blines')
endfunction

let g:clap#provider#blines# = s:blines

let &cpoptions = s:save_cpo
unlet s:save_cpo
