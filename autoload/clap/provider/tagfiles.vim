" Author: romgrk <romgrk.cc@gmail.com>
" Description: Project-wide tags

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:provider = {}

function! s:provider.on_typed() abort
  if exists('g:__clap_forerunner_tempfile')
    call clap#filter#async#dyn#from_tempfile(g:__clap_forerunner_tempfile)
  elseif exists('g:__clap_forerunner_result')
    let query = g:clap.input.get()
    if query ==# ''
      call g:clap.display.set_lines([])
      return
    endif
    call clap#filter#on_typed(function('clap#filter#sync'), query, g:__clap_forerunner_result)
  else
    let args = clap#maple#global_opts()
    let args += ['tagfiles', g:clap.input.get()]
    let args += map(tagfiles(), {_, f -> '--files=' . f})
    let cmd = call(function('clap#maple#build_cmd'), args)
    call clap#filter#async#dyn#start_directly(cmd)
  endif
endfunction

function! s:provider.init() abort
  let g:__clap_builtin_line_splitter_enum = 'TagNameOnly'
  if clap#maple#is_available()
    call clap#rooter#try_set_cwd()
    call clap#job#regular#forerunner#start_command(
            \ clap#maple#tagfiles_forerunner_command())
  endif
endfunction

function! s:extract(tag_row) abort
  let name = trim(matchstr(a:tag_row, '\v^\zs(.*)\ze\s+\[\f+\]$'))
  let file = trim(matchstr(a:tag_row, '\v^.*\[\zs\f+\ze\]$'))
  return [name, file]
endfunction

function! s:provider.sink(selected) abort
  let [name, file] = s:extract(a:selected)
  execute 'tag' name
endfunction

function! s:provider.on_move() abort
  let [lnum, path] = s:extract(g:clap.display.getcurline())
  call clap#preview#file_at(path, lnum)
endfunction

function! s:provider.on_exit() abort
  if exists('g:__clap_builtin_line_splitter_enum')
    unlet g:__clap_builtin_line_splitter_enum
  endif
endfunction

let s:provider.enable_rooter = v:true
let s:provider.support_open_action = v:true
let s:provider.syntax = 'clap_tagfiles'

let g:clap#provider#tagfiles# = s:provider

let &cpoptions = s:save_cpo
unlet s:save_cpo
