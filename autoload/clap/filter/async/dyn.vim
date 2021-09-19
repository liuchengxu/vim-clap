" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Dynamic update version of maple filter.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Currently this is not configurable.
let s:DYN_ITEMS_TO_SHOW = 40

function! s:handle_message(msg) abort
  if !g:clap.display.win_is_valid()
        \ || g:clap.input.get() !=# s:last_query
    return
  endif

  call clap#state#process_raw_message(a:msg)
  call clap#preview#async_open_with_delay()
endfunction

function! clap#filter#async#dyn#start_directly(maple_cmd) abort
  let s:last_query = g:clap.input.get()
  call clap#job#stdio#start_service(function('s:handle_message'), a:maple_cmd)
endfunction

function! clap#filter#async#dyn#start(cmd) abort
  let s:last_query = g:clap.input.get()
  call clap#job#stdio#start_dyn_filter_service(function('s:handle_message'), a:cmd)
endfunction

function! clap#filter#async#dyn#from_tempfile(tempfile) abort
  let s:last_query = g:clap.input.get()

  call clap#job#stdio#start_service(
        \ function('s:handle_message'),
        \ clap#maple#command#filter_dyn(s:DYN_ITEMS_TO_SHOW, a:tempfile),
        \ )
endfunction

function! s:prepare_grep_cmd() abort
  let s:last_query = g:clap.input.get()
  let subcmd = g:clap_enable_icon ? ['--icon=Grep'] : []
  if has_key(g:clap.context, 'no-cache')
    call add(subcmd, '--no-cache')
  endif
  let opts = [
        \ '--number', s:DYN_ITEMS_TO_SHOW,
        \ '--winwidth', winwidth(g:clap.display.winid),
        \ 'grep', g:clap.input.get(),
        \ ]
  return subcmd + opts
endfunction

function! clap#filter#async#dyn#start_grep() abort
  let grep_cmd = s:prepare_grep_cmd()
  let grep_cmd = clap#maple#build_cmd_list(grep_cmd + ['--cmd-dir', clap#rooter#working_dir()])
  call clap#job#stdio#start_service(function('s:handle_message'), grep_cmd)
endfunction

function! clap#filter#async#dyn#grep_from_cache(tempfile) abort
  let grep_cmd = s:prepare_grep_cmd()
  let grep_cmd = clap#maple#build_cmd_list(grep_cmd + ['--input', a:tempfile])
  call clap#job#stdio#start_service(function('s:handle_message'), grep_cmd)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
