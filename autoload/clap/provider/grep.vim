" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Grep on the fly using vim-clap fuzzy matcher.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:grep = {}

if !executable('rg')
  call clap#helper#echo_error('grep provider can not work without the executable rg.')
  finish
endif

function! s:grep.on_typed() abort
  call clap#filter#async#dyn#start_grep()
endfunction

function! s:grep.init() abort
  let g:__clap_match_scope_enum = 'GrepLine'
  call clap#rooter#try_set_cwd()
  if g:__clap_development
    call clap#client#call_on_init('on_init', v:null, clap#client#init_params(v:null))
  else
    call clap#job#regular#forerunner#start_command(clap#maple#command#ripgrep_forerunner())
  endif
endfunction

function! s:grep.exit() abort
  unlet g:__clap_match_scope_enum
endfunction

let s:grep.sink = g:clap#provider#live_grep#.sink
let s:grep['sink*'] = g:clap#provider#live_grep#['sink*']
let s:grep.on_move = g:clap#provider#live_grep#.on_move
let s:grep.on_move_async = function('clap#impl#on_move#async')
let s:grep.enable_rooter = v:true
let s:grep.support_open_action = v:true
let s:grep.syntax = 'clap_grep'

let g:clap#provider#grep# = s:grep

let &cpoptions = s:save_cpo
unlet s:save_cpo
