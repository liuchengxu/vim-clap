" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: APIs for working with Asynchronous jobs.

let s:save_cpo = &cpoptions
set cpoptions&vim

if has('nvim')
  function! clap#job#exists(job_id) abort
    return a:job_id > 0
  endfunction

  function! clap#job#stop(job_id) abort
    silent! call jobstop(a:job_id)
  endfunction

  function! clap#job#start_buffered(cmd, OnEvent) abort
    let job_id = jobstart(a:cmd, {
          \ 'on_exit': a:OnEvent,
          \ 'on_stdout': a:OnEvent,
          \ 'on_stderr': a:OnEvent,
          \ 'stdout_buffered': v:true,
          \ })
    return job_id
  endfunction
else
  let s:job_id_map = {}

  function! clap#job#exists(job_id) abort
    return a:job_id > -1
  endfunction

  function! clap#job#stop(job_id) abort
    " Ignore the invalid job_id
    if has_key(s:job_id_map, a:job_id)
      " Kill it!
      call job_stop(remove(s:job_id_map, a:job_id), 'kill')
    endif
  endfunction

  function! clap#job#vim8_job_id_of(channel) abort
    return ch_info(a:channel)['id']
  endfunction

  function! clap#job#get_vim8_job_id(job) abort
    return ch_info(job_getchannel(a:job))['id']
  endfunction

  " wrap_cmd is only neccessary when cmd is a String, otherwise vim panics.
  if has('win32')
    function! clap#job#wrap_cmd(cmd) abort
      return &shell . ' ' . &shellcmdflag . ' ' . a:cmd
    endfunction
  else
    function! clap#job#wrap_cmd(cmd) abort
      return split(&shell) + split(&shellcmdflag) + [a:cmd]
    endfunction
  endif

  function! clap#job#start_buffered(cmd_list, CloseCallback) abort
    let job = job_start(a:cmd_list, {
          \ 'in_io': 'null',
          \ 'close_cb': a:CloseCallback,
          \ 'noblock': 1,
          \ 'mode': 'raw',
          \ })
    let job_id = ch_info(job_getchannel(job))['id']
    let s:job_id_map[job_id] = job
    return job_id
  endfunction

  function! clap#job#track(job_id, job) abort
    let s:job_id_map[a:job_id] = a:job
  endfunction
endif

let &cpoptions = s:save_cpo
unlet s:save_cpo
