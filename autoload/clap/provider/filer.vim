" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Ivy-like file explorer.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:filer = {}

let s:CREATE_FILE = ' [Create new file]'

function! clap#provider#filer#handle_error(error) abort
  call g:clap.preview.show([a:error])
endfunction

function! clap#provider#filer#set_create_file_entry() abort
  call clap#highlighter#clear_display()
  let input = g:clap.input.get()
  let create_file_line = (g:clap_enable_icon ? ' ' : '') . input . s:CREATE_FILE
  call g:clap.display.set_lines([create_file_line])
endfunction

function! clap#provider#filer#sink(entry) abort
  call clap#handler#sink_with({ -> execute('edit '.fnameescape(a:entry))})
endfunction

function! s:filer.on_move_async() abort
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

function! s:handle_action(action) abort
  echom 'action:'.a:action
  let curline = g:clap.display.getcurline()
  if empty(curline)
    let entry = g:clap.input.get()
  else
    let entry = curline
  endif
  if a:action ==# "remove"
    if entry[-1] ==# '/' || entry[-1] ==# '\'
      call delete(entry, 'd')
    else
      call delete(entry)
    endif
  elseif a:action ==# 'new_dir'
    call mkdir(entry)
  endif
  call s:filer.on_typed()
endfunction

let s:filer.init = function('s:start_rpc_service')
let s:filer.icon = 'File'
let s:filer.syntax = 'clap_filer'
let s:filer.source_type = g:__t_rpc
let s:filer.on_typed = { -> clap#client#notify_provider('on_typed') }
let s:filer.mappings = {
      \ "<CR>": { ->  clap#client#notify_provider('cr') },
      \ "<BS>": { -> clap#client#notify_provider('backspace') },
      \ "<Tab>": { ->  clap#client#notify_provider('tab') },
      \ "<A-U>": { -> clap#client#notify_provider('backspace') },
      \ }
let s:filer.action = {
      \ 'title': 'Filer',
      \ '&Remove': { -> clap#client#notify_provider('shift-tab', { 'action': 'Remove' } ) },
      \ 'New&File': { -> clap#client#notify_provider('shift-tab', { 'action': 'NewFile' }) },
      \ 'New&Directory': { -> clap#client#notify_provider('shift-tab', { 'action': 'NewDirectory'} ) },
      \ }
let g:clap#provider#filer# = s:filer

let &cpoptions = s:save_cpo
unlet s:save_cpo
