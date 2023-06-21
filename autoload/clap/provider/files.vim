" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the files.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:files = {}

function! s:into_filename(line) abort
  if g:clap_enable_icon && clap#maple#is_available()
    return a:line[4:]
  else
    return a:line
  endif
endfunction

function! clap#provider#files#sink_impl(selected) abort
  let fpath = s:into_filename(a:selected)
  call clap#sink#edit_with_open_action(fpath)
endfunction

function! clap#provider#files#sink_star_impl(lines) abort
  call clap#sink#open_quickfix(map(map(a:lines, 's:into_filename(v:val)'),
        \ '{'.
        \   '"filename": v:val,'.
        \   '"text": strftime("Modified %b,%d %Y %H:%M:%S", getftime(v:val))." ".getfperm(v:val)'.
        \ '}'))
endfunction

function! clap#provider#files#on_move_impl() abort
  call clap#preview#file(s:into_filename(g:clap.display.getcurline()))
endfunction

let s:files.sink = function('clap#provider#files#sink_impl')
let s:files['sink*'] = function('clap#provider#files#sink_star_impl')
let s:files.on_move = function('clap#provider#files#on_move_impl')
let s:files.on_move_async = function('clap#impl#on_move#async')
let s:files.on_typed = { -> clap#client#notify_provider('on_typed') }
let s:files.enable_rooter = v:true
let s:files.support_open_action = v:true
let s:files.icon = 'File'
let s:files.syntax = 'clap_files'

let g:clap#provider#files# = s:files

let &cpoptions = s:save_cpo
unlet s:save_cpo
