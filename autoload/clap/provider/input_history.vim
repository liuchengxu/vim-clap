" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the top N input history.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:limited_input_history = []
let s:MAXIMUM_INPUT_HISTORY = 20

let s:input_history = {}

function! clap#provider#input_history#note() abort
  let cur_input = g:clap.input.get()
  if !empty(cur_input)
    call add(s:limited_input_history, cur_input)
    if len(s:limited_input_history) > s:MAXIMUM_INPUT_HISTORY
      let s:limited_input_history = s:limited_input_history[1:]
    endif
  endif
endfunction

function! s:input_history.source() abort
  return copy(s:limited_input_history)
endfunction

function! s:input_history.sink(line) abort
  let provider = g:__clap_last_normal_provider
  call timer_start(0, {-> clap#_for_with_query(provider, a:line)})
endfunction

let g:clap#provider#input_history# = s:input_history

let &cpoptions = s:save_cpo
unlet s:save_cpo
