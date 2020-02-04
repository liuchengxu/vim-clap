" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: APIs for working with Asynchronous jobs.

let s:save_cpo = &cpoptions
set cpoptions&vim

if has('nvim')
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
  function! clap#job#stop(job_id) abort
    " Kill it!
    silent! call jobstop(a:job_id, 'kill')
  endfunction

  function! clap#job#vim8_job_id_of(channel) abort
    return clap#job#parse_vim8_job_id(ch_getjob(a:channel))
  endfunction

  function! clap#job#parse_vim8_job_id(job_str) abort
    return str2nr(matchstr(a:job_str, '\d\+'))
  endfunction

  if has('win32')
    function! clap#job#wrap_cmd(cmd) abort
      return &shell . ' ' . &shellcmdflag . ' ' . a:cmd
    endfunction
  else
    function! clap#job#wrap_cmd(cmd) abort
      return split(&shell) + split(&shellcmdflag) + [a:cmd]
    endfunction
  endif

  function! clap#job#start_buffered(cmd, CloseCallback) abort
    let job = job_start(clap#job#wrap_cmd(a:cmd), {
          \ 'in_io': 'null',
          \ 'close_cb': a:CloseCallback,
          \ 'noblock': 1,
          \ 'mode': 'raw',
          \ })
    return clap#job#parse_vim8_job_id(string(job))
  endfunction
endif

function! clap#job#cwd() abort
  if clap#should_use_raw_cwd()
    return getcwd()
  else
    return clap#path#project_root_or_default(g:clap.start.bufnr)
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
