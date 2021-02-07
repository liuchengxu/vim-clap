" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Project-wide tags

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:proj_tags = {}

let s:support_json_format =
      \ len(filter(systemlist('ctags --list-features'), 'v:val =~# ''^json''')) > 0

function! clap#provider#proj_tags#support_json_format() abort
  return s:support_json_format
endfunction

if !s:support_json_format
  call clap#helper#echo_error('Ensure ctags executable is in your PATH and has the JSON output feature')
  finish
endif

function! s:proj_tags.on_typed() abort
  call clap#impl#on_typed#async_with_cmd(clap#maple#build_cmd('tags', g:clap.input.get(), clap#rooter#working_dir()))
endfunction

function! s:proj_tags.init() abort
  let g:__clap_match_type_enum = 'TagName'
  if clap#maple#is_available()
    call clap#rooter#try_set_cwd()
    call clap#job#regular#forerunner#start_command(clap#maple#command#tags(v:true))
  endif
endfunction

function! s:extract(tag_row) abort
  let lnum = matchstr(a:tag_row, '^.*:\zs\(\d\+\)')
  let path = matchstr(a:tag_row, '\[.*@\zs\(\f*\)\ze\]')
  return [lnum, path]
endfunction

function! s:proj_tags.sink(selected) abort
  let [lnum, path] = s:extract(a:selected)
  call clap#sink#open_file(path, lnum, 1)
endfunction

function! s:proj_tags.on_move() abort
  let [lnum, path] = s:extract(g:clap.display.getcurline())
  call clap#preview#file_at(path, lnum)
endfunction

function! s:proj_tags.on_exit() abort
  if exists('g:__clap_match_type_enum')
    unlet g:__clap_match_type_enum
  endif
endfunction

let s:proj_tags.on_move_async = function('clap#impl#on_move#async')
let s:proj_tags.enable_rooter = v:true
let s:proj_tags.support_open_action = v:true
let s:proj_tags.syntax = 'clap_proj_tags'

let g:clap#provider#proj_tags# = s:proj_tags

let &cpoptions = s:save_cpo
unlet s:save_cpo
