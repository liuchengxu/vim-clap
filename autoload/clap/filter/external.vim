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
  " Need https://github.com/lotabout/skim/commit/7c6211fa7e657441cb9da70962258d4f115ad943
  for ext in ['fzy', 'fzf', 'sk']
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

" Get explicit externalfilter option.
function! s:get_external_filter() abort
  if has_key(g:clap.context, 'externalfilter')
    let s:cur_ext_filter = g:clap.context.externalfilter
  elseif has_key(g:clap.context, 'ef')
    let s:cur_ext_filter = g:clap.context.ef
  else
    let s:cur_ext_filter = v:null
  endif
  return s:cur_ext_filter
endfunction

function! s:cmd_of(ext_filter) abort
  if a:ext_filter ==# 'maple'
    return clap#maple#filter_subcommand(g:clap.input.get())
  else
    return printf(s:ext_cmd[a:ext_filter], g:clap.input.get())
  endif
endfunction

function! clap#filter#external#has_default() abort
  return s:default_ext_filter isnot v:null
endfunction

function! s:default_external_cmd() abort
  if s:default_ext_filter is v:null
    call g:clap.abort('No external filter available')
    return v:null
  endif

  let s:cur_ext_filter = s:default_ext_filter
  return s:cmd_of(s:cur_ext_filter)
endfunction

" Filter using the external tools given the current input.
function! clap#filter#external#get_cmd_or_default() abort
  let external_filter = s:get_external_filter()

  if external_filter isnot v:null
    return s:cmd_of(external_filter)
  endif

  return s:default_external_cmd()
endfunction

function! clap#filter#external#using_maple() abort
  return s:cur_ext_filter ==# 'maple'
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
