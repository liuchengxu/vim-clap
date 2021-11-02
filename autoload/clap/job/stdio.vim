" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Manage a stdio-based service using job feature.
" There is at most one service per Vim/NeoVim session.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:job_id = -1

if has('nvim')

  let s:round_message = ''
  let s:content_length = 0

  function! s:handle_stdout(lines) abort
    while !empty(a:lines)
      let line = remove(a:lines, 0)

      if line ==# ''
        continue
      elseif s:content_length == 0
        if line =~# '^Content-length:'
          let s:content_length = str2nr(matchstr(line, '\d\+$'))
        else
          call clap#helper#echo_error('This should not happen, unknown message:'.line)
        endif
        continue
      endif

      if s:content_length < strlen(l:line)
        let s:round_message .= strpart(line, 0, s:content_length)
        call insert(a:lines, strpart(line, s:content_length))
        let s:content_length = 0
      else
        let s:round_message .= line
        let s:content_length -= strlen(l:line)
      endif

      " The message for this round is still incomplete, contintue to read more.
      if s:content_length > 0
        continue
      endif

      try
        call s:MessageHandler(trim(s:round_message))
      catch
        call clap#helper#echo_error('Failed to handle message:'.v:exception.', throwpoint:'.v:throwpoint)
      finally
        let s:round_message = ''
      endtry

    endwhile
  endfunction

  function! s:on_event(job_id, data, event) abort
    " We only process the job that was spawned last time.
    if a:job_id == s:job_id
      if a:event ==# 'stdout'
        call s:handle_stdout(a:data)
      elseif a:event ==# 'stderr'
        " Ignore the error
        if a:data == ['']
          return
        endif
        if g:clap_enable_debug
          call clap#helper#echo_error('on_stdio_event:'.string(a:data))
        endif
      else
        call clap#spinner#set_idle()
      endif
    endif
  endfunction

  function! s:start_service_job(cmd) abort
    call clap#job#stdio#stop_service()
    let g:clap.display.cache = []
    let s:job_id = jobstart(a:cmd, {
          \ 'on_exit': function('s:on_event'),
          \ 'on_stdout': function('s:on_event'),
          \ 'on_stderr': function('s:on_event'),
          \ })
    call clap#spinner#set_busy()
  endfunction

  function! clap#job#stdio#send_message(msg) abort
    call chansend(s:job_id, a:msg."\n")
  endfunction
else

  function! s:out_cb(channel, message) abort
    if s:job_id > 0 && a:channel == s:job_channel
      if empty(a:message)
        return
      endif
      try
        " Not sure if a change of vim itself, the message now is no longer
        " seperated by new line, hereby we try to split and take the last
        " item.
        let splitted = split(a:message, "\n")
        if !empty(splitted)
          call s:MessageHandler(splitted[-1])
        endif
      catch
        call clap#helper#echo_error('[stdio]Failed to handle message:'.a:message.', exception:'.v:exception.', throwpoint:'.v:throwpoint)
      endtry
    endif
  endfunction

  function! s:err_cb(channel, message) abort
    if s:job_id > 0 && clap#job#vim8_job_id_of(a:channel) == s:job_id
      call clap#helper#echo_error(a:message)
    endif
  endfunction

  function! s:exit_cb(channel, exit_code) abort
    call clap#spinner#set_idle()
  endfunction

  function! s:start_service_job(cmd_list) abort
    call clap#job#stdio#stop_service()
    call clap#spinner#set_busy()
    let g:clap.display.cache = []
    let s:job = job_start(a:cmd_list, {
          \ 'in_io': 'null',
          \ 'mode': 'raw',
          \ 'err_cb': function('s:err_cb'),
          \ 'out_cb': function('s:out_cb'),
          \ 'exit_cb': function('s:exit_cb'),
          \ 'noblock': 1,
          \ })
    let s:job_channel = job_getchannel(s:job)
    let s:job_id = clap#job#get_vim8_job_id(s:job)
    call clap#job#track(s:job_id, s:job)
  endfunction

  function! clap#job#stdio#send_message(msg) abort
    call ch_sendraw(s:job, a:msg."\n")
  endfunction
endif

function! clap#job#stdio#stop_service() abort
  if clap#job#exists(s:job_id)
    call clap#job#stop(s:job_id)
    let s:job_id = -1
  endif
endfunction

function! clap#job#stdio#start_service(MessageHandler, maple_cmd) abort
  let s:MessageHandler = a:MessageHandler
  let g:__clap_stdio_debug_maple_cmd = join(a:maple_cmd, ' ')
  call s:start_service_job(a:maple_cmd)
  return
endfunction

function! clap#job#stdio#start_rpc_service(MessageHandler) abort
  let s:MessageHandler = a:MessageHandler
  call s:start_service_job(clap#maple#build_cmd('rpc'))
  return
endfunction

function! clap#job#stdio#start_dyn_filter_service(MessageHandler, cmd) abort
  let s:MessageHandler = a:MessageHandler

  let filter_cmd = g:clap_enable_icon && g:clap.provider.id ==# 'files' ? ['--icon=File'] : []
  let filter_cmd += ['--number', '100', '--winwidth', winwidth(g:clap.display.winid), 'filter', g:clap.input.get(), '--cmd', a:cmd, '--cmd-dir', clap#rooter#working_dir()]

  call s:start_service_job(clap#maple#build_cmd_list(filter_cmd))
  return
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
