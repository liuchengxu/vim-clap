" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Maple RPC service.

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
        call clap#helper#echo_error('on_event:'.string(a:data))
      endif
    endif
  endfunction

  function! s:start_rpc() abort
    let s:job_id = jobstart(s:rpc_cmd, {
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
      call clap#helper#echo_error(a:message)
    endif
  endfunction

  function! s:start_rpc() abort
    let s:job = job_start(clap#job#wrap_cmd(s:rpc_cmd), {
          \ 'err_cb': function('s:err_cb'),
          \ 'out_cb': function('s:out_cb'),
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
  let s:MessageHandler = a:MessageHandler
  let s:rpc_cmd = clap#maple#run('rpc')
  call s:start_rpc()
  return
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
