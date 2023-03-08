" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Grep on the fly using vim-clap fuzzy matcher.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:grep = {}

function! s:grep.init() abort
  let cwd = clap#rooter#working_dir()
  call clap#client#notify_on_init({'cwd': cwd})
endfunction

let s:grep.sink = g:clap#provider#live_grep#.sink
let s:grep['sink*'] = g:clap#provider#live_grep#['sink*']
let s:grep.on_move = g:clap#provider#live_grep#.on_move
let s:grep.on_move_async = function('clap#impl#on_move#async')
let s:grep.on_typed = { -> clap#client#notify_provider('on_typed') }
let s:grep.enable_rooter = v:true
let s:grep.support_open_action = v:true
let s:grep.icon = 'Grep'
let s:grep.syntax = 'clap_grep'

let g:clap#provider#grep# = s:grep

let &cpoptions = s:save_cpo
unlet s:save_cpo
