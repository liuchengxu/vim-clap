" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Draw a spinner to feel more responsive.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:frames = get(g:, 'clap_spinner_frames', ['◐', '◑', '◒', '◓'])
let s:prompt_format = get(g:, 'clap_prompt_format', '%spinner% %provider_id%> ')

let s:frame_index = 0
let s:spinner = s:frames[0]

" The spinner and current provider prompt are actually displayed in a same window.
function! s:compose_prompt() abort
  let l:prompt = s:prompt_format

  let l:spinner = s:spinner
  " let l:provider_id = get(g:, 'clap_forerunner_status_sign', '').g:clap.provider.id

  " Replace special markers with certain information.
  " \=l:variable is used to avoid escaping issues.
  " let l:prompt = substitute(l:prompt, '\V%spinner%', '\=l:spinner', 'g')
  " let l:prompt = substitute(l:prompt, '\V%provider_id%', '\=l:provider_id', 'g')
  let l:prompt = getcwd()

  if exists('s:spinner_rpc')
    return s:spinner_rpc
  endif

  return l:prompt
endfunction

if has('nvim')
  function! s:set_spinner() abort
    call clap#util#nvim_buf_set_lines(g:clap.spinner.bufnr, [s:compose_prompt()])
  endfunction
else
  function! s:set_spinner() abort
    call popup_settext(g:clap_spinner_winid, s:compose_prompt())
  endfunction
endif

function! clap#spinner#refresh() abort
  call s:set_spinner()
endfunction

function! clap#spinner#get_rpc() abort
  return s:spinner_rpc
endfunction

function! clap#spinner#set_rpc(spinner) abort
  let s:spinner_rpc = a:spinner
endfunction

function! clap#spinner#get() abort
  return s:compose_prompt()
endfunction

function! clap#spinner#width() abort
  return strdisplaywidth(s:compose_prompt())
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
