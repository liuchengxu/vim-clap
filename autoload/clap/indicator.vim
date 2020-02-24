" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Matches indicator.

let s:save_cpo = &cpoptions
set cpoptions&vim

function! s:padding(indicator) abort
  let indicator_len = strlen(a:indicator)
  if indicator_len < g:__clap_indicator_winwidth
    return repeat(' ', g:__clap_indicator_winwidth - indicator_len).a:indicator
  else
    return a:indicator
  endif
endfunction

if has('nvim')
  function! s:set_indicator(indicator) abort
    if bufexists(g:__clap_indicator_bufnr)
      call setbufline(g:__clap_indicator_bufnr, 1, s:padding(a:indicator))
    endif
  endfunction
else
  function! s:set_indicator(indicator) abort
    call popup_settext(g:clap_indicator_winid, s:padding(a:indicator))
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
  call s:set_indicator(a:indicator)
endfunction

function! clap#indicator#set_none() abort
  " Don't repeat(' ') directly as we can see the trailing char of listchars.
  call clap#indicator#set_matches(repeat(' ', &columns).' for eliminating the trailing char')
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
