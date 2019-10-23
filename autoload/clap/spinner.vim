" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Draw a spinner to feel more responsive.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')

let s:frames = ['◐', '◑', '◒', '◓']
" let s:frames = ["●∙∙", "∙●∙", "∙∙●"]
" let s:frames = map(s:frames, '" ".v:val." > "')
let s:frame_index = 0
let s:spinner = s:frames[0]

function! s:compose_spinner() abort
  return s:spinner.' '.g:clap.provider.id.'> '
endfunction

if s:is_nvim
  function! s:set_spinner() abort
    call clap#util#nvim_buf_set_lines(g:clap.spinner.bufnr, [s:compose_spinner()])
  endfunction
else
  function! s:set_spinner() abort
    call popup_settext(g:clap_spinner_winid, s:compose_spinner())
  endfunction
endif

function! clap#spinner#get() abort
  return s:compose_spinner()
endfunction

function! clap#spinner#width() abort
  return strdisplaywidth(s:compose_spinner())
endfunction

function! s:on_frame(...) abort
  let s:spinner = s:frames[s:frame_index]
  call s:set_spinner()
  let s:frame_index += 1
  let s:frame_index = s:frame_index % len(s:frames)
  if !g:clap.is_busy
    call timer_stop(s:timer)
    unlet s:timer
    let s:spinner = s:frames[0]
  endif
endfunction

function! clap#spinner#init() abort
  call s:set_spinner()
endfunction

function! clap#spinner#run() abort
  call s:set_spinner()
  if !exists('s:timer')
    let s:timer = timer_start(80, function('s:on_frame'), {'repeat': -1})
  endif
endfunction

function! clap#spinner#set_busy() abort
  let g:clap.is_busy = 1
  call clap#spinner#run()
endfunction

function! clap#spinner#set_idle() abort
  let g:clap.is_busy = 0
  if exists('s:timer')
    call timer_stop(s:timer)
    unlet s:timer
  endif
  let s:spinner = s:frames[0]
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
