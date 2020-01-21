" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Use the external tools, e.g., fzf, fzy as filter.

let s:save_cpo = &cpoptions
set cpoptions&vim

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
  let s:ext_cmd.maple = clap#maple#filter_cmd_fmt()
else
  let s:default_ext_filter = s:other_fuzzy_ext_filter()
endif

function! clap#filter#external#using_maple() abort
  return s:cur_ext_filter ==# 'maple'
endfunction

function! clap#filter#external#get_cmd_or_default() abort
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
  if s:cur_ext_filter ==# 'maple'
    let g:__clap_maple_fuzzy_matched = []
    let Provider = g:clap.provider._()
  endif
  return printf(s:ext_cmd[s:cur_ext_filter], g:clap.input.get())
endfunction

function! clap#filter#external#has_default() abort
  return s:default_ext_filter isnot v:null
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
