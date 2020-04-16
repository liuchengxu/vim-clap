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

  if g:clap_enable_icon && index(['files', 'git_files'], g:clap.provider.id) > -1
    let enable_icon_opt = '--icon-painter=File'
  else
    let enable_icon_opt = ''
  endif

  if g:clap.provider.id ==# 'files' && has_key(g:clap.context, 'name-only')
    let content_filtering = '--content-filtering=FileNameOnly'
  elseif g:clap.provider.id ==# 'proj_tags'
    let content_filtering = '--content-filtering=TagNameOnly'
  else
    let content_filtering = ''
  endif

  let filter_cmd = printf('%s --number %d --winwidth %d filter "%s" --input "%s" %s',
        \ enable_icon_opt,
        \ s:DYN_ITEMS_TO_SHOW,
        \ winwidth(g:clap.display.winid),
        \ g:clap.input.get(),
        \ a:tempfile,
        \ content_filtering,
        \ )
  call clap#job#stdio#start_service(function('s:handle_message'), clap#maple#build_cmd(filter_cmd))
endfunction

function! clap#filter#async#dyn#start_grep() abort
  let s:last_query = g:clap.input.get()
  let grep_cmd = printf('%s --number %d --winwidth %d grep "" "%s" --cmd-dir "%s"',
        \ g:clap_enable_icon ? '--icon-painter=Grep' : '',
        \ s:DYN_ITEMS_TO_SHOW,
        \ winwidth(g:clap.display.winid),
        \ g:clap.input.get(),
        \ clap#rooter#working_dir(),
        \ )
  call clap#job#stdio#start_service(function('s:handle_message'), clap#maple#build_cmd(grep_cmd))
endfunction

function! clap#filter#async#dyn#grep_from_cache(tempfile) abort
  let s:last_query = g:clap.input.get()
  let grep_cmd = printf('%s %s --number %d --winwidth %d grep "" "%s" --input "%s"',
        \ g:clap_enable_icon ? '--icon-painter=Grep' : '',
        \ has_key(g:clap.context, 'no-cache') ? '--no-cache' : '',
        \ s:DYN_ITEMS_TO_SHOW,
        \ winwidth(g:clap.display.winid),
        \ g:clap.input.get(),
        \ a:tempfile
        \ )
  call clap#job#stdio#start_service(function('s:handle_message'), clap#maple#build_cmd(grep_cmd))
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
