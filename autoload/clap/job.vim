" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: APIs for working with Asynchronous jobs.

let s:save_cpo = &cpoptions
set cpoptions&vim

if has('nvim')
  function! clap#job#stop(job_id) abort
    silent! call jobstop(a:job_id)
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

endif

function! clap#job#cwd() abort
  if clap#should_use_raw_cwd()
    return getcwd()
  else
    let git_root = clap#util#find_git_root(g:clap.start.bufnr)
    return empty(git_root) ? getcwd() : git_root
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
