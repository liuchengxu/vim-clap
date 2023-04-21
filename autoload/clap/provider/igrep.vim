" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Grep using the filer-like interface.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:igrep = {}

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

" APIs used by Rust backend.
function! clap#provider#igrep#handle_on_initialize(result) abort
  let result = a:result
  call g:clap.display.set_lines(result.entries)
  call clap#sign#reset_to_first_line()
  call clap#indicator#update_processed(result.total)
  call clap#sign#reset_to_first_line()
  call g:clap#display_win.shrink_if_undersize()
endfunction

function! clap#provider#igrep#handle_error(error) abort
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

function! clap#provider#igrep#set_prompt(current_dir) abort
  let current_dir = a:current_dir[-1:] ==# s:PATH_SEPERATOR ? a:current_dir : a:current_dir.s:PATH_SEPERATOR
  call s:set_prompt(current_dir)
endfunction

function! clap#provider#igrep#handle_special_entries(abs_path) abort
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

function! clap#provider#igrep#set_create_file_entry() abort
  call clap#highlight#clear()
  let input = g:clap.input.get()
  let create_file_line = (g:clap_enable_icon ? ' ' : '') . input . s:CREATE_FILE
  call g:clap.display.set_lines([create_file_line])
endfunction

function! s:handle_mapping_bs() abort
  call clap#client#notify_provider('backspace')
  return ''
endfunction

function! s:get_entry_by_line(line) abort
  let curline = a:line
  if g:clap_enable_icon
    let curline = curline[4:]
  endif
  let curline = substitute(curline, '\V' . s:CREATE_FILE, '', '')
  return s:smart_concatenate(s:current_dir, curline)
endfunction

function! s:smart_concatenate(cur_dir, curline) abort
  if a:cur_dir[-1:] ==# s:PATH_SEPERATOR
    return a:cur_dir.a:curline
  else
    return a:cur_dir.s:PATH_SEPERATOR.a:curline
  endif
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

function! s:igrep_on_no_matches(input) abort
  execute 'edit' s:smart_concatenate(s:current_dir, a:input)
endfunction

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

let s:igrep.init = function('s:start_rpc_service')
let s:igrep.sink = function('s:igrep_sink')
let s:igrep.icon = 'File'
let s:igrep.syntax = 'clap_grep'
let s:igrep.on_typed = { -> clap#client#notify_provider('on_typed') }
let s:igrep.back_action = { -> clap#client#notify_provider('backspace') }
let s:igrep.mappings = {
      \ "<Tab>": { ->  clap#client#notify_provider('tab') },
      \ "<CR>": { ->  clap#client#notify_provider('cr') },
      \ "<BS>": function('s:handle_mapping_bs'),
      \ }
let s:igrep.source_type = g:__t_rpc
let s:igrep.on_no_matches = function('s:igrep_on_no_matches')
let g:clap#provider#igrep# = s:igrep

let &cpoptions = s:save_cpo
unlet s:save_cpo
