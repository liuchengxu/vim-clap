" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Matches indicator.

let s:save_cpo = &cpoptions
set cpoptions&vim

if has('nvim')
  let s:ns_id = nvim_create_namespace('clap')

  function! s:padding(indicator) abort
    let width = g:clap#floating_win#input.width
    let input_len = len(g:clap.input.get())
    let indicator = repeat(' ', width - input_len - len(a:indicator) - 2).a:indicator
    return indicator
  endfunction

  function! clap#indicator#repadding() abort
    if exists('s:current_indicator')
      call nvim_buf_set_virtual_text(g:clap.input.bufnr, s:ns_id, 0, [[s:padding(s:current_indicator), 'LinNr']], {})
    endif
  endfunction

  function! s:apply_indicator(indicator) abort
    if bufexists(g:clap.input.bufnr)
      let s:current_indicator = a:indicator
      call nvim_buf_clear_highlight(g:clap.input.bufnr, s:ns_id, 0, -1)
      call nvim_buf_set_virtual_text(g:clap.input.bufnr, s:ns_id, 0, [[s:padding(a:indicator), 'LinNr']], {})
    endif
  endfunction
else
  function! s:apply_indicator(indicator) abort
    let indicator_len = strlen(a:indicator)
    if indicator_len < 18
      let indicator = repeat(' ', 18 - indicator_len).a:indicator
    else
      let indicator = a:indicator
    endif
    call popup_settext(g:clap_indicator_winid, indicator)
  endfunction
endif

" Caveat: This function can have a peformance bottle neck if update frequently.
"
" If you feel the responsive is slow, try to disable the indicator.
" especially for the outside async jobs which could be cpu-intensive.
"
" If the initial_size is possible, use clap#impl#refresh_matches_count()
" instead in that it will combine the initial_size info.
function! clap#indicator#set_matches(indicator) abort
  if get(g:, 'clap_disable_matches_indicator', v:false)
    return
  endif
  call s:apply_indicator(a:indicator)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
