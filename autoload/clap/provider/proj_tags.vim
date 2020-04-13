" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Project-wise tags

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:proj_tags = {}

function! s:proj_tags.on_typed()
  if exists('g:__clap_forerunner_tempfile')
    call clap#filter#async#dyn#from_tempfile(g:__clap_forerunner_tempfile)
  elseif exists('g:__clap_forerunner_result')
    let query = g:clap.input.get()
    if query ==# ''
      return
    endif
    call clap#filter#on_typed(function('clap#filter#sync'), query, g:__clap_forerunner_result)
  else
    let cmd = clap#maple#build_cmd(printf('tags "%s" "%s"', g:clap.input.get(), clap#rooter#working_dir()))
    call clap#filter#async#dyn#start_directly(cmd)
  endif
endfunction

function! s:proj_tags.init() abort
  call clap#provider#grep#inject_icon_appended(g:clap_enable_icon)
  if clap#maple#is_available()
    call clap#rooter#try_set_cwd()
    call clap#forerunner#start_subcommand(clap#maple#tags_forerunner_subcommand())
  endif
endfunction

function! s:proj_tags.sink(selected) abort
  let lnum = matchstr(a:selected, '^.*:\zs\(\d\+\)')
  let path = matchstr(a:selected, '\t\zs\f*$')
  normal! m'
  execute 'edit' path
  call cursor(lnum, 1)
endfunction

" let s:proj_tags.on_move = g:clap#provider#tags#.on_move
let s:proj_tags.enable_rooter = v:true
let s:proj_tags.support_open_action = v:true
let s:proj_tags.syntax = 'clap_tags'

let g:clap#provider#proj_tags# = s:proj_tags

let &cpoptions = s:save_cpo
unlet s:save_cpo
