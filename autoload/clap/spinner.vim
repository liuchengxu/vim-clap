" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Draw a spinner to feel more responsive.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:frames = get(g:, 'clap_spinner_frames', ['⠋', '⠙', '⠚', '⠞', '⠖', '⠦', '⠴', '⠲', '⠳', '⠓'])
let s:frames_len = len(s:frames)
let s:prompt_format = get(g:, 'clap_prompt_format', ' %spinner%%forerunner_status%%provider_id%:')

let s:frame_index = 0
let s:spinner = s:frames[0]

let g:__clap_current_forerunner_status = g:clap_forerunner_status_sign.running

" The spinner and current provider prompt are actually displayed in a same window.
function! s:fill_in_placeholders(prompt_format) abort
  let l:prompt = a:prompt_format

  let l:provider_id = g:clap.provider.id

  " Replace special markers with certain information.
  " \=l:variable is used to avoid escaping issues.
  let l:prompt = substitute(l:prompt, '\V%spinner%', '\=s:spinner', 'g')
  let l:prompt = substitute(l:prompt, '\V%forerunner_status%', '\=g:__clap_current_forerunner_status', 'g')
  let l:prompt = substitute(l:prompt, '\V%provider_id%', '\=l:provider_id', 'g')

  return l:prompt
endfunction

if exists('g:ClapPrompt') && type(g:ClapPrompt) == v:t_func
  function! s:user_prompt() abort
    return s:fill_in_placeholders(g:ClapPrompt())
  endfunction
  let s:PromptFn = function('s:user_prompt')
else
  function! s:default_prompt() abort
    return s:fill_in_placeholders(s:prompt_format)
  endfunction
  let s:PromptFn = function('s:default_prompt')
endif

function! s:generate_prompt() abort
  " Provider level prompt format has higher priority.
  if has_key(g:clap.provider._(), 'prompt_format')
    return s:fill_in_placeholders(g:clap.provider._().prompt_format)
  else
    return s:PromptFn()
  endif
endfunction

if has('nvim')
  function! clap#spinner#set(text) abort
    let s:current_prompt = s:fill_in_placeholders(a:text)
    call setbufline(g:clap.spinner.bufnr, 1, s:current_prompt)
    call g:clap#floating_win#spinner.shrink()
  endfunction

  function! clap#spinner#set_raw(text) abort
    let s:current_prompt = a:text
    call setbufline(g:clap.spinner.bufnr, 1, s:current_prompt)
    call g:clap#floating_win#spinner.shrink()
  endfunction

  function! s:set_spinner() abort
    let s:current_prompt = s:generate_prompt()
    call clap#spinner#set(s:current_prompt)
  endfunction
else
  function! clap#spinner#set(text) abort
    let s:current_prompt = s:fill_in_placeholders(a:text)
    call popup_settext(g:clap_spinner_winid, s:current_prompt)
    call clap#popup#shrink_spinner()
  endfunction

  function! clap#spinner#set_raw(text) abort
    let s:current_prompt = a:text
    call popup_settext(g:clap_spinner_winid, s:current_prompt)
    call clap#popup#shrink_spinner()
  endfunction

  function! s:set_spinner() abort
    let s:current_prompt = s:generate_prompt()
    call clap#spinner#set(s:current_prompt)
  endfunction
endif

function! clap#spinner#refresh() abort
  call s:set_spinner()
endfunction

function! clap#spinner#get() abort
  return s:current_prompt
endfunction

function! clap#spinner#width() abort
  if !exists('s:current_prompt')
    let s:current_prompt = s:generate_prompt()
  endif
  return strdisplaywidth(s:current_prompt)
endfunction

function! s:on_frame(...) abort
  let s:spinner = s:frames[s:frame_index]
  call s:set_spinner()
  let s:frame_index += 1
  let s:frame_index = s:frame_index % s:frames_len
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
