" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the buffer commits.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:bcommits = {}

function! s:bcommits.source() abort
  return clap#provider#commits#source_common(v:true)
endfunction

function! s:into_git_diff_cmd(line) abort
  let rev = clap#provider#commits#parse_rev(a:line)
  let prev = s:find_prev(rev)
  return printf('git diff --color=never %s %s -- %s', rev, prev, bufname(g:clap.start.bufnr))
endfunction

function! s:bcommits.on_move() abort
  let cur_line = g:clap.display.getcurline()
  call clap#provider#commits#on_move_common(s:into_git_diff_cmd(cur_line))
endfunction

function! s:bcommits.on_move_async() abort
  call clap#client#call_on_move('on_move', function('clap#provider#commits#on_move_callback'))
endfunction

function! s:bcommits.sink(line) abort
  call clap#provider#commits#sink_inner('!'.s:into_git_diff_cmd(a:line))
endfunction

function! s:bcommits.on_exit() abort
  if exists('s:shas')
    unlet s:shas
  endif
endfunction

function! s:find_prev(cur_rev) abort
  if !exists('s:shas')
    let s:shas = systemlist('git log --format=format:%h')
    let s:shas_len = len(s:shas)
  endif
  let idx = 0
  let prev = 'master'
  for commit in s:shas
    if commit == a:cur_rev
      if idx + 1 < s:shas_len
        let prev = s:shas[idx+1]
      endif
      return prev
    endif
    let idx += 1
  endfor
  return prev
endfunction

let s:bcommits.syntax = 'clap_diff'
let g:clap#provider#bcommits# = s:bcommits

let &cpoptions = s:save_cpo
unlet s:save_cpo
