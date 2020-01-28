" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Filter out the candidate lines synchorously given the input.

let s:save_cpo = &cpoptions
set cpoptions&vim

if has('python3') || has('python')
  let s:py_exe = has('python3') ? 'python3' : 'python'
  let s:pyfile = has('python3') ? 'py3file' : 'pyfile'
else
  let s:py_exe = v:null
endif

let s:can_use_python = v:false
let s:has_py_dynamic_module = v:false

if s:py_exe !=# v:null

  try
    if has('win32')
      let s:LIB = '\pythonx\clap\fuzzymatch_rs.pyd'
      let s:SETUP_PY = '\setup_python.py'
    else
      let s:LIB = '/pythonx/clap/fuzzymatch_rs.so'
      let s:SETUP_PY = '/setup_python.py'
    endif

    let s:plugin_root_dir = fnamemodify(g:clap#autoload_dir, ':h')

    " Import pythonx/clap
    if !has('nvim')
      execute s:pyfile s:plugin_root_dir.s:SETUP_PY
    endif

    let s:has_py_dynamic_module = filereadable(s:plugin_root_dir.s:LIB)

    " For test only
    if get(g:, 'clap_use_pure_python', 0)
      let s:py_fn = 'clap_fzy_py'
    else
      let s:py_fn = s:has_py_dynamic_module ? 'clap_fzy_rs' : 'clap_fzy_py'
    endif

    execute s:py_exe 'from clap.fzy import' s:py_fn

    function! clap#filter#benchmark(query, candidates) abort
      return s:ext_filter(a:query, a:candidates)
    endfunction

    function! s:ext_filter(query, candidates) abort
      let [g:__clap_fuzzy_matched_indices, filtered] = pyxeval(s:py_fn.'()')
      return filtered
    endfunction

    let s:can_use_python = v:true
  catch
    call clap#helper#echo_error(v:exception)
  endtry
endif

function! clap#filter#has_py_dynamic_module() abort
  return s:has_py_dynamic_module
endfunction

if exists('g:clap_builtin_fuzzy_filter_threshold')
  let s:builtin_filter_capacity = g:clap_builtin_fuzzy_filter_threshold
elseif s:has_py_dynamic_module
  let s:builtin_filter_capacity = 100000
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
  function! clap#filter#(query, candidates) abort
    try
      return s:ext_filter(a:query, a:candidates)
    catch
      call clap#helper#echo_error(v:exception)
      return clap#filter#viml#(a:query, a:candidates)
    endtry
  endfunction
else
  function! clap#filter#(query, candidates) abort
    return clap#filter#viml#(a:query, a:candidates)
  endfunction
endif

function! clap#filter#on_typed(FilterFn, query, candidates) abort
  let l:lines = a:FilterFn(a:query, a:candidates)

  if empty(l:lines)
    let l:lines = [g:clap_no_matches_msg]
    let g:__clap_has_no_matches = v:true
    call g:clap.display.set_lines_lazy(lines)
    " In clap#impl#refresh_matches_count() we reset the sign to the first line,
    " But the signs are seemingly removed when setting the lines, so we should
    " postpone the sign update.
    call clap#impl#refresh_matches_count('0')
    call g:clap.preview.hide()
  else
    let g:__clap_has_no_matches = v:false
    call g:clap.display.set_lines_lazy(lines)
    call clap#impl#refresh_matches_count(string(len(l:lines)))
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
