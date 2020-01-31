" Author: Mark Wu <markplace@gmail.com>
" Description: Cache API for clap.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:path_separator = has('win32') ? '\' : '/'

function! s:default_cache_dir() abort
  if has('nvim')
    let user_cache = stdpath('cache')
  elseif exists('$XDG_CACHE_HOME')
    let user_cache = $XDG_CACHE_HOME
  else
    let user_cache = $HOME . s:path_separator . '.cache'
  endif
  return user_cache . s:path_separator . 'clap'
endfunction

if exists('g:clap_cache_directory')
  let s:clap_cache_directory = expand(g:clap_cache_directory)
else
  let s:clap_cache_directory = s:default_cache_dir()
endif

function! clap#cache#directory() abort
  if !isdirectory(s:clap_cache_directory)
    call mkdir(s:clap_cache_directory, 'p')
  endif

  return s:clap_cache_directory
endf

function! clap#cache#location_for(provider_id, fname) abort
  if empty(a:provider_id)
    call clap#helper#echo_error('provider_id can not be empty.')
    return v:null
  endif

  let provider_cache_directory = clap#cache#directory() . s:path_separator . a:provider_id

  if !isdirectory(provider_cache_directory)
    call mkdir(provider_cache_directory, 'p')
  endif

  return provider_cache_directory . s:path_separator . a:fname
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
