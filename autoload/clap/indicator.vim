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
" If the initial_size is possible, use clap#state#refresh_matches_count()
" instead in that it will combine the initial_size info.
function! clap#indicator#set(indicator) abort
  if !g:clap_disable_matches_indicator
    call s:set_indicator(a:indicator)
  endif
endfunction

function! clap#indicator#set_matches_number(number) abort
  let s:matches_number = a:number

  if get(g:clap.display, 'initial_size', -1) > 0
    let l:matches_cnt = a:number.'/'.g:clap.display.initial_size
  else
    let l:matches_cnt = a:number
  endif

  call clap#indicator#set('['.l:matches_cnt.']')
endfunction

function! clap#indicator#update_matches_on_deletecurline() abort
  let s:matches_number -= 1
  if get(g:clap.display, 'initial_size', -1) > 0
    let g:clap.display.initial_size -= 1
    let l:matches_cnt = s:matches_number.'/'.g:clap.display.initial_size
  else
    let l:matches_cnt = s:matches_number
  endif
  call clap#indicator#set('['.l:matches_cnt.']')
endfunction

function! clap#indicator#update_matches_on_forerunner_done() abort
  if exists('s:matches_number')
    call clap#indicator#set(printf('[%s/%s]', s:matches_number, g:clap.display.initial_size))
  else
    call clap#indicator#set(printf('[%s/%s]', g:clap.display.initial_size, g:clap.display.initial_size))
  endif
endfunction

function! clap#indicator#clear() abort
  silent! unlet s:matches_number
endfunction

function! clap#indicator#set_none() abort
  " Don't repeat(' ') directly as we can see the trailing char of listchars.
  call clap#indicator#set(repeat(' ', &columns).' for eliminating the trailing char')
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
