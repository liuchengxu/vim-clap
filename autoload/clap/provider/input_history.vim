" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the top N input history.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:limited_input_history = []
let s:MAXIMUM_INPUT_HISTORY = 100

let s:input_history = {}

function! clap#provider#input_history#note() abort
  let cur_input = g:clap.input.get()
  let provider_id = g:clap.provider.id
  if !empty(cur_input) && provider_id !=# 'input_history' && provider_id !=# 'providers'
    let maybe_new_record = provider_id.':'.cur_input
    if index(s:limited_input_history, maybe_new_record) == -1
      call add(s:limited_input_history, maybe_new_record)
      if len(s:limited_input_history) > s:MAXIMUM_INPUT_HISTORY
        let s:limited_input_history = s:limited_input_history[1:]
      endif
    endif
  endif
endfunction

function! s:input_history.source() abort
  return reverse(copy(s:limited_input_history))
endfunction

function! s:input_history.sink(line) abort
  let idx = 0
  for idx in range(len(a:line))
    if a:line[idx] ==# ':'
      break
    endif
  endfor
  let provider = a:line[:idx-1]
  let query = a:line[idx+1:]
  call timer_start(0, {-> clap#_for_with_query(provider, query)})
endfunction

let g:clap#provider#input_history# = s:input_history

let &cpoptions = s:save_cpo
unlet s:save_cpo
