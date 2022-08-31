" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Python and Rust implementation of fzy filter algorithm.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:py_exe = has('python3') ? 'python3' : 'python'
let s:pyfile = has('python3') ? 'py3file' : 'pyfile'
let s:plugin_root_dir = fnamemodify(g:clap#autoload_dir, ':h')

if has('win32')
  let s:LIB = '\pythonx\clap\fuzzymatch_rs.pyd'
  let s:SETUP_PY = '\setup_python.py'
else
  let s:LIB = '/pythonx/clap/fuzzymatch_rs.so'
  let s:SETUP_PY = '/setup_python.py'
endif

" Import pythonx/clap
if !has('nvim')
  execute s:pyfile s:plugin_root_dir.s:SETUP_PY
endif

let s:has_py_dynamic_module = filereadable(s:plugin_root_dir.s:LIB)
let s:using_dynamic_module = v:false

" For test only
if get(g:, 'clap_use_pure_python', 0)
  let s:py_fn = 'clap_fzy_py'
else
  if s:has_py_dynamic_module
    let s:py_fn = 'clap_fzy_rs'
    let s:using_dynamic_module = v:true
  else
    let s:py_fn = 'clap_fzy_py'
  endif
endif

try
  execute s:py_exe 'from clap.fzy import' s:py_fn
catch
  let s:using_dynamic_module = v:false
endtry

function! clap#filter#sync#python#has_dynamic_module() abort
  return s:has_py_dynamic_module
endfunction

if s:using_dynamic_module
  " Rust dynamic module has the feature of truncating the long lines to make fuzzy matched items visible.
  function! clap#filter#sync#python#(query, candidates, recent_files, context) abort
    " If the query is empty, neovim and vim's python client might crash.
    if a:query ==# ''
      return a:candidates
    endif
    " Possibly a bug of Neovim's Python provider, the result from Python can be null once passed to the vim side.
    let [g:__clap_fuzzy_matched_indices, filtered, g:__clap_lines_truncated_map] = pyxeval(s:py_fn.'()')
    return filtered
  endfunction
else
  function! clap#filter#sync#python#(query, candidates, _recent_files, context) abort
    let [g:__clap_fuzzy_matched_indices, filtered] = pyxeval(s:py_fn.'()')
    return filtered
  endfunction
endif

let &cpoptions = s:save_cpo
unlet s:save_cpo
