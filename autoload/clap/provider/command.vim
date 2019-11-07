" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the command.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:command = {}

function! s:command.sink(selected) abort
  let s:cmd = matchstr(a:selected, '^[!b ]*\zs\(\w*\)\ze ')
endfunction

function! s:command.source() abort
  redir => l:command
  silent command
  redir END
  return split(l:command, "\n")
endfunction

let s:command.on_enter = { -> g:clap.display.setbufvar('&ft', 'clap_command') }

" Actually apply the sink on_exit due to some observed issues, could be some
" conflictions with the hooks of other plugins.
" FIXME: maybe we should rearrange the invocation time for sink?
function! s:command.on_exit() abort
  if exists('s:cmd')
    execute s:cmd
  endif
endfunction

let g:clap#provider#command# = s:command

let &cpoptions = s:save_cpo
unlet s:save_cpo
