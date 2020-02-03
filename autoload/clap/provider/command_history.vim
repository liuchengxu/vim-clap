" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the command history.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Derived from fzf.vim
function! s:command_history_source() abort
  return clap#common_history#source(':')
endfunction

function! s:command_history_sink(selected) abort
  call clap#common_history#sink(':', a:selected)
endfunction

let s:command_history = {}
let s:command_history.sink = function('s:command_history_sink')
let s:command_history.source = function('s:command_history_source')
let s:command_history.syntax = 'clap_command_history'

let g:clap#provider#command_history# = s:command_history

let &cpoptions = s:save_cpo
unlet s:save_cpo
