" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the windows.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:windows = {}

function! s:jump(t, w) abort
  execute a:t.'tabnext'
  execute a:w.'wincmd w'
endfunction

function! s:format_win(tab, win, buf) abort
  let modified = getbufvar(a:buf, '&modified')
  let name = bufname(a:buf)
  let name = empty(name) ? '[No Name]' : name
  let active = tabpagewinnr(a:tab) == a:win
  return (active? '> ' : '  ') . name . (modified? ' [+]' : '')
endfunction

function! s:windows.source() abort
  let lines = []
  for t in range(1, tabpagenr('$'))
    let buffers = tabpagebuflist(t)
    for w in range(1, len(buffers))
      call add(lines,
        \ printf('%s %s  %s',
            \ printf('%3d', t),
            \ printf('%3d', w),
            \ s:format_win(t, w, buffers[w-1])
            \ )
            \ )
    endfor
  endfor
  return lines
endfunction

function! s:windows.sink(line) abort
  let list = matchlist(a:line, '^ *\([0-9]\+\) *\([0-9]\+\)')
  call s:jump(list[1], list[2])
endfunction

let g:clap#provider#windows# = s:windows

let &cpoptions = s:save_cpo
unlet s:save_cpo
