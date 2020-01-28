" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Maple RPC service.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:job_id = -1

function! s:on_complete() abort
endfunction

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
        call clap#helper#echo_error('Failed to handle message:'.v:exception)
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
      else
        call s:on_complete()
      endif
    endif
  endfunction

  function! s:start_rpc() abort
    let s:job_id = jobstart(s:cmd, {
          \ 'on_exit': function('s:on_event'),
          \ 'on_stdout': function('s:on_event'),
          \ 'on_stderr': function('s:on_event'),
          \ })
  endfunction

  function! clap#rpc#send_message(msg) abort
    call chansend(s:job_id, a:msg."\n")
  endfunction

else

  function! s:out_cb(channel, message) abort
    if s:job_id > 0 && clap#job#vim8_job_id_of(a:channel) == s:job_id
      " call clap#provider#filer#handle_stdout(a:message)
      if a:message =~# '^Content-length:' || a:message ==# ''
        return
      endif
      try
        call s:MessageHandler(a:message)
      catch
        call clap#helper#echo_error('Failed to handle message:'.a:message.', exception:'.v:exception)
      endtry
    endif
  endfunction

  function! s:err_cb(channel, message) abort
    if s:job_id > 0 && clap#job#vim8_job_id_of(a:channel) == s:job_id
      echom 'error callback'
    endif
  endfunction

  function! s:close_cb(channel) abort
    if s:job_id > 0 && clap#job#vim8_job_id_of(a:channel) == s:job_id
      echom 'close callback'
    endif
  endfunction

  function! s:exit_cb(job, _exit_code) abort
    if s:job_id > 0 && clap#job#parse_vim8_job_id(a:job) == s:job_id
      echom 'exit callback'
    endif
  endfunction

  function! s:start_rpc() abort
    let s:job = job_start(clap#job#wrap_cmd(s:cmd), {
          \ 'err_cb': function('s:err_cb'),
          \ 'out_cb': function('s:out_cb'),
          \ 'exit_cb': function('s:exit_cb'),
          \ 'close_cb': function('s:close_cb'),
          \ 'noblock': 1,
          \ })
    let s:job_id = clap#job#parse_vim8_job_id(string(s:job))
  endfunction

  function! clap#rpc#send_message(msg) abort
    call ch_sendraw(s:job, a:msg."\n")
  endfunction
endif

function! clap#rpc#stop() abort
  if s:job_id > 0
    call clap#job#stop(s:job_id)
    let s:job_id = -1
  endif
endfunction

function! clap#rpc#start(MessageHandler) abort
  call clap#rpc#stop()

  let s:chunks = []
  call g:clap.preview.hide()
  let s:cmd = clap#maple#run('rpc')
  let s:MessageHandler = a:MessageHandler
  call s:start_rpc()
  return
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
