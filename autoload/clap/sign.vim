" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Multi selection support.

let s:signed = []
let s:sign_group = 'clapSelected'

if !exists('s:sign_inited')
  call sign_define(s:sign_group, {'text': ' >', 'texthl': "WarningMsg", "linehl": "PmenuSel"})
  let s:sign_inited = 1
endif

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
  call sign_unplace(s:sign_group, {'buffer': g:clap.display.bufnr})
  let s:signed = []
endfunction
