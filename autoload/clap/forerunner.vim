" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Spawn a job when initalizing the display window if possible.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:job_id = -1

if exists('g:clap_builtin_fuzzy_filter_threshold')
  let s:builtin_fuzzy_filter_threshold = g:clap_builtin_fuzzy_filter_threshold
elseif clap#filter#has_rust_ext()
  let s:builtin_fuzzy_filter_threshold = 100000
else
  let s:builtin_fuzzy_filter_threshold = 10000
endif

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
    let tmp = tempname()
    if writefile(s:chunks, tmp) == 0
      let g:__clap_forerunner_tempfile = tmp
    endif
    unlet s:chunks
  endif

  let g:clap_forerunner_status_sign = g:clap_forerunner_status_sign_done
  call clap#spinner#refresh()
endfunction

function! s:on_complete_maple() abort
  if !empty(s:chunks)
    let decoded = json_decode(s:chunks[0])

    if empty(g:clap.input.get())
      call g:clap.display.set_lines_lazy(decoded.lines)
    endif

    let g:clap.display.initial_size = decoded.total
    call clap#impl#refresh_matches_count(string(decoded.total))

    if has_key(decoded, 'tempfile')
      let g:__clap_forerunner_tempfile = decoded.tempfile
    else
      let g:__clap_forerunner_result = decoded.lines
    endif
  endif

  let g:clap_forerunner_status_sign = g:clap_forerunner_status_sign_done
  call clap#spinner#refresh()
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

function! s:on_event_maple(job_id, data, event) abort
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
      call s:on_complete_maple()
    endif
  endif
endfunction

function! s:close_cb(channel) abort
  if clap#job#vim8_job_id_of(a:channel) == s:job_id
    " https://github.com/vim/vim/issues/5143
    if ch_canread(a:channel)
      let s:chunks = split(ch_readraw(a:channel), "\n")
      call s:on_complete()
    endif
  endif
endfunction

function! s:close_cb_maple(channel) abort
  if clap#job#vim8_job_id_of(a:channel) == s:job_id
    if ch_canread(a:channel)
      let s:chunks = split(ch_readraw(a:channel), "\n")
      call s:on_complete_maple()
    endif
  endif
endfunction

if has('nvim')
  function! s:start_maple(cmd) abort
    let s:job_id = clap#job#start_buffered(a:cmd, function('s:on_event_maple'))
  endfunction

  function! s:start_forerunner(cmd) abort
    let s:job_id = clap#job#start_buffered(a:cmd, function('s:on_event'))
  endfunction
else
  function! s:start_maple(cmd) abort
    let s:job_id = clap#job#start_buffered(a:cmd, function('s:close_cb_maple'))
  endfunction

  function! s:start_forerunner(cmd) abort
    let s:job_id = clap#job#start_buffered(a:cmd, function('s:close_cb'))
  endfunction
endif

if clap#maple#is_available()
  let s:empty_filter_cmd = printf(clap#maple#filter_cmd_fmt(), '')

  function! s:into_maple_cmd(cmd) abort
    let cmd_dir = clap#rooter#working_dir()
    let cmd = printf('%s --cmd "%s" --cmd-dir "%s" --output-threshold %d',
          \ s:empty_filter_cmd,
          \ a:cmd,
          \ cmd_dir,
          \ s:builtin_fuzzy_filter_threshold,
          \ )
    return cmd
  endfunction

  function! clap#forerunner#start(cmd) abort
    let s:chunks = []
    let g:clap_forerunner_status_sign = '!'
    call clap#spinner#refresh()
    call s:start_maple(s:into_maple_cmd(a:cmd))
  endfunction
else
  function! clap#forerunner#start(cmd) abort
    let s:chunks = []
    let g:clap_forerunner_status_sign = g:clap_forerunner_status_sign_running
    call clap#spinner#refresh()
    call clap#rooter#run(function('s:start_forerunner'), a:cmd)
  endfunction
endif

function! clap#forerunner#stop() abort
  if s:job_id > 0
    call clap#job#stop(s:job_id)
    let s:job_id = -1
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
