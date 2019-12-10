" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Spawn a job when initalizing the display window if possible.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:job_id = -1

let s:builtin_fuzzy_filter_threshold = get(g:, 'g:clap_builtin_fuzzy_filter_threshold', 100000)

function! s:on_complete() abort
  if empty(g:clap.input.get())
    call g:clap.display.set_lines_lazy(s:chunks)
  endif

  let chunks_size = len(s:chunks)
  let g:clap.display.initial_size = chunks_size
  call clap#impl#refresh_matches_count(string(chunks_size))

  " If the total results is not huge we could keep them in the memory
  " and use the built-in fzy impl later.
  if chunks_size < s:builtin_fuzzy_filter_threshold
    " g:__clap_forerunner_result is sort of a cache here.
    " If we already have g:__clap_forerunner_result and you
    " just created a new file outside the vim, this new file maybe not recongnized.
    " TODO: add a flag to disable this cache.
    let g:__clap_forerunner_result = s:chunks
  else
    unlet s:chunks
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
  if clap#job#vim8_job_id_of(a:channel) == s:job_id
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
          \ })
  endfunction
else
  function! s:start_forerunner(cmd) abort
    let job = job_start(clap#job#wrap_cmd(a:cmd), {
          \ 'in_io': 'null',
          \ 'close_cb': function('s:close_cb'),
          \ 'noblock': 1,
          \ 'mode': 'raw',
          \ })
    let s:job_id = clap#job#parse_vim8_job_id(string(job))
  endfunction
endif

function! clap#forerunner#start(cmd) abort
  let s:chunks = []
  call clap#rooter#run(function('s:start_forerunner'), a:cmd)
endfunction

function! clap#forerunner#stop() abort
  if s:job_id > 0
    call clap#job#stop(s:job_id)
    let s:job_id = -1
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
