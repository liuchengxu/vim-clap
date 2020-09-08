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

  let s:async_preview_implemented = ['files', 'git_files', 'grep', 'grep2', 'proj_tags', 'tags', 'blines', 'history']

  function! s:handle_on_move_result(result, error) abort
    if a:error isnot v:null
      return
    endif
    if has_key(a:result, 'lines')
      try
        call g:clap.preview.show(a:result.lines)
      catch
        return
      endtry
      if has_key(a:result, 'fname')
        call g:clap.preview.set_syntax(clap#ext#into_filetype(a:result.fname))
      endif
      call clap#preview#highlight_header()

      if has_key(a:result, 'hi_lnum')
        call g:clap.preview.add_highlight(a:result.hi_lnum+1)
      endif
    endif
  endfunction

  function! s:dispatch_on_move_impl() abort
    if index(s:async_preview_implemented, g:clap.provider.id) > -1
      return clap#client#call_on_move('on_move', function('s:handle_on_move_result'))
    endif
    call s:sync_run_with_delay()
  endfunction
else
  function! s:dispatch_on_move_impl() abort
    call s:sync_run_with_delay()
  endfunction
endif

function! clap#impl#on_move#invoke() abort
  if get(g:, '__clap_has_no_matches', v:false)
    return
  endif
  if has_key(g:clap.provider._(), 'on_move_async')
    call g:clap.provider._().on_move_async()
  elseif has_key(g:clap.provider._(), 'on_move')
    call s:dispatch_on_move_impl()
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
