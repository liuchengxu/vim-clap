" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the lines of all loaded buffer.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:lines = {}

function! s:lines.sink(selected) abort
  let splitted = split(a:selected)
  let bufnr = splitted[0][1:-2]
  let lnum = str2nr(splitted[2])
  execute 'b' bufnr
  silent call cursor(lnum, 1)
  normal! ^zvzz
endfunction

function! s:buflisted() abort
  return filter(range(1, bufnr('$')), 'buflisted(v:val) && getbufvar(v:val, "&filetype") !=# "qf"')
endfunction

function! s:bufnr_display(bufnr) abort
  let bufnr = str2nr(a:bufnr)
  if bufnr < 10
    return '['.bufnr.']'.'  '
  elseif bufnr < 100
    return '['.bufnr.']'.' '
  else
    return '['.bufnr.']'
  endif
endfunction

function! s:lines.source() abort
  let cur = []
  let rest = []
  let buf = bufnr('')

  let buflisted = s:buflisted()

  let longest_name = 0
  let bufnames = {}
  for b in buflisted
    let bp = pathshorten(fnamemodify(bufname(b), ':~:.'))
    let longest_name = max([longest_name, len(bp)])
    let bufnames[b] = bp
  endfor

  let len_bufnames = min([15, longest_name])

  for b in buflisted
    let lines = getbufline(b, 1, '$')
    if empty(lines)
      let path = fnamemodify(bufname(b), ':p')
      let lines = filereadable(path) ? readfile(path) : []
    endif

    let bufname = bufnames[b]
    if len(bufname) > len_bufnames + 1
      let bufname = '…' . bufname[-len_bufnames+1:]
    endif
    let bufname = printf('%'.len_bufnames.'s', bufname)

    let b_display = s:bufnr_display(b)
    let linefmt = '%s  %s  %4d  %s'
    call extend(b == buf ? cur : rest,
    \ filter(
    \   map(lines, '(empty(v:val)) ? "" : printf(linefmt, b_display, bufname, v:key + 1, v:val)'),
    \   '!empty(v:val)'))
  endfor

  return extend(cur, rest)
endfunction

function! s:lines.on_enter() abort
  call g:clap.display.setbufvar('&ft', 'clap_lines')
endfunction

let g:clap#provider#lines# = s:lines

let &cpoptions = s:save_cpo
unlet s:save_cpo
