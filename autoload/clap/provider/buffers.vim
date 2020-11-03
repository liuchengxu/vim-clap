" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the open buffers.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:cur_tab_only = get(g:, 'clap_provider_buffers_cur_tab_only', v:false)

function! s:padding(origin, target_width) abort
  let width = strdisplaywidth(a:origin)
  if width < a:target_width
    return a:origin.repeat(' ', a:target_width - width)
  else
    return a:origin
  endif
endfunction

function! s:format_buffer(b) abort
  let name = bufname(a:b)
  let name = empty(name) ? '[No Name]' : fnamemodify(name, ':p:~:.')
  let flag = a:b == bufnr('')  ? '%' : (a:b == bufnr('#') ? '#' : ' ')
  let modified = getbufvar(a:b, '&modified') ? ' [+]' : ''
  let readonly = getbufvar(a:b, '&modifiable') ? '' : ' [RO]'

  let bp = s:padding('['.a:b.']', 5)
  let fsize = s:padding(clap#util#getfsize(name), 6)
  let icon = g:clap_enable_icon ? s:padding(clap#icon#for(name), 3) : ''
  let extra = join(filter([modified, readonly], '!empty(v:val)'), '')
  let line = s:padding(get(s:line_info, a:b, ''), 10)

  return trim(printf('%s %s %s %s %s %s %s', bp, fsize, icon, line, name, flag, extra))
endfunction

function! s:buffers() abort
  let l:buffers = execute('buffers')
  let s:line_info = {}
  for line in split(l:buffers, "\n")
    let bufnr = str2nr(trim(matchstr(line, '^\s*\d\+')))
    let lnum = matchstr(line, '\s\+\zsline.*$')
    let s:line_info[bufnr] = lnum
  endfor
  let bufs = map(clap#util#buflisted_sorted(s:cur_tab_only), 's:format_buffer(str2nr(v:val))')
  if empty(bufs)
    return []
  else
    return bufs[1:] + [bufs[0]]
  endif
endfunction

function! s:extract_bufnr(line) abort
  return matchstr(a:line, '^\[\zs\d\+\ze\]')
endfunction

function! s:buffers_sink(selected) abort
  let b = s:extract_bufnr(a:selected)
  if has_key(g:clap, 'open_action')
    execute g:clap.open_action
  endif
  execute 'buffer' b
endfunction

function! s:buffers_on_move() abort
  let curline = g:clap.display.getcurline()
  if empty(curline)
    return
  endif
  let bufnr = str2nr(s:extract_bufnr(curline))
  if !has_key(s:line_info, bufnr)
    return
  endif
  let lnum = str2nr(matchstr(s:line_info[bufnr], '\d\+'))
  let [start, end, hi_lnum] = clap#preview#get_range(lnum)
  let lines = getbufline(bufnr, start+1, end+1)
  call insert(lines, bufname(bufnr))
  call g:clap.preview.show(lines)
  call g:clap.preview.setbufvar('&syntax', getbufvar(bufnr, '&syntax'))
  call clap#preview#highlight_header()
endfunction

function! s:action_delete() abort
  let current_matches = g:clap.display.line_count()
  execute 'bdelete' s:current_bufnr
  call g:clap.display.deletecurline()
  call clap#indicator#update_matches_on_deletecurline()
  call g:clap.preview.hide()
  call g:clap#display_win.shrink_if_undersize()
endfunction

function! s:actions_title() abort
  let s:current_bufnr = s:extract_bufnr(g:clap.display.getcurline())
  return 'Choose action for buffer '.s:current_bufnr.':'
endfunction

let s:buffers = {}
let s:buffers.sink = function('s:buffers_sink')
let s:buffers.source = function('s:buffers')
let s:buffers.on_move = function('s:buffers_on_move')
let s:buffers.syntax = 'clap_buffers'
let s:buffers.support_open_action = v:true
let s:buffers.action = {
      \ 'title': function('s:actions_title'),
      \ '&Delete': function('s:action_delete'),
      \ 'OpenInNew&Tab': { -> clap#selection#try_open('ctrl-t') },
      \ 'Open&Vertically': { -> clap#selection#try_open('ctrl-v') },
      \ }

let g:clap#provider#buffers# = s:buffers

let &cpoptions = s:save_cpo
unlet s:save_cpo
