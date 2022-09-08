" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Grep on the fly with cache and dynamic results.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:grep2 = {}

if !executable('rg')
  call clap#helper#echo_error('grep2 provider can not work without the executable rg.')
  finish
endif

function! s:grep2.on_typed()
  call clap#client#notify('on_typed')
endfunction

function! s:grep2.init() abort
  call clap#client#notify_on_init()
endfunction

let s:grep2.sink = g:clap#provider#grep#.sink
let s:grep2['sink*'] = g:clap#provider#grep#['sink*']
let s:grep2.on_move = g:clap#provider#grep#.on_move
let s:grep2.on_move_async = function('clap#impl#on_move#async')
let s:grep2.enable_rooter = v:true
let s:grep2.support_open_action = v:true
let s:grep2.icon = 'Grep'
let s:grep2.syntax = 'clap_grep'

let g:clap#provider#grep2# = s:grep2

let &cpoptions = s:save_cpo
unlet s:save_cpo
