" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Job control of async provider.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:job_timer = -1
let s:dispatcher_delay = 300
let s:job_id = -1

let s:is_win = has('win32')

let s:drop_cache = get(g:, 'clap_dispatcher_drop_cache', v:true)

if has('nvim')

  if s:drop_cache
    " to_cache is a List.
    function! s:handle_cache(to_cache) abort
      let s:droped_size += len(a:to_cache)
    endfunction

    function! s:set_matches_count() abort
      let matches_count = s:loaded_size + s:droped_size
      call clap#indicator#set_matches('['.matches_count.']')
    endfunction
  else
    function! s:handle_cache(to_cache) abort
      call extend(g:clap.display.cache, a:to_cache)
    endfunction

    function! s:set_matches_count() abort
      let matches_count = s:loaded_size + len(g:clap.display.cache)
      call clap#indicator#set_matches('['.matches_count.']')
    endfunction
  endif

  function! s:apply_append_or_cache(raw_output) abort
    let raw_output = a:raw_output

    " Here are dragons!
    let line_count = g:clap.display.line_count()

    " Reach the preload capacity for the first time
    " Append the minimum raw_output, the rest goes to the cache.
    if len(raw_output) + line_count >= g:clap.display.preload_capacity
      let start = g:clap.display.preload_capacity - line_count
      let to_append = raw_output[:start-1]
      let to_cache = raw_output[start :]

      " Discard?
      call s:handle_cache(to_cache)

      " Converter
      if s:has_converter
        let to_append = map(to_append, 's:Converter(v:val)')
      endif

      call g:clap.display.append_lines(to_append)

      let s:preload_is_complete = v:true
      let s:loaded_size = line_count + len(to_append)
    else
      if s:loaded_size == 0
        let s:loaded_size = len(raw_output)
      else
        let s:loaded_size = line_count + len(raw_output)
      endif
      if s:has_converter
        let raw_output = map(raw_output, 's:Converter(v:val)')
      endif
      call g:clap.display.append_lines(raw_output)
    endif

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
    if s:job_id == -1 || a:job_id != s:job_id
      return
    endif

    if a:event ==# 'stdout'
      if len(a:data) > 1
        " Second last is the real last one for neovim.
        call s:append_output(a:data[:-2])
      endif
    elseif a:event ==# 'stderr'
      if !empty(a:data) && a:data != ['']
        let error_info = [
              \ 'Error occurs when dispatching the command',
              \ 'job_id: '.a:job_id,
              \ 'command: '.s:executed_cmd,
              \ 'message: '
              \ ]
        let error_info += a:data
        call s:abort_job(error_info)
      endif
    else
      call s:on_exit_common()
    endif
  endfunction

  function! s:job_start(cmd) abort
    let s:job_id = jobstart(a:cmd, {
          \ 'on_exit': function('s:on_event'),
          \ 'on_stdout': function('s:on_event'),
          \ 'on_stderr': function('s:on_event'),
          \ 'cwd': s:job_cwd(),
          \ })
  endfunction

  function! s:jobstop() abort
    if s:job_id > 0
      silent! call jobstop(s:job_id)
      let s:job_id = -1
    endif
  endfunction

else

  if s:drop_cache
    function! s:handle_cache(_line_to_cache) abort
      let s:droped_size += 1
    endfunction

    function! s:matched_count_when_preload_is_complete() abort
      return s:loaded_size + s:droped_size
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
    let to_append = a:preload

    if s:has_converter
      let to_append = map(to_append, 's:Converter(v:val)')
    endif

    call g:clap.display.append_lines(to_append)
    let s:loaded_size = len(to_append)
    let s:preload_is_complete = v:true
    let s:did_preload = v:true
  endfunction

  function! s:update_indicator() abort
    if s:preload_is_complete
      let matches_count = s:matched_count_when_preload_is_complete()
    else
      let matches_count = g:clap.display.line_count()
    endif

    call clap#indicator#set_matches('['.matches_count.']')
  endfunction

  function! s:post_check() abort
    if !s:preload_is_complete
      call s:append_output(s:vim_output)
    endif
    call s:on_exit_common()
    call s:update_indicator()
  endfunction

  function! s:parse_job_id(job_str) abort
    return str2nr(matchstr(a:job_str, '\d\+'))
  endfunction

  function! s:job_id_of(channel) abort
    return s:parse_job_id(ch_getjob(a:channel))
  endfunction

  function! s:out_cb(channel, message) abort
    if s:job_id > 0 && s:job_id_of(a:channel) == s:job_id
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
    if s:job_id > 0 && s:job_id_of(a:channel) == s:job_id
      let error_info = [
            \ 'Error occurs when dispatching the command',
            \ 'channel: '.a:channel,
            \ 'message: '.string(a:message),
            \ 'command: '.s:executed_cmd,
            \ ]
      call s:abort_job(error_info)
    endif
  endfunction

  function! s:close_cb(channel) abort
    if s:job_id > 0 && s:job_id_of(a:channel) == s:job_id
      call s:post_check()
    endif
  endfunction

  function! s:exit_cb(job, _exit_code) abort
    if s:job_id > 0 && s:parse_job_id(a:job) == s:job_id
      call s:post_check()
    endif
  endfunction

  function! s:job_start(cmd) abort
    if s:is_win
      let cmd = &shell . ' ' . &shellcmdflag . ' ' . a:cmd
    else
      let cmd = split(&shell) + split(&shellcmdflag) + [a:cmd]
    endif
    let job = job_start(cmd, {
          \ 'in_io': 'null',
          \ 'err_cb': function('s:err_cb'),
          \ 'out_cb': function('s:out_cb'),
          \ 'exit_cb': function('s:exit_cb'),
          \ 'close_cb': function('s:close_cb'),
          \ 'noblock': 1,
          \ 'cwd': s:job_cwd(),
          \ })
    let s:job_id = s:parse_job_id(string(job))
  endfunction

  function! s:jobstop() abort
    if s:job_id > 0
      " Kill it!
      silent! call jobstop(s:job_id, 'kill')
      let s:job_id = -1
    endif
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
  if exists('g:__clap_maple_fuzzy_matched')
    let hl_lines = g:__clap_maple_fuzzy_matched[:g:clap.display.line_count()-1]
    call clap#impl#add_highlight_for_fuzzy_indices(hl_lines)
  endif
endfunction

function! s:has_no_matches() abort
  if g:clap.display.is_empty()
    return v:true
  else
    return v:false
  endif
endfunction

function! s:job_cwd() abort
  if get(g:, 'clap_disable_run_rooter', v:false)
    return getcwd()
  else
    let git_root = clap#util#find_git_root(g:clap.start.bufnr)
    return empty(git_root) ? getcwd() : git_root
  endif
endfunction

function! s:apply_job_start(_timer) abort
  call s:job_start(s:cmd)

  let s:executed_time = strftime('%Y-%m-%d %H:%M:%S')
  let s:executed_cmd = s:cmd
endfunction

function! s:prepare_job_start(cmd) abort
  call s:jobstop()

  let s:cache_size = 0
  let s:loaded_size = 0
  let g:clap.display.cache = []
  let s:preload_is_complete = v:false
  let s:droped_size = 0

  let s:cmd = a:cmd

  let s:vim_output = []

  if has_key(g:clap.provider._(), 'converter')
    let s:has_converter = v:true
    let s:Converter = g:clap.provider._().converter
  else
    let s:has_converter = v:false
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
