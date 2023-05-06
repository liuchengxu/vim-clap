" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Grep using the filer-like interface.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:igrep = {}

let s:PATH_SEPERATOR = has('win32') && !(exists('+shellslash') && &shellslash) ? '\' : '/'
let s:DIRECTORY_IS_EMPTY = (g:clap_enable_icon ? '  ' : '').'<Empty directory>'
let s:CREATE_FILE = ' [Create new file]'

function! clap#provider#igrep#handle_error(error) abort
  call g:clap.preview.show([a:error])
endfunction

function! s:get_entry_by_line(line) abort
  let curline = a:line
  if g:clap_enable_icon
    let curline = curline[4:]
  endif
  let curline = substitute(curline, '\V' . s:CREATE_FILE, '', '')
  return clap#file_explorer#join(s:current_dir, curline)
endfunction

function! s:igrep_sink(selected) abort
  execute 'edit' fnameescape(s:get_entry_by_line(a:selected))
endfunction

function! clap#provider#igrep#sink(entry) abort
  call clap#handler#sink_with({ -> execute('edit '.fnameescape(a:entry))})
endfunction

function! s:igrep.on_move_async() abort
  if stridx(g:clap.display.getcurline(), s:CREATE_FILE) > -1
    call g:clap.preview.hide()
    return
  endif
  call clap#client#notify_provider('on_move')
endfunction

function! s:igrep.on_no_matches(input) abort
  execute 'edit' clap#file_explorer#join(s:current_dir, a:input)
endfunction

function! s:start_rpc_service() abort
  let s:winwidth = winwidth(g:clap.display.winid)
  let s:current_dir = clap#file_explorer#init_current_dir()
  call clap#file_explorer#set_prompt(s:current_dir, s:winwidth)
  call clap#client#notify_on_init({'cwd': s:current_dir})
endfunction

let s:igrep.init = function('s:start_rpc_service')
let s:igrep.sink = function('s:igrep_sink')
let s:igrep.icon = 'File'
let s:igrep.syntax = 'clap_grep'
let s:igrep.source_type = g:__t_rpc
let s:igrep.on_typed = { -> clap#client#notify_provider('on_typed') }
let s:igrep.mappings = {
      \ "<Tab>": { ->  clap#client#notify_provider('tab') },
      \ "<CR>": { ->  clap#client#notify_provider('cr') },
      \ "<BS>": { -> clap#client#notify_provider('backspace') },
      \ "<A-U>": { -> clap#client#notify_provider('backspace') },
      \ }
let g:clap#provider#igrep# = s:igrep

let &cpoptions = s:save_cpo
unlet s:save_cpo
