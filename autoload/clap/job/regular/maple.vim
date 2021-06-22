" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Spawn a regular job using maple binary.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:job_id = -1
let s:job_timer = -1

let s:maple_delay = get(g:, 'clap_maple_delay', 100)

function! s:on_complete() abort
  " At initial vim8.2, bufwinid(popup_bufnr) seemingly does not work as expected. Ref #223.
  " if bufwinid(g:clap.display.bufnr) == -1
  "
  " Some long-running jobs can be still running, but the window has been canceled by user.
  if g:clap.display.winid == -1
    return
  endif

  call clap#spinner#set_idle()

  " Skip the job processing if use already clears the input at the moment.
  if empty(g:clap.input.get())
    return
  endif

  if empty(s:chunks)
    if exists('g:__clap_lines_truncated_map')
      unlet g:__clap_lines_truncated_map
    endif
    return
  endif

  try
    let decoded = json_decode(s:chunks[0])
  catch
    echoerr '[maple]decoded on_complete:'.string(s:chunks)
    return
  endtry
  if has_key(decoded, 'error')
    call g:clap.display.set_lines([
          \ 'The external job runs into some issue:',
          \ 'jobid: '.s:job_id,
          \ 'executable: '.s:cmd[0],
          \ 'args: '.join(s:cmd[1:], ' '),
          \ 'error:',
          \ ] + split(decoded.error, "\n"))
    call clap#indicator#set_matches_number(0)
    call clap#sign#disable_cursorline()
    return
  endif

  if decoded.total == 0
    call g:clap.display.set_lines([g:clap_no_matches_msg])
    call clap#indicator#set_matches_number(0)
    call clap#sign#disable_cursorline()
    call g:clap#display_win.shrink_if_undersize()
    call g:clap.preview.hide()
    if exists('g:__clap_lines_truncated_map')
      unlet g:__clap_lines_truncated_map
    endif
    return
  endif

  call clap#state#refresh_matches_count(decoded.total)

  call g:clap.display.set_lines(s:Converter isnot v:null ? map(decoded.lines, 's:Converter(v:val)') : decoded.lines)

  if has_key(decoded, 'indices')
    call clap#highlight#add_fuzzy_async(decoded.indices)
  endif

  if has_key(decoded, 'truncated_map')
    let g:__clap_lines_truncated_map = decoded.truncated_map
  endif

  call clap#sign#reset_to_first_line()
  call g:clap#display_win.shrink_if_undersize()
endfunction

if has('nvim')

  function! s:on_event(job_id, data, event) abort
    " We only process the job that was spawned last time.
    if a:job_id == s:job_id
      if a:event ==# 'stdout'
        if len(a:data) > 1
          " Second last is the real last one for neovim.
          call extend(s:chunks, a:data[:-2])
        endif
      elseif a:event ==# 'stderr'
        " Ignore the error
      else
        call s:on_complete()
      endif
    endif
  endfunction

  function! s:start_maple() abort
    let s:job_id = clap#job#start_buffered(s:cmd, function('s:on_event'))
  endfunction

else

  function! s:close_cb(channel) abort
    if clap#job#vim8_job_id_of(a:channel) == s:job_id
      try
        let s:chunks = split(ch_readraw(a:channel), "\n")
        call s:on_complete()
      catch
        call clap#helper#echo_error(v:exception)
        call clap#spinner#set_idle()
      endtry
    endif
  endfunction

  function! s:start_maple() abort
    let s:job_id = clap#job#start_buffered(s:cmd, function('s:close_cb'))
  endfunction
endif

function! clap#job#regular#maple#stop() abort
  if s:job_id > 0
    call clap#job#stop(s:job_id)
    let s:job_id = -1
  endif
endfunction

function! s:apply_start(_timer) abort
  let s:chunks = []
  let g:clap.display.cache = []
  let s:Converter = get(g:clap.provider._(), 'converter', v:null)
  call g:clap.preview.hide()
  call s:start_maple()
endfunction

function! clap#job#regular#maple#start(cmd) abort
  if s:job_timer != -1
    call timer_stop(s:job_timer)
  endif

  call clap#job#regular#maple#stop()

  let s:cmd = a:cmd
  let s:job_timer = timer_start(s:maple_delay, function('s:apply_start'))
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
