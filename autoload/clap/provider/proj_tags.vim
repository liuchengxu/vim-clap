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

if g:__clap_development
  function! s:proj_tags.on_typed() abort
    call clap#client#call('on_typed', v:null, {'query': g:clap.input.get()})
  endfunction
else
  function! s:proj_tags.on_typed() abort
    if exists('g:__clap_forerunner_tempfile')
      call clap#filter#async#dyn#from_tempfile(g:__clap_forerunner_tempfile)
    else
      call clap#filter#async#dyn#start_directly(
            \ clap#maple#build_cmd('ctags', 'recursive-tags --dir', clap#rooter#working_dir(), '--query', g:clap.input.get()))
    endif
  endfunction

  function! s:proj_tags.init() abort
    let g:__clap_match_type_enum = 'TagName'
    if clap#maple#is_available()
      call clap#rooter#try_set_cwd()
      call clap#job#regular#forerunner#start_command(clap#maple#command#tags(v:true))
    endif
  endfunction
endif

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
