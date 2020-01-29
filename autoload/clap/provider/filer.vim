" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Ivy-like file explorer.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:filer = {}

if g:clap_enable_icon
  let s:FOLDER_IS_EMPTY = ['  Directory is empty']
else
  let s:FOLDER_IS_EMPTY = ['Directory is empty']
endif

function! s:handle_round_message(message) abort
  try
    let decoded = json_decode(a:message)
  catch
    call clap#helper#echo_error('Failed to decode message:'.a:message.', exception:'.v:exception)
    return
  endtry

  if has_key(decoded, 'error')
    call g:clap.display.set_lines([decoded.error])

  elseif has_key(decoded, 'result')
    let result = decoded.result
    if result.total == 0
      call g:clap.display.set_lines(s:FOLDER_IS_EMPTY)
    else
      let s:filer_cache[result.dir] = result.entries
      call g:clap.display.set_lines(result.entries)
    endif
    call clap#sign#reset_to_first_line()
    call clap#impl#refresh_matches_count(string(result.total))
    call g:clap#display_win.shrink_if_undersize()

  else
    call clap#helper#echo_error('This should not happen, neither error nor result is found.')
  endif
endfunction

function! s:is_directory(path) abort
  return a:path[-1:] ==# '/'
endfunction

function! s:set_prompt() abort
  if strlen(s:current_dir) < s:winwidth * 3 / 4
    call clap#spinner#set(s:current_dir)
  else
    let parent = fnamemodify(s:current_dir, ':p:h')
    let last = fnamemodify(s:current_dir, ':p:t')
    call clap#spinner#set(pathshorten(parent).'/'.last)
  endif
endfunction

function! s:goto_parent() abort
  " The root directory
  if s:current_dir ==# '/'
    return
  endif
  if s:is_directory(s:current_dir)
    let parent_dir = fnamemodify(s:current_dir, ':h:h')
  else
    let parent_dir = fnamemodify(s:current_dir, ':h')
  endif

  if parent_dir ==# '/'
    let s:current_dir = '/'
  else
    let s:current_dir = parent_dir.'/'
  endif
  call s:set_prompt()
  call s:filter_or_send_message()
endfunction

function! s:filter_or_send_message() abort
  if has_key(s:filer_cache, s:current_dir)
    call s:do_filter()
  else
    call s:send_message()
  endif
endfunction

function! s:bs_action() abort
  call clap#highlight#clear()

  let input = g:clap.input.get()
  if input ==# ''
    call s:goto_parent()
  else
    call g:clap.input.set(input[:-2])
    call s:filter_or_send_message()
  endif
  return ''
endfunction

function! s:do_filter() abort
  let query = g:clap.input.get()
  call clap#filter#on_typed(function('clap#filter#'), query, s:filer_cache[s:current_dir])
endfunction

function! s:send_message() abort
  " Note: must use v:true/v:false for json_encode
  let msg = json_encode({
        \ 'method': 'filer',
        \ 'params': {'cwd': s:current_dir, 'enable_icon': s:enable_icon},
        \ 'id': 1
        \ })
  call clap#rpc#send_message(msg)
endfunction

function! s:tab_action() abort
  call clap#highlight#clear()

  if exists('g:__clap_has_no_matches') && g:__clap_has_no_matches
    return
  endif

  let current_entry = s:get_current_entry()
  if filereadable(current_entry)
    " TODO: preview file
    return ''
  endif

  let s:current_dir = current_entry
  call s:set_prompt()
  call g:clap.input.set('')

  call s:filter_or_send_message()

  return ''
endfunction

function! s:get_current_entry() abort
  let curline = g:clap.display.getcurline()
  if g:clap_enable_icon
    let curline = curline[4:]
  endif

  if s:current_dir[-1:] ==# '/'
    return s:current_dir.curline
  else
    return s:current_dir.'/'.curline
  endif
endfunction

function! s:filer_sink(selected) abort
  if g:clap_enable_icon
    let curline = a:selected[4:]
  else
    let curline = a:selected
  endif
  if s:current_dir[-1:] ==# '/'
    let current_entry = s:current_dir.curline
  else
    let current_entry = s:current_dir.'/'.curline
  endif
  execute 'edit' current_entry
endfunction

function! s:filer_on_typed() abort
  call clap#highlight#clear()
  call s:filter_or_send_message()
  return ''
endfunction

function! s:filer_on_no_matches(input) abort
  execute 'edit' a:input
endfunction

function! s:start_rpc_service() abort
  let s:filer_cache = {}
  if !empty(g:clap.provider.args) && isdirectory(expand(g:clap.provider.args[0]))
    let s:current_dir = expand(g:clap.provider.args[0]).'/'
  else
    let s:current_dir = getcwd().'/'
  endif
  let s:winwidth = winwidth(g:clap.display.winid)
  let s:enable_icon = g:clap_enable_icon ? v:true : v:false
  call s:set_prompt()
  call clap#rpc#start(function('s:handle_round_message'))
  call s:send_message()
endfunction

let s:filer.init = function('s:start_rpc_service')
let s:filer.sink = function('s:filer_sink')
let s:filer.syntax = 'clap_filer'
let s:filer.on_typed = function('s:filer_on_typed')
let s:filer.bs_action = function('s:bs_action')
let s:filer.tab_action = function('s:tab_action')
let s:filer.source_type = g:__t_rpc
let s:filer.on_no_matches = function('s:filer_on_no_matches')
let g:clap#provider#filer# = s:filer

let &cpoptions = s:save_cpo
unlet s:save_cpo
