" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Filter out the candidate lines given input.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:pattern_builder = {}

let s:default_ext_filter = v:null

if has('python3') || has('python')
  let s:py_exe = has('python3') ? 'python3' : 'python'
else
  let s:py_exe = v:null
endif

let s:ext_cmd = {}
let s:ext_cmd.fzy = 'fzy --show-matches="%s"'
let s:ext_cmd.fzf = 'fzf --filter="%s"'
let s:ext_cmd.sk = 'sk --filter="%s"'

" Use "%s" instead of bare %s in case of the query containing ';',
" e.g., rg --files | maple hello;world, world can be misinterpreted as a
" command.
let s:maple_bin = fnamemodify(g:clap#autoload_dir, ':h').'/target/release/maple'

if exists('g:clap_default_external_filter')
  let s:default_ext_filter = g:clap_default_external_filter
  if index(keys(s:ext_cmd), s:default_ext_filter) == -1
    call g:clap.abort('Unsupported external filter: '.s:default_ext_filter)
  endif
elseif executable(s:maple_bin)
  let s:default_ext_filter = 'maple'
  let s:ext_cmd.maple = s:maple_bin.' "%s"'
elseif executable('maple')
  let s:default_ext_filter = 'maple'
  let s:ext_cmd.maple = 'maple "%s"'
else
  " TODO support skim, skim seems to have a score at the beginning.
  for ext in ['fzy', 'fzf']
    if executable(ext)
      let s:default_ext_filter = ext
      break
    endif
  endfor
endif

function! clap#filter#using_maple() abort
  return s:default_ext_filter == 'maple'
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

function! s:maple_converter(line) abort
  let json_decoded = json_decode(a:line)
  if exists('g:__clap_maple_fuzzy_matched')
    call add(g:__clap_maple_fuzzy_matched, json_decoded.indices)
  endif
  return json_decoded.text
endfunction

function! clap#filter#get_external_cmd_or_default() abort
  if has_key(g:clap.context, 'externalfilter')
    let ext_filter = g:clap.context.externalfilter
  elseif has_key(g:clap.context, 'ef')
    let ext_filter = g:clap.context.ef
  elseif s:default_ext_filter is v:null
    call g:clap.abort('No external filter available')
    return
  else
    let ext_filter = s:default_ext_filter
  endif
  if ext_filter ==# 'maple'
    let g:__clap_maple_fuzzy_matched = []
    let Provider = g:clap.provider._()
    if !has_key(Provider, 'converter')
      let Provider.converter = function('s:maple_converter')
    endif
  endif
  return printf(s:ext_cmd[ext_filter], g:clap.input.get())
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

if s:py_exe isnot v:null

  function! s:setup_python() abort
    if !has('nvim')
execute s:py_exe "<< EOF"
import sys
from os.path import normpath, join
import vim
plugin_root_dir = vim.eval('g:clap#autoload_dir')
python_root_dir = normpath(join(plugin_root_dir, '..', 'pythonx'))
sys.path.insert(0, python_root_dir)
import clap
EOF
    endif
  endfunction

  try
    call s:setup_python()

    if has('win32')
      let s:LIB = '\pythonx\clap\fuzzymatch_rs.pyd'
    else
      let s:LIB = '/pythonx/clap/fuzzymatch_rs.so'
    endif

    let s:has_rust_ext = filereadable(fnamemodify(g:clap#autoload_dir, ':h').s:LIB)
    " For test only
    if get(g:, 'clap_use_pure_python', 0)
      let s:py_fn = 'clap_fzy_py'
    else
      let s:py_fn = s:has_rust_ext ? 'clap_fzy_rs' : 'clap_fzy_py'
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

function! clap#filter#has_rust_ext() abort
  return get(s:, 'has_rust_ext', v:false)
endfunction

if s:can_use_python
  function! clap#filter#(query, candidates) abort
    try
      return s:ext_filter(a:query, a:candidates)
    catch
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
