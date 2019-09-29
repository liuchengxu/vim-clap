" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Filter out the candidate lines given input.

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
    call add(s:matchadd_pattern, l:s)
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

function! clap#filter#matchadd_pattern() abort
  return s:matchadd_pattern
endfunction

function! s:filter(line, pattern) abort
  return a:line =~ a:pattern
endfunction

function! clap#filter#(lines, input) abort
  let s:pattern_builder.input = a:input
  let l:filter_pattern = s:pattern_builder.build()
  return filter(a:lines, 's:filter(v:val, l:filter_pattern)')
endfunction
