" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Job control of async provider.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:job_timer = -1
let s:dispatcher_delay = 300
let s:job_id = -1

let s:drop_cache = get(g:, 'clap_dispatcher_drop_cache', v:true)

function! s:jobstop() abort
  if s:job_id > 0
    call clap#job#stop(s:job_id)
    let s:job_id = -1
  endif
endfunction

function! s:put_raw_lines(lines) abort
  if s:has_converter
    let lines = map(a:lines, 's:Converter(v:val)')
  else
    let lines = a:lines
  endif

  " Set or append lines
  if s:did_set_lines
    call g:clap.display.append_lines(lines)
  else
    call g:clap.display.set_lines(lines)
    let s:did_set_lines = v:true
  endif
endfunction

if has('nvim')

  if s:drop_cache
    " to_cache is a List.
    function! s:handle_cache(to_cache) abort
      let s:dropped_size += len(a:to_cache)
    endfunction

    function! s:set_matches_count() abort
      let matches_count = s:loaded_size + s:dropped_size
      call clap#impl#refresh_matches_count(string(matches_count))
    endfunction
  else
    function! s:handle_cache(to_cache) abort
      call extend(g:clap.display.cache, a:to_cache)
    endfunction

    function! s:set_matches_count() abort
      let matches_count = s:loaded_size + len(g:clap.display.cache)
      call clap#impl#refresh_matches_count(string(matches_count))
    endfunction
  endif

  function! s:apply_append_or_cache(raw_output) abort
    let raw_output = a:raw_output

    " Reach the preload capacity for the first time
    " Append the minimum raw_output, the rest goes to the cache.
    if len(raw_output) + s:loaded_size >= g:clap.display.preload_capacity
      " Here are dragons!
      let start = g:clap.display.preload_capacity - s:loaded_size
      let to_append = raw_output[:start-1]
      let to_cache = raw_output[start :]

      " Discard?
      call s:handle_cache(to_cache)

      let s:preload_is_complete = v:true
      let s:loaded_size += len(to_append)

      let to_put = to_append
    else
      let s:loaded_size += len(raw_output)
      let to_put = raw_output
    endif

    call s:put_raw_lines(to_put)
  endfunction

  function! s:append_output(data) abort
    if empty(a:data)
      return
    endif

    if s:preload_is_complete
      call s:handle_cache(a:data)
    else
      call s:apply_append_or_cache(a:data)
    endif

    call s:set_matches_count()
  endfunction

  function! s:on_event(job_id, data, event) abort
    " We only process the job that was spawned last time.
    if s:job_id == a:job_id
      if a:event ==# 'stdout'
        " Second last is the real last one for neovim.
        call s:append_output(a:data[:-2])
      elseif a:event ==# 'stderr'
        if !empty(a:data) && a:data != ['']
          let error_info = [
                \ 'Error occurs when dispatching the command',
                \ 'job_id: '.a:job_id,
                \ 'working directory: '.(exists('g:__clap_provider_cwd') ? g:__clap_provider_cwd : getcwd()),
                \ 'command: '.s:executed_cmd,
                \ 'message: '
                \ ]
          let error_info += a:data
          call s:abort_job(error_info)
        endif
      else
        call s:on_exit_common()
      endif
    endif
  endfunction

  function! s:job_start(cmd) abort
    " We choose the lcd way instead of the cwd option of job for the
    " consistence purpose.
    let s:job_id = jobstart(a:cmd, {
          \ 'on_exit': function('s:on_event'),
          \ 'on_stdout': function('s:on_event'),
          \ 'on_stderr': function('s:on_event'),
          \ })
  endfunction

