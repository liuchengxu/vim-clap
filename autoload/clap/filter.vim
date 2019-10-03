" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Filter out the candidate lines given input.

let s:save_cpo = &cpo
set cpo&vim

let s:pattern_builder = {}

let s:default_ext_filter = v:null

let s:ext_cmd = {}
let s:ext_cmd.fzy = 'fzy --show-matches="%s"'
let s:ext_cmd.fzf = 'fzf --filter="%s"'
let s:ext_cmd.sk = 'sk --filter="%s"'

" TODO support skim, skim seems to have a score at the beginning.
for ext in ['fzy', 'fzf']
  if executable(ext)
    let s:default_ext_filter = ext
    break
  endif
endfor

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

function! clap#filter#matchadd_pattern() abort
  return s:matchadd_pattern
endfunction

function! clap#filter#get_external_cmd_or_default() abort
  if has_key(g:clap.context, 'externalfilter')
    let ext_filter = g:clap.context.externalfilter
  elseif has_key(g:clap.context, 'ef')
    let ext_filter = g:clap.context.ef
  elseif s:default_ext_filter is v:null
    call g:clap.abort("No external filter available")
    return
  else
    let ext_filter = s:default_ext_filter
  endif
  return printf(s:ext_cmd[ext_filter], g:clap.input.get())
endfunction

function! s:filter(line, pattern) abort
  return a:line =~ a:pattern
endfunction

function! clap#filter#(lines, input) abort
  let s:pattern_builder.input = a:input
  let l:filter_pattern = s:pattern_builder.build()
  return filter(a:lines, 's:filter(v:val, l:filter_pattern)')
endfunction

let &cpo = s:save_cpo
unlet s:save_cpo
