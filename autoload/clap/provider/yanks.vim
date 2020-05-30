" Author: Ratheesh S <ratheeshreddy@gmail.com>
" Description: List the recently yanked/deleted lines
" Based on : https://github.com/sgur/ctrlp-extensions.vim

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:min_len      = get(g:, 'clap_provider_yanks_min_len', 1)
let s:max_yanks    = get(g:, 'clap_provider_yanks_max_entries', 20)
let s:yank_info_map = {}
if exists('g:clap_provider_yanks_history') && filereadable(expand(g:clap_provider_yanks_history))
  let s:yank_history = readfile(expand(g:clap_provider_yanks_history))
else
  let s:yank_history = []
endif

let s:yanks = {}

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
  let s:yank_info_map[last_yanked] = { 'syntax': getbufvar('', '&syntax'), 'bufname': bufname() }

  " Trim yank entries(purge old ones)
  if len(s:yank_history) > s:max_yanks
    let oldest_yanked = remove(s:yank_history, -1)
    if type(oldest_yanked) == v:t_string && has_key(s:yank_info_map, oldest_yanked)
      call remove(s:yank_info_map, oldest_yanked)
    endif
  endif
endfunction

function! s:save_history() abort
  if g:clap_provider_yanks_history && !empty(s:yank_history)
    call writefile(s:yank_history, expand(g:clap_provider_yanks_history))
  endif
endfunction

function! clap#provider#yanks#init() abort
  augroup ClapYanksCollect
    autocmd!
    autocmd TextYankPost * call clap#provider#yanks#collect()
    if exists('g:clap_provider_yanks_history')
      autocmd VimLeavePre  * call s:save_history()
    endif
  augroup END

  " collect the data from default register
  call clap#provider#yanks#collect()
endfunction

function! s:yanks.source() abort
  return s:yank_history
endfunction

function! s:yanks.on_move() abort
  let curline = g:clap.display.getcurline()
  let lines = split(curline, "\n")[:10]
  if !empty(lines)
    call g:clap.preview.show(lines)
    if has_key(s:yank_info_map, curline)
      call g:clap.preview.setbufvar('&syntax', s:yank_info_map[curline].syntax)
    endif
  endif
endfunction

function! s:yanks.sink(selected) abort
  call setreg('"', a:selected)
  normal! ""p
endfunction

function! s:yanks.on_enter() abort
  if !get(g:, 'clap_enable_yanks_provider', 1)
    call clap#helper#echo_error('Clap yanks provider is disabled, set g:clap_enable_yanks_provider to 1 to enable.')
    call clap#handler#exit()
    call feedkeys("\<Esc>", 'n')
  endif
endfunction

let g:clap#provider#yanks# = s:yanks

let &cpoptions = s:save_cpo
unlet s:save_cpo
