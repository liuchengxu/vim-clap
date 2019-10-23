" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the open buffers.

let s:save_cpo = &cpoptions
set cpoptions&vim

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
  let icon = s:padding(clap#icon#for(name), 3)
  let extra = join(filter([modified, readonly], '!empty(v:val)'), '')
  let line = s:padding(get(s:line_info, a:b, ''), 10)

  return trim(printf('%s %s %s %s %s %s %s', bp, fsize, icon, line, name, flag, extra))
endfunction

function! s:buffers() abort
  redir => l:buffers
    silent buffers
  redir END
  let s:line_info = {}
  for line in split(l:buffers, "\n")
    let bufnr = str2nr(trim(matchstr(line, '^\s*\d\+')))
    let lnum = matchstr(line, '\s\+\zsline.*$')
    let s:line_info[bufnr] = lnum
  endfor
  let bufs = map(clap#util#buflisted_sorted(), 's:format_buffer(str2nr(v:val))')
  return bufs[1:] + [bufs[0]]
endfunction

function! s:buffers_sink(selected) abort
  call g:clap.start.goto_win()
  let b = matchstr(a:selected, '^\[\zs\d\+\ze\]')
  execute 'buffer' b
endfunction

function! s:buffers_on_enter() abort
  call g:clap.display.setbufvar('&ft', 'clap_buffers')
endfunction

let s:buffers = {}
let s:buffers.sink = function('s:buffers_sink')
let s:buffers.source = function('s:buffers')
let s:buffers.on_enter = function('s:buffers_on_enter')

let g:clap#provider#buffers# = s:buffers

let &cpoptions = s:save_cpo
unlet s:save_cpo
