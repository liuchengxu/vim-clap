" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Spawn a job when initalizing the display window if possible.

function! s:on_complete() abort
  if empty(g:clap.input.get())
    call g:clap.display.set_lines_lazy(s:chunks)
  endif
  let chunks_size = len(s:chunks)
  if chunks_size < 10000
    let g:__clap_forerunner_cached = s:chunks
    let g:clap.display.initial_size = chunks_size
    call clap#impl#refresh_matches_count(string(chunks_size))
  endif
endfunction

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

function! s:close_cb(channel) abort
  if clap#util#job_id_of(a:channel) == s:job_id
    " https://github.com/vim/vim/issues/5143
    let s:chunks = split(ch_readraw(a:channel), "\n")
    call s:on_complete()
  endif
endfunction

if has('nvim')
  function! s:start_forerunner(cmd) abort
    let s:job_id = jobstart(a:cmd, {
          \ 'on_exit': function('s:on_event'),
          \ 'on_stdout': function('s:on_event'),
          \ 'on_stderr': function('s:on_event'),
          \ 'stdout_buffered': v:true,
          \ 'cwd': clap#job#cwd(),
          \ })
  endfunction
else
  function! s:start_forerunner(cmd) abort
    let job = job_start(['bash', '-c', a:cmd], {
          \ 'in_io': 'null',
          \ 'close_cb': function('s:close_cb'),
          \ 'noblock': 1,
          \ 'mode': 'raw',
          \ 'cwd': clap#job#cwd(),
          \ })
    let s:job_id = clap#util#parse_vim8_job_id(string(job))
  endfunction
endif

function! clap#forerunner#start(cmd) abort
  let s:chunks = []
  call s:start_forerunner(a:cmd)
endfunction
