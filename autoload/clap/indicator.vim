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

  function! s:apply_indicator(indicator) abort
    if bufexists(g:clap.input.bufnr)
      let indicator = s:padding(a:indicator)
      call nvim_buf_clear_highlight(g:clap.input.bufnr, s:ns_id, 0, -1)
      call nvim_buf_set_virtual_text(g:clap.input.bufnr, s:ns_id, 0, [[indicator, 'LinNr']], {})
    endif
  endfunction
else
  function! s:apply_indicator(indicator) abort
    call popup_settext(g:clap_indicator_winid, a:indicator)
  endfunction
endif

" Caveat: This function can have a peformance bottle neck if update frequently.
" If you feel the responsive is slow, try to disable the indicator.
" especially for the outside async jobs which could be cpu-intensive.
function! clap#indicator#set_matches(indicator) abort
  if get(g:, 'clap_disable_matches_indicator', v:false)
    return
  endif
  call s:apply_indicator(a:indicator)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
