" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Dynamic update version of maple filter.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Deprecated, s:PAR_RUN is encouraged.
let s:DYN_ITEMS_TO_SHOW = 40
" TODO: make this confiurable?
let s:PAR_RUN = v:true

function! s:handle_message(msg) abort
  if !g:clap.display.win_is_valid()
        \ || g:clap.input.get() !=# s:last_query
    return
  endif

  call clap#state#process_dyn_message(a:msg)
  call clap#preview#async_open_with_delay()
endfunction

function! clap#filter#async#dyn#start_directly(maple_cmd) abort
  let s:last_query = g:clap.input.get()
  call clap#job#stdio#start_service(function('s:handle_message'), a:maple_cmd)
endfunction

function! clap#filter#async#dyn#start_blines() abort
  let s:last_query = g:clap.input.get()
  let blines_cmd = clap#maple#command#blines()
  if s:PAR_RUN
    call add(blines_cmd, '--par-run')
  endif
  call clap#job#stdio#start_service(function('s:handle_message'), blines_cmd)
endfunction

function! clap#filter#async#dyn#start_filter(cmd) abort
  let s:last_query = g:clap.input.get()

  let filter_cmd = g:clap_enable_icon && g:clap.provider.id ==# 'files' ? ['--icon=File'] : []
  let filter_cmd += [
        \ '--number', s:PAR_RUN ? g:clap.display.preload_capacity : 100,
        \ '--winwidth', winwidth(g:clap.display.winid),
        \ '--case-matching', has_key(g:clap.context, 'ignorecase') ? 'ignore' : 'smart',
        \ 'filter', g:clap.input.get(), '--cmd', a:cmd, '--cmd-dir', clap#rooter#working_dir(),
        \ ]

  if s:PAR_RUN
    call add(filter_cmd, '--par-run')
  endif

  let filter_cmd = clap#maple#build_cmd_list(filter_cmd)
  call clap#job#stdio#start_service(function('s:handle_message'), filter_cmd)
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
  let opts = s:PAR_RUN ? ['--number', g:clap.display.preload_capacity] : ['--number', s:DYN_ITEMS_TO_SHOW]
  let opts += [
        \ '--winwidth', winwidth(g:clap.display.winid),
        \ 'grep', g:clap.input.get(),
        \ ]
  return subcmd + opts
endfunction

function! clap#filter#async#dyn#start_grep() abort
  let grep_cmd = s:prepare_grep_cmd()

  if exists('g:__clap_forerunner_tempfile')
    let grep_cmd += ['--input', g:__clap_forerunner_tempfile]
  else
    let grep_cmd += ['--cmd-dir', clap#rooter#working_dir()]
    call clap#filter#async#dyn#start_grep()
  endif

  if s:PAR_RUN
    call add(grep_cmd, '--par-run')
  endif
  let grep_cmd = clap#maple#build_cmd_list(grep_cmd)

  call clap#job#stdio#start_service(function('s:handle_message'), grep_cmd)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
