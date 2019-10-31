" Author: Ratheesh S <ratheeshreddy@gmail.com>
" Description: List the recently yanked/deleted lines
" Based on : https://github.com/sgur/ctrlp-extensions.vim

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:yank_history = []
let s:yanks        = {}
let s:max_yanks    = get(g:, 'clap_provider_yanks_max_entries', 20)
let s:min_len      = get(g:, 'clap_provider_yanks_min_len', 1)

function! clap#provider#yanks#collect() abort
  let last_yanked = getreg('"')

  if len(last_yanked) < s:min_len
    return
  endif

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

function! s:yanks_enter() abort
  if !get(g:, 'clap_enable_yanks_provider', 1)
    call clap#error('Clap yanks provider is disabled, set g:clap_enable_yanks_provider to 1 to enable.')
    call clap#handler#exit()
    call feedkeys("\<Esc>", 'n')
  endif
endfunction

let s:yanks.on_enter = function('s:yanks_enter')

let g:clap#provider#yanks# = s:yanks

let &cpoptions = s:save_cpo
unlet s:save_cpo
