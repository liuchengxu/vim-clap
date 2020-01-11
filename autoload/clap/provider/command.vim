" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the command.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:command = {}

function! s:command.sink(selected) abort
  let cmd = matchstr(a:selected, '^[!b ]*\zs\(\w*\)\ze ')
  execute cmd
endfunction

function! s:command.source() abort
  redir => l:command
  silent command
  redir END
  return split(l:command, "\n")
endfunction

let s:command.syntax = 'clap_command'

let g:clap#provider#command# = s:command

let &cpoptions = s:save_cpo
unlet s:save_cpo
