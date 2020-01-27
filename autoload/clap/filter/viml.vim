" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Native VimL implementation of filter.
" Used when there is no +python3 and external binary.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:pattern_builder = {}

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

function! s:filter(line, pattern) abort
  return a:line =~ a:pattern
endfunction

" Return substring pattern or the smartcase input pattern.
function! clap#filter#viml#matchadd_pattern() abort
  return get(s:, 'matchadd_pattern', '')
endfunction

function! clap#filter#viml#(query, candidates) abort
  let s:pattern_builder.input = a:query
  let l:filter_pattern = s:pattern_builder.build()
  return filter(copy(a:candidates), 's:filter(v:val, l:filter_pattern)')
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
