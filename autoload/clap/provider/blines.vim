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

function! s:blines.on_move() abort
  let curline = g:clap.display.getcurline()
  let lnum = str2nr(split(curline)[0])
  let [start, end, hi_lnum] = clap#util#get_preview_line_range(lnum, 5)
  let lines = getbufline(g:clap.start.bufnr, start, end)
  call g:clap.preview.show(lines)
  call g:clap.preview.load_syntax(s:origin_ft)
  call g:clap.preview.add_highlight(hi_lnum+1)
endfunction

function! s:blines.on_enter() abort
  let s:origin_ft = getbufvar(g:clap.start.bufnr, '&ft')
  call g:clap.display.setbufvar('&ft', 'clap_blines')
endfunction

let g:clap#provider#blines# = s:blines

let &cpoptions = s:save_cpo
unlet s:save_cpo
