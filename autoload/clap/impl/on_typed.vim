" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Default `on_typed` implementation.

let s:save_cpo = &cpoptions
set cpoptions&vim

function! s:try_cache_and_then_run_dyn(DynRun) abort
  if exists('g:__clap_forerunner_tempfile')
    call clap#filter#async#dyn#from_tempfile(g:__clap_forerunner_tempfile)
  elseif exists('g:__clap_forerunner_result')
    let query = g:clap.input.get()
    if query ==# ''
      return
    endif
    call clap#filter#on_typed(function('clap#filter#sync'), query, g:__clap_forerunner_result)
  else
    call a:DynRun()
  endif
endfunction

function! s:async_dyn_start_grep() abort
  call clap#filter#async#dyn#start_grep()
endfunction

function! s:async_dyn_with_cmd() abort
  call clap#filter#async#dyn#start_directly(s:async_dyn_cmd)
endfunction

function! clap#impl#on_typed#async_grep() abort
  call s:try_cache_and_then_run_dyn(function('s:async_dyn_start_grep'))
endfunction

function! clap#impl#on_typed#async_with_cmd(cmd) abort
  let s:async_dyn_cmd = a:cmd
  call s:try_cache_and_then_run_dyn(function('s:async_dyn_with_cmd'))
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
