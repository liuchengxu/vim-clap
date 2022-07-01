" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the buffer lines.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:blines = {}

function! s:blines.sink(selected) abort
  let lnum = matchstr(a:selected, '^\s*\(\d\+\) ')
  let lnum = str2nr(trim(lnum))
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
  call clap#preview#buffer(lnum, s:origin_syntax)
endfunction

function! s:blines.on_enter() abort
  let s:origin_syntax = getbufvar(g:clap.start.bufnr, '&syntax')
  call g:clap.display.setbufvar('&syntax', 'clap_blines')
endfunction

if clap#maple#is_available()
  function! clap#provider#blines#initialize() abort
    if g:clap.display.initial_size < 100000
      let s:lines_on_empty_query = getbufline(g:clap.start.bufnr, 1, g:clap.display.preload_capacity)
      call g:clap.display.set_lines_lazy(clap#provider#blines#format(s:lines_on_empty_query))
      call g:clap#display_win.shrink_if_undersize()
      call clap#indicator#set_matches_number(g:clap.display.initial_size)
      call clap#sign#toggle_cursorline()
    endif
  endfunction
  function! clap#provider#blines#on_empty() abort
    if !exists('s:lines_on_empty_query')
      let s:lines_on_empty_query = getbufline(g:clap.start.bufnr, 1, g:clap.display.preload_capacity)
    endif
    return copy(s:lines_on_empty_query)
  endfunction
else
  function! s:blines.init() abort
    let line_count = g:clap.start.line_count()
    let g:clap.display.initial_size = line_count

    if line_count > 0 && line_count < 100000
      let lines = getbufline(g:clap.start.bufnr, 1, g:clap.display.preload_capacity)
      call g:clap.display.set_lines_lazy(clap#provider#blines#format(lines))
      call g:clap#display_win.shrink_if_undersize()
      call clap#indicator#set_matches_number(line_count)
      call clap#sign#toggle_cursorline()
    endif
  endfunction
endif

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

let s:ALWAYS_ASYNC = exists('g:clap_builtin_fuzzy_filter_threshold') && g:clap_builtin_fuzzy_filter_threshold == 0

function! s:blines.on_typed() abort
  call g:clap.display.clear_highlight()
  let l:cur_input = g:clap.input.get()

  if empty(l:cur_input)
    call g:clap.display.set_lines_lazy(clap#provider#blines#on_empty())
    call clap#indicator#set_matches_number(g:clap.display.initial_size)
    call clap#sign#toggle_cursorline()
    call g:clap#display_win.shrink_if_undersize()
    call g:clap.preview.hide()
    call clap#highlight#clear()
    return
  endif

  if clap#filter#async#external#using_maple()
    if filereadable(expand('#'.g:clap.start.bufnr.':p'))
      call clap#filter#async#dyn#start_directly(clap#maple#command#blines())
    else
      let l:raw_lines = clap#provider#blines#format(g:clap.start.get_lines())
      call clap#filter#on_typed(g:clap.provider.filter(), l:cur_input, l:raw_lines)
    endif
  else
    let cmd = g:clap.provider.source_async_or_default()
    call clap#rooter#run(function('clap#dispatcher#job_start'), cmd)
  endif

  call clap#spinner#set_busy()
endfunction

" if Source() is 1,000,000+ lines, it could be very slow, e.g.,
" `blines` provider, so we did a hard code for blines provider here.
let s:blines.source_type = g:__t_func_list
let s:blines['sink*'] = function('s:blines_sink_star')
let s:blines.on_move_async = function('clap#impl#on_move#async')
let g:clap#provider#blines# = s:blines

let &cpoptions = s:save_cpo
unlet s:save_cpo
