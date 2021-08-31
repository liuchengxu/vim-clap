" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: A long-running stdio service in the background.

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
        call clap#helper#echo_error('[daemon]Failed to handle message:'.v:exception.', throwpoint:'.v:throwpoint)
      finally
        let s:round_message = ''
      endtry

    endwhile
  endfunction

  function! s:on_event(job_id, data, event) abort
    " ~~We only process the job that was spawned last time.~~
    " We spawn the daemon service only once.
    if a:event ==# 'stdout'
      call s:handle_stdout(a:data)
    elseif a:event ==# 'stderr'
      " Ignore the error
      if a:data == ['']
        return
      endif
      call clap#helper#echo_error('on_event:'.string(a:data))
    endif
  endfunction

  function! s:start_service_job(cmd) abort
    call clap#job#daemon#stop()
    let s:job_id = jobstart(a:cmd, {
          \ 'on_exit': function('s:on_event'),
          \ 'on_stdout': function('s:on_event'),
          \ 'on_stderr': function('s:on_event'),
          \ })
  endfunction

  function! clap#job#daemon#send_message(msg) abort
    call chansend(s:job_id, a:msg."\n")
  endfunction
else

  function! s:out_cb(channel, message) abort
    if clap#job#exists(s:job_id) && a:channel == s:job_channel
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
    if clap#job#exists(s:job_id) && clap#job#vim8_job_id_of(a:channel) == s:job_id
      call clap#helper#echo_error(a:message)
    endif
  endfunction

  function! s:start_service_job(cmd_list) abort
    call clap#job#daemon#stop()
    let s:job = job_start(a:cmd_list, {
          \ 'err_cb': function('s:err_cb'),
          \ 'out_cb': function('s:out_cb'),
          \ 'noblock': 1,
          \ })
    let s:job_channel = job_getchannel(s:job)
    let s:job_id = clap#job#get_vim8_job_id(s:job)
    call clap#job#track(s:job_id, s:job)
  endfunction

  function! clap#job#daemon#send_message(msg) abort
    call ch_sendraw(s:job, a:msg."\n")
  endfunction
endif

function! clap#job#daemon#stop() abort
  if clap#job#exists(s:job_id)
    call clap#job#stop(s:job_id)
    let s:job_id = -1
  endif
endfunction

function! clap#job#daemon#is_running() abort
  return s:job_id != -1
endfunction

function! clap#job#daemon#start(MessageHandler) abort
  let s:MessageHandler = a:MessageHandler
  call s:start_service_job(clap#maple#build_cmd('rpc'))
  call clap#client#notify('initialize_global_env', {
      \   'is_nvim': has('nvim') ? v:true : v:false,
      \   'enable_icon': g:clap_enable_icon ? v:true : v:false,
      \   'clap_preview_size': g:clap_preview_size,
      \ })
  call clap#client#call('init_ext_map', v:null, {'autocmd_filetypedetect': execute('autocmd filetypedetect')})
  return
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
