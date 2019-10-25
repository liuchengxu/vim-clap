" Author: Ratheesh S <ratheeshreddy@gmail.com>
" Description: List the recently yanked/deleted lines
" Based on : https://github.com/sgur/ctrlp-extensions.vim

let s:save_cpo = &cpo
set cpo&vim

let s:yank_history = []
let s:yanks = {}
let s:max_yanks = get(g:, 'clap_provider_yanks_max_entries', 20)

function! clap#provider#yanks#collect() abort
  let last_yanked = getreg('"')

  if !empty(s:yank_history) && last_yanked == s:yank_history[0]
    return
  endif

  call filter(s:yank_history, 'v:val != last_yanked')
  call insert(s:yank_history, last_yanked)

  " Trim yank entries(purge old ones)
  if len(s:yank_history) > s:max_yanks
    call remove(s:yank_history, s:max_yanks, -1)
  endif

endfunction

function! clap#provider#yanks#init() abort
  augroup ClapYanksCollect
    autocmd!
    autocmd TextYankPost * call clap#provider#yanks#collect()
  augroup END

  " collect the data from default register
  call clap#provider#yanks#collect()
endfunction

function! s:yanks.source() abort
  return s:yank_history
endfunction

function! s:yanks_sink(selected) abort
  call setreg('"', a:selected)
  normal! ""p
endfunction

let s:yanks.sink = function('s:yanks_sink')

let g:clap#provider#yanks# = s:yanks

let &cpo = s:save_cpo
unlet s:save_cpo
