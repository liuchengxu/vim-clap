" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Multi selection support.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')
let s:signed = []
let s:sign_group = 'clapSelected'
let s:sign_cur_group = 'clapCurrentSelected'
let s:last_signed_id = -1

if !exists('s:sign_inited')
  call sign_define(s:sign_group, get(g:, 'clap_selected_sign', {
        \ 'text': ' >',
        \ 'texthl': 'WarningMsg',
        \ 'linehl': 'ClapSelected'
        \ }))
  call sign_define(s:sign_cur_group, get(g:, 'clap_current_selection_sign', {
        \ 'text': '>>',
        \ 'texthl': 'WarningMsg',
        \ 'linehl': 'ClapCurrentSelection',
        \ }))
  let s:sign_inited = 1
endif

" lnum is the sign id
function! s:place_sign_at(lnum) abort
  call sign_place(a:lnum, s:sign_group, 'clapSelected', g:clap.display.bufnr, {'lnum': a:lnum})
endfunction

function! s:unplace_sign_at(sign_id) abort
  call sign_unplace(s:sign_group, {'buffer': g:clap.display.bufnr, 'id': a:sign_id})
endfunction

function! s:place_cur_sign_at(lnum) abort
  call sign_place(a:lnum, s:sign_cur_group, 'clapCurrentSelected', g:clap.display.bufnr, {'lnum': a:lnum})
endfunction

function! s:unplace_cur_sign_at(sign_id) abort
  call sign_unplace(s:sign_cur_group, {'buffer': g:clap.display.bufnr, 'id': a:sign_id})
endfunction

function! clap#sign#toggle_cursorline() abort
  if s:last_signed_id != -1
    call s:unplace_cur_sign_at(s:last_signed_id)
  endif
  let curlnum = g:clap.display.getcurlnum()
  call s:place_cur_sign_at(curlnum)
  let s:last_signed_id = curlnum
endfunction

function! s:sign_of_first_line() abort
  return sign_getplaced(g:clap.display.bufnr, {'group': s:sign_cur_group, 'lnum': 1})[0].signs
endfunction

function! clap#sign#reset_to_first_line() abort
  if s:last_signed_id == 1 && !empty(s:sign_of_first_line())
    return
  endif
  if s:last_signed_id != -1
    call s:unplace_cur_sign_at(s:last_signed_id)
  endif
  call g:clap.display.set_cursor(1, 1)
  let curlnum = 1
  let g:__clap_display_curlnum = curlnum
  call s:place_cur_sign_at(curlnum)
  let s:last_signed_id = curlnum
endfunction

function! clap#sign#toggle_cursorline_multi() abort
  let curlnum = g:clap.display.getcurlnum()
  let sign_idx = index(s:signed, curlnum)
  if sign_idx != -1
    call s:unplace_sign_at(curlnum)
    unlet s:signed[sign_idx]
  else
    call s:place_sign_at(curlnum)
    call add(s:signed, curlnum)
  endif
  call clap#handler#internal_navigate('down')
endfunction

function! clap#sign#disable_cursorline() abort
  call sign_unplace(s:sign_cur_group, {'buffer': g:clap.display.bufnr})
endfunction

function! clap#sign#toggle() abort
  let curlnum = g:clap.display.getcurlnum()

  let sign_idx = index(s:signed, curlnum)
  if sign_idx == -1
    call s:place_sign_at(curlnum)
    call add(s:signed, curlnum)
  else
    let sign_id = s:signed[sign_idx]
    call s:unplace_sign_at(sign_id)
    unlet s:signed[sign_idx]
  endif

  return ''
endfunction

function! clap#sign#get() abort
  return s:signed
endfunction

if s:is_nvim
  function! s:unplace_all_signs() abort
    if nvim_buf_is_valid(g:clap.display.bufnr)
      call sign_unplace(s:sign_group, {'buffer': g:clap.display.bufnr})
      call sign_unplace(s:sign_cur_group, {'buffer': g:clap.display.bufnr})
    endif
  endfunction
else
  " Now we close the popups, so don't have to clear the signs manually,
  " as the window and associated buffer will be deleted when you call
  " popup_close().
  function! s:unplace_all_signs() abort
  endfunction
endif

function! clap#sign#reset() abort
  call s:unplace_all_signs()
  let s:signed = []
  let s:last_signed_id = -1
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
