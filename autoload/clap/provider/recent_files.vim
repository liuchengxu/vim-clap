" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Persistent recent files, ordered by the Mozilla's Frecency algorithm.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:recent_files = {}

let s:recent_files.init = { -> clap#client#notify_on_init() }
let s:recent_files.on_typed = { -> clap#client#notify_provider('on_typed') }
let s:recent_files.on_move_async = { -> clap#client#notify_provider('on_move') }
let s:recent_files.sink = function('clap#provider#files#sink_impl')
let s:recent_files.support_open_action = v:true
let s:recent_files.icon = 'File'
let s:recent_files.syntax = 'clap_files'

let g:clap#provider#recent_files# = s:recent_files

let &cpoptions = s:save_cpo
unlet s:save_cpo
