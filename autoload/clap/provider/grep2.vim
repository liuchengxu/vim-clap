" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Grep on the fly with cache and dynamic results.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:grep2 = {}

function! s:grep2.on_typed()
  if exists('g:__clap_forerunner_tempfile')
    call clap#filter#async#dyn#grep_from_cache(g:__clap_forerunner_tempfile)
  elseif exists('g:__clap_forerunner_result')
    let query = g:clap.input.get()
    if query ==# ''
      return
    endif
    call clap#filter#on_typed(function('clap#filter#sync'), query, g:__clap_forerunner_result)
  else
    call clap#filter#async#dyn#start_grep()
  endif
endfunction

function! s:grep2.init() abort
  let g:__clap_builtin_line_splitter_enum = 'GrepExcludeFilePath'
  call clap#provider#grep#inject_icon_appended(g:clap_enable_icon)
  if clap#maple#is_available()
    call clap#rooter#try_set_cwd()
    call clap#job#regular#forerunner#start_command(clap#maple#ripgrep_forerunner_command())
  endif
endfunction

function! s:grep2.exit() abort
  unlet g:__clap_builtin_line_splitter_enum
endfunction

let s:grep2.sink = g:clap#provider#grep#.sink
let s:grep2['sink*'] = g:clap#provider#grep#['sink*']
let s:grep2.on_move = g:clap#provider#grep#.on_move
let s:grep2.enable_rooter = v:true
let s:grep2.support_open_action = v:true
let s:grep2.syntax = 'clap_grep'

let g:clap#provider#grep2# = s:grep2

let &cpoptions = s:save_cpo
unlet s:save_cpo
