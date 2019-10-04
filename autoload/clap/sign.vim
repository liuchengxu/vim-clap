" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Multi selection support.

let s:save_cpo = &cpo
set cpo&vim

let s:signed = []
let s:sign_group = 'clapSelected'
let s:sign_cur_group = 'clapCurrentSelected'
let s:last_signed_id = -1

if !exists('s:sign_inited')
  call sign_define(s:sign_group, {'text': ' >', 'texthl': "WarningMsg", "linehl": "PmenuSel"})
  call sign_define(s:sign_cur_group, {'text': '>>', 'texthl': "WarningMsg", "linehl": "PmenuSel"})
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

function! clap#sign#toggle() abort
  let curlnum = g:clap.display.getcurlnum()

  let sign_idx = index(s:signed, curlnum)
  if sign_idx == -1
    call sign_place(curlnum, s:sign_group, 'clapSelected', g:clap.display.bufnr, {'lnum': curlnum})
    call add(s:signed, curlnum)
  else
    let sign_id = s:signed[sign_idx]
    call sign_unplace(s:sign_group, {'buffer': g:clap.display.bufnr, 'id': sign_id})
    unlet s:signed[sign_idx]
  endif

  return ''
endfunction

function! clap#sign#get() abort
  return s:signed
endfunction

function! clap#sign#reset() abort
  " Now we close the popups, so don't have to clear the signs manually,
  " as the window and associated buffer will be deleted when you call
  " popup_close().
  "
  " call sign_unplace(s:sign_group, {'buffer': g:clap.display.bufnr})
  " call sign_unplace(s:sign_cur_group, {'buffer': g:clap.display.bufnr})

  let s:signed = []
  let s:last_signed_id = -1
endfunction

let &cpo = s:save_cpo
unlet s:save_cpo
