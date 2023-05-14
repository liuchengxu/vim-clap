" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Grep using the filer-like interface.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:igrep = {}

let s:CREATE_FILE = ' [Create new file]'

function! s:igrep.on_move_async() abort
  if stridx(g:clap.display.getcurline(), s:CREATE_FILE) > -1
    call g:clap.preview.hide()
    return
  endif
  call clap#client#notify_provider('on_move')
endfunction

function! s:start_rpc_service() abort
  let current_dir = clap#file_explorer#init_current_dir()
  call clap#file_explorer#set_prompt(current_dir, winwidth(g:clap.display.winid))
  call clap#client#notify_on_init({'cwd': current_dir})
endfunction

let s:igrep.init = function('s:start_rpc_service')
let s:igrep.icon = 'File'
let s:igrep.syntax = 'clap_grep'
let s:igrep.source_type = g:__t_rpc
let s:igrep.on_typed = { -> clap#client#notify_provider('on_typed') }
let s:igrep.mappings = {
      \ "<CR>": { ->  clap#client#notify_provider('cr') },
      \ "<BS>": { -> clap#client#notify_provider('backspace') },
      \ "<Tab>": { ->  clap#client#notify_provider('tab') },
      \ "<A-U>": { -> clap#client#notify_provider('backspace') },
      \ }
let g:clap#provider#igrep# = s:igrep

let &cpoptions = s:save_cpo
unlet s:save_cpo
