" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Filter out the candidate lines synchorously given the input.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:pattern_builder = {}

let s:default_ext_filter = v:null

if has('python3') || has('python')
  let s:py_exe = has('python3') ? 'python3' : 'python'
  let s:pyfile = has('python3') ? 'py3file' : 'pyfile'
else
  let s:py_exe = v:null
endif

let s:ext_cmd = {}

" Use "%s" instead of bare %s in case of the query containing ';',
" e.g., rg --files | maple hello;world, world can be misinterpreted as a
" command.
let s:ext_cmd.fzy = 'fzy --show-matches="%s"'
let s:ext_cmd.fzf = 'fzf --filter="%s"'
let s:ext_cmd.sk = 'sk --filter="%s"'

function! s:other_fuzzy_ext_filter() abort
  " TODO support skim, skim seems to have a score at the beginning.
  for ext in ['fzy', 'fzf']
    if executable(ext)
      return ext
    endif
  endfor
  return v:null
endfunction

if exists('g:clap_default_external_filter')
  let s:default_ext_filter = g:clap_default_external_filter
  if index(keys(s:ext_cmd), s:default_ext_filter) == -1
    call g:clap.abort('Unsupported external filter: '.s:default_ext_filter)
  endif
elseif clap#maple#is_available()
  let s:default_ext_filter = 'maple'
else
  let s:default_ext_filter = s:other_fuzzy_ext_filter()
endif

function! clap#filter#using_maple() abort
  return s:default_ext_filter ==# 'maple'
endfunction

function! s:pattern_builder._force_case() abort
  " Smart case
  if self.input =~? '\u'
    return '\C'
  else
    return '\c'
  endif
endfunction

function! s:pattern_builder.smartcase() abort
  let l:_force_case = self._force_case()
  let s:matchadd_pattern = l:_force_case.self.input
  return l:_force_case.self.input
endfunction

function! s:pattern_builder.substring() abort
  let l:_force_case = self._force_case()
  let l:filter_pattern = ['\V\^', l:_force_case]
  let s:matchadd_pattern = []
  for l:s in split(self.input)
    call add(filter_pattern, printf('\.\*\zs%s\ze', l:s))
    " FIXME can not distinguish `f f` highlight
    " these two f should be highlighed with different colors
    call add(s:matchadd_pattern, l:_force_case.l:s)
  endfor
  return join(l:filter_pattern, '')
endfunction

function! s:pattern_builder.build() abort
  if stridx(self.input, ' ') != -1
    return self.substring()
  else
    return self.smartcase()
  endif
endfunction

" Return substring pattern or the smartcase input pattern.
function! clap#filter#matchadd_pattern() abort
  return get(s:, 'matchadd_pattern', '')
endfunction

function! clap#filter#has_external_default() abort
  return s:default_ext_filter isnot v:null
endfunction

" Get explicit externalfilter option.
function! s:get_external_filter() abort
  if has_key(g:clap.context, 'externalfilter')
    let s:cur_ext_filter = g:clap.context.externalfilter
  elseif has_key(g:clap.context, 'ef')
    let s:cur_ext_filter = g:clap.context.ef
  elseif s:default_ext_filter is v:null
    call g:clap.abort('No external filter available')
    return
  else
    let s:cur_ext_filter = s:default_ext_filter
  endif
  return printf(s:ext_cmd[s:cur_ext_filter], g:clap.input.get())
endfunction

function! s:filter(line, pattern) abort
  return a:line =~ a:pattern
endfunction

function! s:fallback_filter(query, candidates) abort
  let s:pattern_builder.input = a:query
  let l:filter_pattern = s:pattern_builder.build()
  return filter(copy(a:candidates), 's:filter(v:val, l:filter_pattern)')
endfunction

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
      return s:fallback_filter(a:query, a:candidates)
    endtry
  endfunction
else
  function! clap#filter#(query, candidates) abort
    return s:fallback_filter(a:query, a:candidates)
  endfunction
endif

let &cpoptions = s:save_cpo
unlet s:save_cpo
