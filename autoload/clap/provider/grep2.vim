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
  call clap#filter#async#dyn#start_grep()
endfunction

function! s:grep2.init() abort
  let g:__clap_match_scope_enum = 'GrepLine'
  call clap#rooter#try_set_cwd()
  if g:__clap_development
    call clap#client#notify_on_init('on_init')
  else
    call clap#job#regular#forerunner#start_command(clap#maple#command#ripgrep_forerunner())
  endif
endfunction

function! s:grep2.exit() abort
  unlet g:__clap_match_scope_enum
endfunction

let s:grep2.sink = g:clap#provider#grep#.sink
let s:grep2['sink*'] = g:clap#provider#grep#['sink*']
let s:grep2.on_move = g:clap#provider#grep#.on_move
let s:grep2.on_move_async = function('clap#impl#on_move#async')
let s:grep2.enable_rooter = v:true
let s:grep2.support_open_action = v:true
let s:grep2.syntax = 'clap_grep'

let g:clap#provider#grep2# = s:grep2

let &cpoptions = s:save_cpo
unlet s:save_cpo
