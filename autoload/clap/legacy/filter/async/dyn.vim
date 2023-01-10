" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Dynamic update version of maple filter.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Deprecated, s:PAR_RUN is encouraged.
let s:DYN_ITEMS_TO_SHOW = 40
" TODO: make this confiurable?
let s:PAR_RUN = v:true

function! s:handle_stdio_message(msg) abort
  if !g:clap.display.win_is_valid()
        \ || g:clap.input.get() !=# s:last_query
    return
  endif

  let decoded = json_decode(a:msg)

  if type(decoded) != v:t_dict
    return
  endif

  call clap#state#process_filter_message(decoded, v:false)
  call clap#preview#update_with_delay()
endfunction

function! clap#legacy#filter#async#dyn#start_directly(maple_cmd) abort
  let s:last_query = g:clap.input.get()
  call clap#job#stdio#start_service_with_delay(function('s:handle_stdio_message'), a:maple_cmd)
endfunction

function! clap#legacy#filter#async#dyn#start_blines() abort
  let s:last_query = g:clap.input.get()
  let blines_cmd = clap#maple#command#blines()
  if s:PAR_RUN
    call add(blines_cmd, '--par-run')
  endif
  call clap#job#stdio#start_service_with_delay(function('s:handle_stdio_message'), blines_cmd)
endfunction

function! clap#legacy#filter#async#dyn#start_ctags_recursive() abort
  let s:last_query = g:clap.input.get()

  let no_cache = has_key(g:clap.context, 'no-cache')

  if !no_cache && exists('g:__clap_forerunner_tempfile')
    call clap#legacy#filter#async#dyn#start_filter_with_cache(g:__clap_forerunner_tempfile)
  else
    let ctags_cmd = g:clap_enable_icon ? ['--icon=ProjTags'] : []
    if no_cache
      let ctags_cmd += ['--no-cache']
    endif
    let ctags_cmd += [
          \ '--winwidth', winwidth(g:clap.display.winid),
          \ '--number', s:PAR_RUN ? g:clap.display.preload_capacity : s:DYN_ITEMS_TO_SHOW,
          \ '--case-matching', has_key(g:clap.context, 'ignorecase') ? 'ignore' : 'smart',
          \ 'ctags', 'recursive-tags',
          \ '--dir', clap#rooter#working_dir(),
          \ '--query', g:clap.input.get(),
          \ ]
    if s:PAR_RUN
      call add(ctags_cmd, '--par-run')
    endif
    let ctags_cmd = clap#maple#build_cmd_list(ctags_cmd)
    call clap#job#stdio#start_service_with_delay(function('s:handle_stdio_message'), ctags_cmd)
  endif
endfunction

function! clap#legacy#filter#async#dyn#start_filter(cmd) abort
  let s:last_query = g:clap.input.get()

  let filter_cmd = g:clap_enable_icon && g:clap.provider.id ==# 'files' ? ['--icon=File'] : []
  let filter_cmd += [
        \ '--number', s:PAR_RUN ? g:clap.display.preload_capacity : s:DYN_ITEMS_TO_SHOW,
        \ '--winwidth', winwidth(g:clap.display.winid),
        \ '--case-matching', has_key(g:clap.context, 'ignorecase') ? 'ignore' : 'smart',
        \ 'filter', g:clap.input.get(), '--cmd', a:cmd, '--cmd-dir', clap#rooter#working_dir(),
        \]
  if has_key(g:clap.context, 'name-only')
    call add(filter_cmd, '--match-scope=FileName')
  endif
  if s:PAR_RUN
    call add(filter_cmd, '--par-run')
  endif

  let filter_cmd = clap#maple#build_cmd_list(filter_cmd)
  call clap#job#stdio#start_service_with_delay(function('s:handle_stdio_message'), filter_cmd)
endfunction

function! clap#legacy#filter#async#dyn#start_filter_with_cache(tempfile) abort
  let s:last_query = g:clap.input.get()

  call clap#job#stdio#start_service_with_delay(
        \ function('s:handle_stdio_message'),
        \ clap#maple#command#filter_dyn(s:DYN_ITEMS_TO_SHOW, a:tempfile),
        \ )
endfunction

function! clap#legacy#filter#async#dyn#start_grep() abort
  let s:last_query = g:clap.input.get()

  let grep_cmd = g:clap_enable_icon ? ['--icon=Grep'] : []
  if has_key(g:clap.context, 'no-cache')
    call add(grep_cmd, '--no-cache')
  endif
  let grep_cmd += [
        \ '--number', s:PAR_RUN ? g:clap.display.preload_capacity : s:DYN_ITEMS_TO_SHOW,
        \ '--winwidth', winwidth(g:clap.display.winid),
        \ '--case-matching', has_key(g:clap.context, 'ignorecase') ? 'ignore' : 'smart',
        \ 'grep', g:clap.input.get(),
        \ ]

  if exists('g:__clap_forerunner_tempfile')
    let grep_cmd += ['--input', g:__clap_forerunner_tempfile]
  else
    let grep_cmd += ['--cmd-dir', clap#rooter#working_dir()]
  endif

  if s:PAR_RUN
    call add(grep_cmd, '--par-run')
  endif
  let grep_cmd = clap#maple#build_cmd_list(grep_cmd)

  call clap#job#stdio#start_service_with_delay(function('s:handle_stdio_message'), grep_cmd)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
