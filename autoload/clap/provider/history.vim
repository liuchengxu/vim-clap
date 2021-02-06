" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the open buffers and oldfiles in order.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:history = {}

function! s:raw_history() abort
  let history = uniq(map(
    \ filter([expand('%')], 'len(v:val)')
    \   + filter(map(clap#util#buflisted_sorted(v:false), 'bufname(v:val)'), 'len(v:val)')
    \   + filter(copy(v:oldfiles), "filereadable(fnamemodify(v:val, ':p'))"),
    \ 'fnamemodify(v:val, ":~:.")'))
  if exists('*g:ClapProviderHistoryCustomFilter')
    return filter(history, 'g:ClapProviderHistoryCustomFilter(v:val)')
  else
    return history
  endif
endfunction

function! s:all_files() abort
  if g:clap_enable_icon
    return map(s:raw_history(), 'clap#icon#for(v:val). " " .v:val')
  else
    return s:raw_history()
  endif
endfunction

function! s:history_sink(selected) abort
  let fpath = g:clap_enable_icon ? a:selected[4:] : a:selected
  call clap#sink#edit_with_open_action(fpath)
endfunction

let s:history.syntax = 'clap_files'
let s:history.sink = function('s:history_sink')
let s:history.on_move = function('clap#provider#files#on_move_impl')
let s:history.on_move_async = function('clap#impl#on_move#async')
let s:history.source = function('s:all_files')
let s:history.support_open_action = v:true

let g:clap#provider#history# = s:history

let &cpoptions = s:save_cpo
unlet s:save_cpo
