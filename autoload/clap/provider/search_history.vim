" Author: Mark Wu <markplace@gmail.com>
" Description: List the search history.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Derived from fzf.vim
function! s:search_history_source() abort
  return clap#common_history#source('/')
endfunction

function! s:search_history_sink(selected) abort
  call clap#common_history#sink('/', a:selected)
endfunction

let s:search_history = {}
let s:search_history.sink = function('s:search_history_sink')
let s:search_history.source = function('s:search_history_source')
let s:search_history.syntax = 'clap_command_history'

let g:clap#provider#search_history# = s:search_history

let &cpoptions = s:save_cpo
unlet s:save_cpo
