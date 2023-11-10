" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Matches indicator.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:indicator = { 'matched': 0, 'processed': -1 }

function! s:padding(indicator) abort
  let indicator_len = strlen(a:indicator)
  if indicator_len < get(g:, '__clap_indicator_winwidth', 0)
    return repeat(' ', g:__clap_indicator_winwidth - indicator_len).a:indicator
  else
    return a:indicator
  endif
endfunction

if has('nvim')
  function! s:update_indicator(indicator) abort
    if bufexists(g:__clap_indicator_bufnr)
      call setbufline(g:__clap_indicator_bufnr, 1, a:indicator)
    endif
  endfunction
else
  function! s:update_indicator(indicator) abort
    if exists('g:__clap_indicator_winid')
      call popup_settext(g:__clap_indicator_winid, a:indicator)
    endif
  endfunction
endif

function! s:indicator.reset() abort
  let self.matched = 0
  let self.processed = -1
endfunction

function! s:indicator.format() abort
  let selected = clap#sign#current_selections_count()
  if self.processed == -1
    return printf('%d [%d]', self.matched, selected)
  else
    return printf('%d/%d [%d]', self.matched, self.processed, selected)
  endif
endfunction

" Caveat: This function can have a performance bottle neck if update frequently.
"
" If you feel the responsive is slow, try to disable the indicator.
" especially for the outside async jobs which could be cpu-intensive.
"
" If the initial_size is possible, use clap#state#refresh_matches_count()
" instead in that it will combine the initial_size info.
function! s:indicator.render(formatted) abort
  if g:clap_disable_matches_indicator
    return
  endif
  call s:update_indicator(s:padding(a:formatted))
endfunction

function! clap#indicator#update_on_deletecurline() abort
  let s:indicator.matched -= 1
  let s:indicator.processed -= 1
  call s:indicator.render(s:indicator.format())
endfunction

function! clap#indicator#update_matched(matched) abort
  let s:indicator.matched = a:matched
  call s:indicator.render(s:indicator.format())
endfunction

function! clap#indicator#update_processed(processed) abort
  let s:indicator.processed = a:processed
  call s:indicator.render(s:indicator.format())
endfunction

function! clap#indicator#update(matched, processed) abort
  let s:indicator.matched = a:matched
  let s:indicator.processed = a:processed
  call s:indicator.render(s:indicator.format())
endfunction

" API for specific provider like dumb_jump.
function! clap#indicator#update_matched_only(matched) abort
  let s:indicator.matched = a:matched
  let s:indicator.processed = -1
  let selected = clap#sign#current_selections_count()
  call s:indicator.render(printf('%d [%d]', a:matched, selected))
endfunction

function! clap#indicator#render() abort
  call s:indicator.render(s:indicator.format())
endfunction

function! clap#indicator#reset() abort
  call s:indicator.reset()
  call s:indicator.render(s:indicator.format())
endfunction

function! clap#indicator#set_none() abort
  " Don't repeat(' ') directly as we can see the trailing char of listchars.
  call s:update_indicator(repeat(' ', &columns).' for eliminating the trailing char')
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
