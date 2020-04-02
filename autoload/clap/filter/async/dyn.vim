" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Dynamic update version of maple filter.

let s:save_cpo = &cpoptions
set cpoptions&vim

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

let &cpoptions = s:save_cpo
unlet s:save_cpo
