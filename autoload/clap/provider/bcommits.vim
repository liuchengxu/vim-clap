" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the buffer commits.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:bcommits = {}

function! s:bcommits.source() abort
  return clap#provider#commits#source_common(v:true)
endfunction

function! s:bcommits.on_move() abort
  let cur_line = g:clap.display.getcurline()
  let rev = clap#provider#commits#parse_rev(cur_line)
  let prev = s:find_prev(rev)
  let cmd = 'git diff --color=never ' . rev . ' ' . prev . ' -- '.bufname(g:clap.start.bufnr)
  call clap#provider#commits#on_move_common(cmd)
endfunction

function! s:bcommits.sink(line) abort
  let rev = clap#provider#commits#parse_rev(a:line)
  let prev = s:find_prev(rev)
  let cmd = printf('!git diff --color=never %s %s -- %s', rev, prev, bufname(g:clap.start.bufnr))
  call clap#provider#commits#sink_inner(cmd)
endfunction

function! s:bcommits.on_exit() abort
  if exists('s:shas')
    unlet s:shas
  endif
endfunction

function! s:find_prev(cur_rev) abort
  if !exists('s:shas')
    let s:shas = split(system('git log --format=format:%h'), "\n")
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
    let idx = idx+1
  endfor
  return prev
endfunction

let s:bcommits.syntax = 'clap_diff'
let g:clap#provider#bcommits# = s:bcommits

let &cpoptions = s:save_cpo
unlet s:save_cpo
