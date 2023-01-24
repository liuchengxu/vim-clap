" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Ivy-like file explorer.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:filer = {}

let s:PATH_SEPERATOR = has('win32') && !(exists('+shellslash') && &shellslash) ? '\' : '/'
let s:DIRECTORY_IS_EMPTY = (g:clap_enable_icon ? '  ' : '').'Directory is empty'
let s:CREATE_FILE = ' [Create new file]'

function! clap#provider#filer#hi_empty_dir() abort
  syntax match ClapEmptyDirectory /^.*Directory is empty/
  hi default link ClapEmptyDirectory WarningMsg
endfunction

function! clap#provider#filer#handle_on_initialize(result) abort
  let result = a:result
  call g:clap.display.set_lines(result.entries)
  call clap#sign#reset_to_first_line()
  call clap#indicator#update_processed(result.total)
  call clap#sign#reset_to_first_line()
  call g:clap#display_win.shrink_if_undersize()
endfunction

function! clap#provider#filer#handle_error(error) abort
  call g:clap.preview.show([a:error])
endfunction

function! s:set_prompt(current_dir) abort
  let current_dir = a:current_dir
  let cwd = getcwd()
  if stridx(current_dir, cwd) == 0
    let current_dir = '.' . current_dir[len(cwd):]
  else
    let current_dir = fnamemodify(current_dir, ':~')
  end
  if strlen(current_dir) < s:winwidth * 3 / 4
    call clap#spinner#set(current_dir)
  else
    let parent = fnamemodify(current_dir, ':p:h')
    let last = fnamemodify(current_dir, ':p:t')
    let short_dir = pathshorten(parent).s:PATH_SEPERATOR.last
    if strlen(short_dir) < s:winwidth * 3 / 4
      call clap#spinner#set(short_dir)
    else
      call clap#spinner#set(pathshorten(current_dir))
    endif
  endif
endfunction

function! clap#provider#filer#set_prompt(current_dir) abort
  let current_dir = a:current_dir[-1:] ==# s:PATH_SEPERATOR ? a:current_dir : a:current_dir.s:PATH_SEPERATOR
  call s:set_prompt(current_dir)
endfunction

if has('win32')
  function! s:is_root_directory(dir) abort
    return a:dir =~? '^\([a-z]:\|\(\\\\\|\/\/\)[^\\\/]\+\(\\\|\/\/\)[^\\\/]\+\)\(\\\|\/\)\+$'
  endfunction
else
  function! s:is_root_directory(dir) abort
    return a:dir ==# s:PATH_SEPERATOR
  endfunction
endif

if has('nvim')
  function! s:bs_action() abort
    call clap#client#notify('backspace')
    return ''
  endfunction
else
  function! s:bs_action(before_bs) abort
    call clap#client#notify('backspace')
    return ''
  endfunction
endif

function! s:build_create_file_line(input) abort
  return (g:clap_enable_icon ? ' ' : '') . a:input . s:CREATE_FILE
endfunction

function! s:get_entry_by_line(line) abort
  let curline = a:line
  if g:clap_enable_icon
    let curline = curline[4:]
  endif
  let curline = substitute(curline, '\V' . s:CREATE_FILE, '', '')
  return s:smart_concatenate(s:current_dir, curline)
endfunction

function! clap#provider#filer#handle_special_entries(abs_path) abort
  let curline = g:clap.display.getcurline()

  if curline =~# s:DIRECTORY_IS_EMPTY
    let input = g:clap.input.get()
    call clap#handler#sink_with({-> execute('edit '.a:abs_path)})
    return v:true
  endif

  if curline =~# s:CREATE_FILE
        \ || (g:clap.display.line_count() == 1 && g:clap.display.get_lines()[0] =~# s:CREATE_FILE)
    " Create file if it doesn't exist
    stopinsert
    call clap#handler#sink_with({-> execute('edit '.a:abs_path)})
    return v:true
  endif

  return v:false
endfunction

function! s:smart_concatenate(cur_dir, curline) abort
  if a:cur_dir[-1:] ==# s:PATH_SEPERATOR
    return a:cur_dir.a:curline
  else
    return a:cur_dir.s:PATH_SEPERATOR.a:curline
  endif
endfunction

function! s:filer_sink(selected) abort
  execute 'edit' fnameescape(s:get_entry_by_line(a:selected))
endfunction

function! clap#provider#filer#sink(entry) abort
  call clap#handler#sink_with({ -> execute('edit '.fnameescape(a:entry))})
endfunction

function! clap#provider#filer#set_create_file_entry() abort
  call clap#highlight#clear()
  call g:clap.display.set_lines([s:build_create_file_line(g:clap.input.get())])
endfunction

function! s:filer.on_move_async() abort
  if stridx(g:clap.display.getcurline(), s:CREATE_FILE) > -1
    call g:clap.preview.hide()
    return
  endif
  call clap#client#notify('on_move')
endfunction

function! s:filer_on_no_matches(input) abort
  execute 'edit' s:smart_concatenate(s:current_dir, a:input)
endfunction

if has('win32')
  function! s:normalize_path_sep(path) abort
    return substitute(a:path, '[/\\]',s:PATH_SEPERATOR, 'g')
  endfunction
else
  function! s:normalize_path_sep(path) abort
    return a:path
  endfunction
endif

function! s:set_initial_current_dir() abort
  if empty(g:clap.provider.args)
    let s:current_dir = getcwd()
    if s:current_dir[-1:] !=# s:PATH_SEPERATOR
      let s:current_dir = s:current_dir.s:PATH_SEPERATOR
    endif
    return
  endif

  let maybe_dir = g:clap.provider.args[0]
  " %:p:h, % is actually g:clap.start.bufnr
  if maybe_dir =~# '^%.\+'
    let m = matchstr(maybe_dir, '^%\zs\(.*\)')
    let target_dir = fnamemodify(bufname(g:clap.start.bufnr), m)
  elseif isdirectory(expand(maybe_dir))
    let target_dir = maybe_dir
  else
    let s:current_dir = getcwd()
    if s:current_dir[-1:] !=# s:PATH_SEPERATOR
      let s:current_dir = s:current_dir.s:PATH_SEPERATOR
    endif
    return
  endif

  let target_dir = s:normalize_path_sep(expand(target_dir))
  if target_dir[-1:] ==# s:PATH_SEPERATOR
    let s:current_dir = target_dir
  else
    let s:current_dir = target_dir.s:PATH_SEPERATOR
  endif
endfunction

function! s:start_rpc_service() abort
  let s:winwidth = winwidth(g:clap.display.winid)
  call s:set_initial_current_dir()
  call s:set_prompt(s:current_dir)
  call clap#client#notify_on_init({'cwd': s:current_dir})
endfunction

let s:filer.init = function('s:start_rpc_service')
let s:filer.sink = function('s:filer_sink')
let s:filer.icon = 'File'
let s:filer.syntax = 'clap_filer'
let s:filer.on_typed = { -> clap#client#notify('on_typed') }
let s:filer.bs_action = function('s:bs_action')
let s:filer.back_action = { -> clap#client#notify('backspace') }
let s:filer.tab_action = { -> clap#client#notify('tab') }
let s:filer.cr_action = { -> clap#client#notify('cr') }
let s:filer.source_type = g:__t_rpc
let s:filer.on_no_matches = function('s:filer_on_no_matches')
let g:clap#provider#filer# = s:filer

let &cpoptions = s:save_cpo
unlet s:save_cpo
