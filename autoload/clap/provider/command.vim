" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the command.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:command = {}

function! s:command.sink(selected) abort
  " :h command, note the characters in the first two columns
  let cmd = matchstr(a:selected, '^[!"|b ]*\zs\(\w*\)\ze ')
  execute cmd
endfunction

function! s:command.source() abort
  return split(execute('command'), "\n")
endfunction

let s:command.syntax = 'clap_command'

let g:clap#provider#command# = s:command

let &cpoptions = s:save_cpo
unlet s:save_cpo
