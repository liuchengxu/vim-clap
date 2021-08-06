" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: CursorMoved handler

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:on_move_timer = -1
let s:on_move_delay = get(g:, 'clap_on_move_delay', 300)

function! s:sync_run_with_delay() abort
  if s:on_move_timer != -1
    call timer_stop(s:on_move_timer)
  endif
  let s:on_move_timer = timer_start(s:on_move_delay, { -> g:clap.provider._().on_move() })
endfunction

if clap#maple#is_available()
  function! clap#impl#on_move#handler(result, error) abort
    if a:error isnot v:null
      return
    endif
    call clap#state#process_preview_result(a:result)
  endfunction

  function! s:dispatch_on_move_impl() abort
    if has_key(g:clap.provider._(), 'on_move_async')
      call g:clap.provider._().on_move_async()
    else
      call s:sync_run_with_delay()
    endif
  endfunction

  function! clap#impl#on_move#async() abort
    call clap#client#call_on_move('on_move', function('clap#impl#on_move#handler'))
  endfunction
else
  function! s:dispatch_on_move_impl() abort
    call s:sync_run_with_delay()
  endfunction

  function! clap#impl#on_move#async() abort
  endfunction

  function! clap#impl#on_move#handler(_result, _error) abort
  endfunction
endif

function! clap#impl#on_move#invoke() abort
  if get(g:, '__clap_has_no_matches', v:false)
    return
  endif
  " Currently the on_move impl is for preview only.
  if !clap#preview#is_enabled()
    return
  endif

  if has_key(g:clap.provider._(), 'on_move_async')
    call g:clap.provider._().on_move_async()
  elseif has_key(g:clap.provider._(), 'on_move')
    call s:dispatch_on_move_impl()
  endif
endfunction

function! clap#impl#on_move#invoke_async() abort
  if get(g:, '__clap_has_no_matches', v:false)
    return
  endif
  if has_key(g:clap.provider._(), 'on_move_async')
    call g:clap.provider._().on_move_async()
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
