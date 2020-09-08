" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the windows.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:windows = {}

function! s:jump(w, t) abort
  execute a:t.'tabnext'
  execute a:w.'wincmd w'
endfunction

function! s:get_clap_winids() abort
  let clap_winids = map(
        \ filter(['display', 'input', 'spinner', 'preview'], 'exists("g:clap.".v:val.".winid")'),
        \ 'g:clap[v:val].winid'
        \ )
  if exists('g:__clap_indicator_bufnr')
    call extend(clap_winids, win_findbuf(g:__clap_indicator_bufnr))
  endif
  return clap_winids
endfunction

function! s:format_win(winid) abort
  let buf = winbufnr(a:winid)
  let modified = getbufvar(buf, '&modified')
  let name = bufname(buf)
  let name = empty(name) ? '[No Name]' : name
  let active = a:winid == g:clap.start.winid
  return (active? '> ' : '  ') . name . (modified? ' [+]' : '')
endfunction

function! s:windows.source() abort
  let clap_winids = s:get_clap_winids()
  let lines = []
  for t in range(1, tabpagenr('$'))
    for w in range(1, tabpagewinnr(t, '$'))
      " Skip Clap windows
      let winid = win_getid(w, t)
      if index(clap_winids, winid) != -1
        continue
      endif
      call add(lines, printf('%s %s  %s', printf('%3d', t), printf('%3d', w), s:format_win(winid)))
    endfor
  endfor
  return lines
endfunction

function! s:parse_win(line) abort
  let tab_win = matchlist(a:line, '^ *\([0-9]\+\) *\([0-9]\+\)')
  return [tab_win[2], tab_win[1]]
endfunction

function! s:windows.on_move() abort
  let [win, tab] = s:parse_win(g:clap.display.getcurline())
  let winid = win_getid(win, tab)
  let fpath = bufname(winbufnr(winid))
  let lnum = has('nvim') ? nvim_win_get_cursor(winid)[0] : line('.', winid)
  call clap#preview#file_at(fpath, lnum)
endfunction

function! s:windows.sink(line) abort
  let [win, tab] = s:parse_win(a:line)
  call s:jump(win, tab)
endfunction

let g:clap#provider#windows# = s:windows

let &cpoptions = s:save_cpo
unlet s:save_cpo
