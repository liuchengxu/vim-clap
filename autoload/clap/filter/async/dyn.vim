" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Dynamic update version of maple filter.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Currently this is not configurable.
let s:DYN_ITEMS_TO_SHOW = 30

function! s:handle_message(msg) abort
  if !g:clap.display.win_is_valid()
        \ || g:clap.input.get() !=# s:last_query
    return
  endif

  call clap#state#handle_message(a:msg)
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

  if g:clap_enable_icon
    if index(['files', 'git_files'], g:clap.provider.id) > -1
      let enable_icon_opt = ['--icon-painter=File']
    elseif 'proj_tags' ==# g:clap.provider.id
      let enable_icon_opt = ['--icon-painter=ProjTags']
    else
      let enable_icon_opt = []
    endif
  else
    let enable_icon_opt = []
  endif

  if g:clap.provider.id ==# 'files' && has_key(g:clap.context, 'name-only')
    let line_splitter = ['--line-splitter=FileNameOnly']
  elseif g:clap.provider.id ==# 'proj_tags'
    let line_splitter = ['--line-splitter=TagNameOnly']
  else
    let line_splitter = []
  endif

  let filter_cmd = clap#maple#build_cmd_list(enable_icon_opt + ['--number', s:DYN_ITEMS_TO_SHOW, '--winwidth', winwidth(g:clap.display.winid), 'filter', g:clap.input.get(), '--input', a:tempfile] + line_splitter)
  call clap#job#stdio#start_service(function('s:handle_message'), filter_cmd)
endfunction

function! s:grep_cmd_common() abort
  return ['--number', s:DYN_ITEMS_TO_SHOW, '--winwidth', winwidth(g:clap.display.winid), 'grep', g:clap.input.get()]
endfunction

function! clap#filter#async#dyn#start_grep() abort
  let s:last_query = g:clap.input.get()
  let subcmd = g:clap_enable_icon ? ['--icon-painter=Grep'] : []
  let grep_cmd = clap#maple#build_cmd_list(subcmd + s:grep_cmd_common() + ['--cmd-dir', clap#rooter#working_dir()])
  call clap#job#stdio#start_service(function('s:handle_message'), grep_cmd)
endfunction

function! clap#filter#async#dyn#grep_from_cache(tempfile) abort
  let s:last_query = g:clap.input.get()
  let subcmd = g:clap_enable_icon ? ['--icon-painter=Grep'] : []
  if has_key(g:clap.context, 'no-cache')
    call add(subcmd, '--no-cache')
  endif
  let grep_cmd = clap#maple#build_cmd_list(subcmd + s:grep_cmd_common() + ['--input', a:tempfile])
  call clap#job#stdio#start_service(function('s:handle_message'), grep_cmd)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
