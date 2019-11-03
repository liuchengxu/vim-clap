" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the command history.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Derived from fzf.vim
function! s:command_history() abort
  let max  = histnr(':')
  let fmt  = ' %'.len(string(max)).'d '
  let list = filter(map(range(1, max), 'histget(":", - v:val)'), '!empty(v:val)')
  return list
endfunction

function! s:command_history_source() abort
  let cmd_hist = s:command_history()
  let max  = histnr(':')
  let cmd_hist_len = len(cmd_hist)
  return map(cmd_hist, 'printf("%4d", cmd_hist_len - v:key)."  ".v:val')
endfunction

function! s:command_history_sink(selected) abort
  let item = matchstr(a:selected, '\d\+\s\+\zs\(.*\)')
  call histadd(':', item)
  let s:cmd = item
endfunction

let s:command_history = {}
let s:command_history.sink = function('s:command_history_sink')
let s:command_history.source = function('s:command_history_source')
let s:command_history.on_enter = { -> g:clap.display.setbufvar('&ft', 'clap_command_history') }
let s:command_history.on_exit = { -> execute(s:cmd) }

let g:clap#provider#command_history# = s:command_history

let &cpoptions = s:save_cpo
unlet s:save_cpo
