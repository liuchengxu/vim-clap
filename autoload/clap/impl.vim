" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Default implementation for various hooks.

let s:save_cpo = &cpo
set cpo&vim

let s:is_nvim = has('nvim')

let s:NO_MATCHES = 'NO MATCHES FOUND'

"                          filter
"                       /  (sync)
"             on_typed -
"           /           \
"          /              dispatcher
" on_enter                 (async)        --> on_exit
"          \
"           \
"             on_move
"
function! clap#impl#on_typed() abort
  call g:clap.display.clear_highlight()

  let l:cur_input = g:clap.input.get()

  if empty(l:cur_input)
    call g:clap.display.set_lines(g:clap.provider.get_source())
    call clap#indicator#set_matches('['.g:clap.display.line_count().']')
    call g:clap#display_win.compact_if_undersize()
    return
  endif

  call clap#spinner#set_busy()

  if get(g:, '__clap_should_refilter', v:false)
    let l:lines = g:clap.provider.get_source()
    let g:__clap_should_refilter = v:false
  else
    " Assuming in the middle of typing, we are continuing to filter.
    let l:lines = g:clap.display.get_lines()

    " If there is no matches for the current filtered result, restore to the original source.
    if l:lines == [s:NO_MATCHES]
      let l:lines = g:clap.provider.get_source()
    endif
  endif

  let l:has_no_matches = v:false

  let l:lines = call(g:clap.provider.filter(), [l:lines, l:cur_input])

  if empty(l:lines)
    let l:lines = [s:NO_MATCHES]
    let l:has_no_matches = v:true
  endif

  call g:clap.display.set_lines(lines)

  " NOTE: some local variable without explicit l:, e.g., count,
  " may run into some erratic read-only error.
  if l:has_no_matches
    if get(g:clap.display, 'initial_size', -1) > 0
      let l:count = '0/'.g:clap.display.initial_size
    else
      let l:count = '0'
    endif
    call clap#indicator#set_matches('['.l:count.']')
  else
    let l:matches_cnt = g:clap.display.line_count()
    if get(g:clap.display, 'initial_size', -1) > 0
      let l:matches_cnt .= '/'.g:clap.display.initial_size
    endif
    call clap#indicator#set_matches('['.l:matches_cnt.']')
  endif

  call g:clap#display_win.compact_if_undersize()
  call clap#spinner#set_idle()

  call g:clap.display.add_highlight()
endfunction

let &cpo = s:save_cpo
unlet s:save_cpo
