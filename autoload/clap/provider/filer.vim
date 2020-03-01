" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Ivy-like file explorer.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:filer = {}

let s:DIRECTORY_IS_EMPTY = (g:clap_enable_icon ? '  ' : '').'Directory is empty'

function! clap#provider#filer#hi_empty_dir() abort
  syntax match ClapEmptyDirectory /^.*Directory is empty/
  hi default link ClapEmptyDirectory WarningMsg
endfunction

function! s:handle_round_message(message) abort
  try
    let decoded = json_decode(a:message)
  catch
    call clap#helper#echo_error('Failed to decode message:'.a:message.', exception:'.v:exception)
    return
  endtry

  " Only process the latest request, drop the outdated responses.
  if s:last_request_id != decoded.id
    return
  endif

  if has_key(decoded, 'error')
    let error = decoded.error
    let s:filer_error_cache[error.dir] = error.message
    call g:clap.display.set_lines([error.message])
    call clap#indicator#set_matches('[??]')

  elseif has_key(decoded, 'result')
    let result = decoded.result
    if result.total == 0
      let s:filer_empty_cache[result.dir] = s:DIRECTORY_IS_EMPTY
      call g:clap.display.set_lines([s:DIRECTORY_IS_EMPTY])
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

function! s:set_prompt() abort
  if strlen(s:current_dir) < s:winwidth * 3 / 4
    call clap#spinner#set(s:current_dir)
  else
    let parent = fnamemodify(s:current_dir, ':p:h')
    let last = fnamemodify(s:current_dir, ':p:t')
    let short_dir = pathshorten(parent).'/'.last
    if strlen(short_dir) < s:winwidth * 3 / 4
      call clap#spinner#set(short_dir)
    else
      call clap#spinner#set(pathshorten(s:current_dir))
    endif
  endif
endfunction

function! s:goto_parent() abort
  " The root directory
  if s:current_dir ==# '/'
    return
  endif

  if s:current_dir[-1:] ==# '/'
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

function! s:send_message() abort
  let s:last_request_id += 1
  " Note: must use v:true/v:false for json_encode
  let msg = json_encode({
        \ 'method': 'filer',
        \ 'params': {'cwd': s:current_dir, 'enable_icon': s:enable_icon},
        \ 'id': s:last_request_id
        \ })
  call clap#rpc#send_message(msg)
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
  let candidates = s:filer_cache[s:current_dir]
  if query ==# ''
    call g:clap.display.set_lines(candidates)
    call g:clap#display_win.shrink_if_undersize()
  else
    call clap#filter#on_typed(function('clap#filter#'), query, candidates)
  endif
endfunction

function! s:reset_to(new_dir) abort
  let s:current_dir = a:new_dir
  call s:set_prompt()
  call clap#highlight#clear()
  call g:clap.input.set('')
  call s:filter_or_send_message()
endfunction

function! s:get_current_entry() abort
  let curline = g:clap.display.getcurline()
  if g:clap_enable_icon
    let curline = curline[4:]
  endif
  return s:smart_concatenate(s:current_dir, curline)
endfunction

function! s:try_go_to_dir_is_ok() abort
  let input = g:clap.input.get()
  if input[-1:] ==# '/'
    if isdirectory(expand(input))
      call s:reset_to(expand(input))
      return v:true
    endif
  endif
  return v:false
endfunction

function! s:tab_action() abort
  if s:try_go_to_dir_is_ok()
    return
  endif

  if exists('g:__clap_has_no_matches') && g:__clap_has_no_matches
    return
  endif

  if has_key(s:filer_error_cache, s:current_dir)
    call g:clap.display.set_lines([s:filer_error_cache[s:current_dir]])
    return
  endif

  if has_key(s:filer_empty_cache, s:current_dir)
    if g:clap.display.get_lines() != [s:DIRECTORY_IS_EMPTY]
      call g:clap.display.set_lines([s:DIRECTORY_IS_EMPTY])
    endif
    return
  endif

  let current_entry = s:get_current_entry()
  if filereadable(current_entry)
    " TODO: preview file
    return ''
  endif

  call s:reset_to(current_entry)

  return ''
endfunction

function! s:smart_concatenate(cur_dir, curline) abort
  if a:cur_dir[-1:] ==# '/'
    return a:cur_dir.a:curline
  else
    return a:cur_dir.'/'.a:curline
  endif
endfunction

function! s:filer_sink(selected) abort
  if g:clap_enable_icon
    let curline = a:selected[4:]
  else
    let curline = a:selected
  endif
  execute 'edit' s:smart_concatenate(s:current_dir, curline)
endfunction

function! s:filer_on_typed() abort
  " <Tab> and <Backspace> also trigger the CursorMoved event.
  " s:filter_or_send_message() is already handled in tab and bs action,
  " on_typed handler only needs to take care of the filtering.
  if exists('s:filer_cache') && has_key(s:filer_cache, s:current_dir)
    call clap#highlight#clear()
    call s:do_filter()
  endif
  return ''
endfunction

function! s:filer_on_no_matches(input) abort
  execute 'edit' a:input
endfunction

function! s:start_rpc_service() abort
  let s:filer_cache = {}
  let s:filer_error_cache = {}
  let s:filer_empty_cache = {}
  let s:last_request_id = 0
  if !empty(g:clap.provider.args) && isdirectory(expand(g:clap.provider.args[0]))
    let target_dir = g:clap.provider.args[0]
    if target_dir[-1:] ==# '/'
      let s:current_dir = expand(target_dir)
    else
      let s:current_dir = expand(target_dir).'/'
    endif
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
