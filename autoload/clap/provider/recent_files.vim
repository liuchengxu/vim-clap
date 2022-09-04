" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Persistent recent files, ordered by the Mozilla's Frecency algorithm.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:recent_files = {}

function! s:recent_files.on_typed() abort
  call clap#client#notify('on_typed')
endfunction

function! s:recent_files.on_move_async() abort
  call clap#client#notify('on_move')
endfunction

function! s:recent_files.init() abort
  call clap#client#notify_on_init()
endfunction

let s:recent_files.sink = function('clap#provider#files#sink_impl')
let s:recent_files.support_open_action = v:true
let s:recent_files.icon = 'File'
let s:recent_files.syntax = 'clap_files'

let g:clap#provider#recent_files# = s:recent_files

let &cpoptions = s:save_cpo
unlet s:save_cpo
