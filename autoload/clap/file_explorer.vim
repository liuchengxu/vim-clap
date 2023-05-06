" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Common utilities for filer-like providers.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:PATH_SEPERATOR = has('win32') && !(exists('+shellslash') && &shellslash) ? '\' : '/'
let s:DIRECTORY_IS_EMPTY = (g:clap_enable_icon ? '  ' : '').'<Empty directory>'
let s:CREATE_FILE = ' [Create new file]'

if has('win32')
  function! s:normalize_path_sep(path) abort
    return substitute(a:path, '[/\\]',s:PATH_SEPERATOR, 'g')
  endfunction

  function! s:is_root_directory(dir) abort
    return a:dir =~? '^\([a-z]:\|\(\\\\\|\/\/\)[^\\\/]\+\(\\\|\/\/\)[^\\\/]\+\)\(\\\|\/\)\+$'
  endfunction
else
  function! s:normalize_path_sep(path) abort
    return a:path
  endfunction

  function! s:is_root_directory(dir) abort
    return a:dir ==# s:PATH_SEPERATOR
  endfunction
endif

function! clap#file_explorer#init_current_dir() abort
  if empty(g:clap.provider.args)
    let current_dir = getcwd()
    if current_dir[-1:] !=# s:PATH_SEPERATOR
      let current_dir = current_dir.s:PATH_SEPERATOR
    endif
    return current_dir
  endif

  let maybe_dir = g:clap.provider.args[0]
  " %:p:h, % is actually g:clap.start.bufnr
  if maybe_dir =~# '^%.\+'
    let m = matchstr(maybe_dir, '^%\zs\(.*\)')
    let target_dir = fnamemodify(bufname(g:clap.start.bufnr), m)
  elseif isdirectory(expand(maybe_dir))
    let target_dir = maybe_dir
  else
    let current_dir = getcwd()
    if current_dir[-1:] !=# s:PATH_SEPERATOR
      let current_dir = current_dir.s:PATH_SEPERATOR
    endif
    return
  endif

  let target_dir = s:normalize_path_sep(expand(target_dir))
  if target_dir[-1:] ==# s:PATH_SEPERATOR
    let current_dir = target_dir
  else
    let current_dir = target_dir.s:PATH_SEPERATOR
  endif

  return current_dir
endfunction

function! clap#file_explorer#join(cur_dir, curline) abort
  if a:cur_dir[-1:] ==# s:PATH_SEPERATOR
    return a:cur_dir.a:curline
  else
    return a:cur_dir.s:PATH_SEPERATOR.a:curline
  endif
endfunction

function! clap#file_explorer#handle_special_entries(abs_path) abort
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

" APIs used by Rust backend.
function! clap#file_explorer#handle_on_initialize(result) abort
  let result = a:result
  call g:clap.display.set_lines(result.entries)
  call clap#sign#reset_to_first_line()
  call clap#indicator#update_processed(result.total)
  call clap#sign#reset_to_first_line()
  call g:clap#display_win.shrink_if_undersize()
endfunction

function! clap#file_explorer#set_prompt(current_dir, winwidth) abort
  let current_dir = a:current_dir[-1:] ==# s:PATH_SEPERATOR ? a:current_dir : a:current_dir.s:PATH_SEPERATOR
  let cwd = getcwd()
  if stridx(current_dir, cwd) == 0
    let current_dir = '.' . current_dir[len(cwd):]
  else
    let current_dir = fnamemodify(current_dir, ':~')
  end
  if strlen(current_dir) < a:winwidth * 3 / 4
    call clap#spinner#set(current_dir)
  else
    let parent = fnamemodify(current_dir, ':p:h')
    let last = fnamemodify(current_dir, ':p:t')
    let short_dir = pathshorten(parent).s:PATH_SEPERATOR.last
    if strlen(short_dir) < a:winwidth * 3 / 4
      call clap#spinner#set(short_dir)
    else
      call clap#spinner#set(pathshorten(current_dir))
    endif
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
