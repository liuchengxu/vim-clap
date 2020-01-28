" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Ivy-like file explorer.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:filer = {}

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
    let s:filer_cache[result.dir] = result.data
    call g:clap.display.set_lines(result.data)
    call clap#sign#reset_to_first_line()
    call clap#impl#refresh_matches_count(string(result.total))
    call g:clap#display_win.shrink_if_undersize()

  else
    call clap#helper#echo_error('This should not happen, neither error nor result is found.')
  endif
endfunction

function! s:goto_parent() abort
  if s:current_dir[-1:] ==# '/'
    let parent_dir = fnamemodify(s:current_dir, ':h:h')
  else
    let parent_dir = fnamemodify(s:current_dir, ':h')
  endif

  let s:current_dir = parent_dir

  call clap#spinner#set(pathshorten(s:current_dir))

  call s:filter_or_send_message()
endfunction

function! s:filter_or_send_message() abort
  if has_key(s:filer_cache, s:current_dir)
    call s:do_filter()
  else
    call s:send_message()
  endif
endfunction

function! clap#provider#filer#bs() abort
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
  let msg = json_encode({
        \ 'method': 'filer',
        \ 'params': {'cwd': s:current_dir},
        \ 'id': 1
        \ })
  call clap#rpc#send_message(msg)
endfunction

function! clap#provider#filer#tab() abort
  call clap#highlight#clear()

  let current_entry = s:get_current_entry()

  if filereadable(current_entry)
    " TODO: preview file
    return ''
  endif

  let s:current_dir = current_entry

  call clap#spinner#set(pathshorten(s:current_dir))
  call g:clap.input.set('')

  call s:filter_or_send_message()

  return ''
endfunction

function! s:get_current_entry() abort
  let curline = g:clap.display.getcurline()

  if s:current_dir[-1:] ==# '/'
    return s:current_dir.curline
  else
    return s:current_dir.'/'.curline
  endif
endfunction

function! clap#provider#filer#sink(selected) abort
  let curline = a:selected
  if s:current_dir[-1:] ==# '/'
    let current_entry = s:current_dir.curline
  else
    let current_entry = s:current_dir.'/'.curline
  endif
  execute 'edit' current_entry
endfunction

function! clap#provider#filer#on_typed() abort
  call clap#highlight#clear()
  call s:filter_or_send_message()
  return ''
endfunction

function! clap#provider#filer#start_rpc_service() abort
  let s:filer_cache = {}
  let s:current_dir = getcwd()
  call clap#spinner#set(pathshorten(s:current_dir))
  call clap#rpc#start(function('s:handle_round_message'))
  let msg = json_encode({
        \ 'method': 'filer',
        \ 'params': {'cwd': s:current_dir},
        \ 'id': 1
        \ })
  call clap#rpc#send_message(msg)
endfunction

let s:filer.init = function('clap#provider#filer#start_rpc_service')
let s:filer.sink = function('clap#provider#filer#sink')
let s:filer.syntax = 'clap_filer'
let s:filer.on_typed = function('clap#provider#filer#on_typed')
let s:filer.bs_action = function('clap#provider#filer#bs')
let s:filer.tab_action = function('clap#provider#filer#tab')
let s:filer.source_type = g:__t_rpc
let g:clap#provider#filer# = s:filer

let &cpoptions = s:save_cpo
unlet s:save_cpo
