" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Filter out the candidate lines synchorously given the input.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:can_use_python = v:false
let s:has_py_dynamic_module = v:false

if has('python3') || has('python')
  try
    let s:has_py_dynamic_module = clap#filter#sync#python#has_dynamic_module()
    let s:can_use_python = v:true
  catch
    call clap#helper#echo_error(v:exception)
  endtry
endif

if exists('g:clap_builtin_fuzzy_filter_threshold')
  let s:builtin_filter_capacity = g:clap_builtin_fuzzy_filter_threshold
elseif s:has_py_dynamic_module
  let s:builtin_filter_capacity = 30000
else
  let s:builtin_filter_capacity = 10000
endif

function! clap#filter#beyond_capacity(size) abort
  return a:size > s:builtin_filter_capacity
endfunction

function! clap#filter#capacity() abort
  return s:builtin_filter_capacity
endfunction

if s:can_use_python

  let s:related_builtin_providers = ['tags', 'buffers', 'files', 'git_files', 'history', 'filer']

  function! s:enable_icon() abort
    if g:clap_enable_icon
          \ && index(s:related_builtin_providers, g:clap.provider.id) > -1
      return v:true
    else
      return v:false
    endif
  endfunction

  function! s:line_splitter() abort
    return exists('g:__clap_builtin_line_splitter_enum') ? g:__clap_builtin_line_splitter_enum : 'Full'
  endfunction

  function! clap#filter#sync(query, candidates) abort
    try
      return clap#filter#sync#python#(a:query, a:candidates, winwidth(g:clap.display.winid), s:enable_icon(), s:line_splitter())
    catch
      call clap#helper#echo_error(v:exception.', throwpoint:'.v:throwpoint)
      return clap#filter#sync#viml#(a:query, a:candidates)
    endtry
  endfunction
else
  function! clap#filter#sync(query, candidates) abort
    return clap#filter#sync#viml#(a:query, a:candidates)
  endfunction
endif

function! clap#filter#on_typed(FilterFn, query, candidates) abort
  let l:lines = a:FilterFn(a:query, a:candidates)

  if empty(l:lines)
    let l:lines = [g:clap_no_matches_msg]
    let g:__clap_has_no_matches = v:true
    call g:clap.display.set_lines_lazy(lines)
    " In clap#state#refresh_matches_count() we reset the sign to the first line,
    " But the signs are seemingly removed when setting the lines, so we should
    " postpone the sign update.
    call clap#state#refresh_matches_count(0)
    call g:clap.preview.hide()
  else
    let g:__clap_has_no_matches = v:false
    call g:clap.display.set_lines_lazy(lines)
    call clap#state#refresh_matches_count(len(l:lines))
  endif

  call g:clap#display_win.shrink_if_undersize()
  call clap#spinner#set_idle()

  if !g:__clap_has_no_matches
    if exists('g:__clap_fuzzy_matched_indices')
      call clap#highlight#add_fuzzy_sync()
    else
      call g:clap.display.add_highlight()
    endif
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