else

  if s:drop_cache
    function! s:handle_cache(_line_to_cache) abort
      let s:dropped_size += 1
    endfunction

    function! s:matched_count_when_preload_is_complete() abort
      return s:loaded_size + s:dropped_size
    endfunction
  else
    function! s:handle_cache(line_to_cache) abort
      call add(g:clap.display.cache, a:line_to_cache)
    endfunction

    function! s:matched_count_when_preload_is_complete() abort
      return s:loaded_size + len(g:clap.display.cache)
    endfunction
  endif

  function! s:append_output(preload) abort
    if empty(a:preload)
      return
    endif

    call s:put_raw_lines(a:preload)

    let s:loaded_size = len(a:preload)
    let s:preload_is_complete = v:true
  endfunction

  function! s:update_indicator() abort
    if s:preload_is_complete
      let matches_count = s:matched_count_when_preload_is_complete()
    else
      let matches_count = g:clap.display.line_count()
    endif

    call clap#impl#refresh_matches_count(string(matches_count))
  endfunction

  function! s:post_check() abort
    if !s:preload_is_complete
      call s:append_output(s:vim_output)
    endif
    call s:on_exit_common()
    call s:update_indicator()
  endfunction

  function! s:out_cb(channel, message) abort
    if s:job_id > 0 && clap#job#vim8_job_id_of(a:channel) == s:job_id
      if s:preload_is_complete
        call s:handle_cache(a:message)
      else
        call add(s:vim_output, a:message)
        if len(s:vim_output) >= g:clap.display.preload_capacity
          call s:append_output(s:vim_output)
        endif
      endif
    endif
  endfunction

  function! s:err_cb(channel, message) abort
    if s:job_id > 0 && clap#job#vim8_job_id_of(a:channel) == s:job_id
      let error_info = [
            \ 'Error occurs when dispatching the command',
            \ 'working directory: '.(exists('g:__clap_provider_cwd') ? g:__clap_provider_cwd : getcwd()),
            \ 'channel: '.a:channel,
            \ 'message: '.string(a:message),
            \ 'command: '.s:executed_cmd,
            \ ]
      call s:abort_job(error_info)
    endif
  endfunction

  function! s:close_cb(channel) abort
    if s:job_id > 0 && clap#job#vim8_job_id_of(a:channel) == s:job_id
      call s:post_check()
    endif
  endfunction

  function! s:exit_cb(job, _exit_code) abort
    if s:job_id > 0 && clap#job#parse_vim8_job_id(a:job) == s:job_id
      call s:post_check()
    endif
  endfunction

  function! s:job_start(cmd) abort
    let job = job_start(clap#job#wrap_cmd(a:cmd), {
          \ 'in_io': 'null',
          \ 'err_cb': function('s:err_cb'),
          \ 'out_cb': function('s:out_cb'),
          \ 'exit_cb': function('s:exit_cb'),
          \ 'close_cb': function('s:close_cb'),
          \ 'noblock': 1,
          \ })
    let s:job_id = clap#job#parse_vim8_job_id(string(job))
  endfunction

endif

function! s:abort_job(error_info) abort
  call s:jobstop()
  call g:clap.display.set_lines(a:error_info)
  call clap#spinner#set_idle()
endfunction

function! s:on_exit_common() abort
  if s:has_no_matches()
    call g:clap.display.set_lines([g:clap_no_matches_msg])
    call clap#indicator#set_matches('[0]')
    call clap#sign#disable_cursorline()
  else
    call clap#sign#reset_to_first_line()
  endif
  call clap#spinner#set_idle()
endfunction

function! s:has_no_matches() abort
  let g:__clap_has_no_matches = !s:did_set_lines
  return g:__clap_has_no_matches
endfunction

function! s:apply_job_start(_timer) abort
  call clap#rooter#run(function('s:job_start'), s:cmd)

  let s:executed_time = strftime('%Y-%m-%d %H:%M:%S')
  let s:executed_cmd = s:cmd
endfunction

function! s:prepare_job_start(cmd) abort
  call s:jobstop()

  let s:cache_size = 0
  let s:loaded_size = 0
  let s:dropped_size = 0
  let g:clap.display.cache = []
  let s:preload_is_complete = v:false
  let s:did_set_lines = v:false

  let s:cmd = a:cmd

  let s:vim_output = []

  let s:has_converter = has_key(g:clap.provider._(), 'converter')
  if s:has_converter
    let s:Converter = g:clap.provider._().converter
  endif
endfunction

function! s:job_strart_with_delay() abort
  if s:job_timer != -1
    call timer_stop(s:job_timer)
  endif

  let s:job_timer = timer_start(s:dispatcher_delay, function('s:apply_job_start'))
endfunction

" Start a job immediately given the command.
function! clap#dispatcher#job_start(cmd) abort
  call s:prepare_job_start(a:cmd)
  call s:apply_job_start('')
endfunction

" Start a job with a delay given the command.
function! clap#dispatcher#job_start_with_delay(cmd) abort
  call s:prepare_job_start(a:cmd)
  call s:job_strart_with_delay()
endfunction

function! clap#dispatcher#jobstop() abort
  call s:jobstop()
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
