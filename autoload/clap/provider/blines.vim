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
  let items = split(g:clap.display.getcurline())
  if empty(items)
    return
  endif
  if items[0] !~# '^\s*\d\+$'
    return
  endif
  let lnum = str2nr(items[0])
  let [start, end, hi_lnum] = clap#preview#get_line_range(lnum, 5)
  let lines = getbufline(g:clap.start.bufnr, start, end)
  call clap#preview#show_with_line_highlight(lines, s:origin_syntax, hi_lnum+1)
endfunction

function! s:blines.on_enter() abort
  let s:origin_syntax = getbufvar(g:clap.start.bufnr, '&syntax')
  call g:clap.display.setbufvar('&syntax', 'clap_blines')
endfunction

function! s:blines.init() abort
  let line_count = g:clap.start.line_count()
  let g:clap.display.initial_size = line_count

  if line_count > 0
    let lines = getbufline(g:clap.start.bufnr, 1, g:clap.display.preload_capacity)
    call g:clap.display.set_lines_lazy(clap#provider#blines#format(lines))
    call g:clap#display_win.shrink_if_undersize()
    call clap#indicator#set_matches('['.line_count.']')
    call clap#sign#toggle_cursorline()
  endif
endfunction

function! s:into_qf_entry(line) abort
  if a:line =~# '^\s*\d\+ '
    let items = matchlist(a:line, '^\s*\(\d\+\) \(.*\)')
    return { 'bufnr': g:clap.start.bufnr, 'lnum': str2nr(trim(items[1])), 'text': clap#util#trim_leading(items[2]) }
  else
    return { 'bufnr': g:clap.start.bufnr, 'text': a:line }
  endif
endfunction

function! s:blines_sink_star(lines) abort
  call clap#util#open_quickfix(map(a:lines, 's:into_qf_entry(v:val)'))
endfunction

" if Source() is 1,000,000+ lines, it could be very slow, e.g.,
" `blines` provider, so we did a hard code for blines provider here.
let s:blines.source_type = g:__t_func_list
let s:blines['sink*'] = function('s:blines_sink_star')
let g:clap#provider#blines# = s:blines

let &cpoptions = s:save_cpo
unlet s:save_cpo
