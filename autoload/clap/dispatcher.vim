" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Job control of async provider.

let s:save_cpo = &cpo
set cpo&vim

if has('nvim')

  function! s:apply_append_or_cache(raw_output) abort
    let raw_output = a:raw_output

    " Here are dragons!
    let line_count = g:clap.display.line_count()

    " Reach the preload capacity for the first time
    " Append the minimum raw_output, the rest goes to the cache.
    if len(raw_output) + line_count >= g:clap.display.preload_capacity
      let start = g:clap.display.preload_capacity - line_count
      let to_append = raw_output[:start-1]
      let to_cache = raw_output[start:]

      " Discard?
      call extend(g:clap.display.cache, to_cache)

      " Converter
      if has_key(g:clap.provider._(), 'converter')
        let to_append = map(to_append, 'g:clap.provider._().converter(v:val)')
      endif

      call g:clap.display.append_lines(to_append)

      let s:preload_is_complete = v:true
      let s:loaded_size = line_count + len(to_append)
    else
      let s:loaded_size = line_count + len(raw_output)
      if has_key(g:clap.provider._(), 'converter')
        let raw_output = map(raw_output, 'g:clap.provider._().converter(v:val)')
      endif
      call g:clap.display.append_lines(raw_output)
    endif
  endfunction

  function! s:append_output(data) abort
    if empty(a:data)
      return
    endif

    if s:preload_is_complete
      call extend(g:clap.display.cache, a:data)
    else
      call s:apply_append_or_cache(a:data)
    endif

    let matches_count = s:loaded_size + len(g:clap.display.cache)

    call clap#indicator#set_matches('['.matches_count.']')
  endfunction

  function! s:on_event(job_id, data, event) abort
    if a:event == 'stdout'
      if len(a:data) > 1
        " Second last is the real last one for neovim.
        call s:append_output(a:data[:-2])
      endif
    elseif a:event == 'stderr'
      " Ignore the errors?
    else
      call s:check_if_no_matches()
      call clap#sign#toggle_cursorline()
      call clap#spinner#set_idle()
    endif
  endfunction

  function! s:job_start(cmd) abort
    let s:jobid = jobstart(a:cmd, {
          \ 'on_exit': function('s:on_event'),
          \ 'on_stdout': function('s:on_event'),
          \ 'on_stderr': function('s:on_event'),
          \ })
  endfunction

  function! s:jobstop() abort
    if exists('s:jobid')
      silent! call jobstop(s:jobid)
      unlet s:jobid
    endif
  endfunction

else

  function! s:append_output(preload) abort
    let to_append = a:preload

    if has_key(g:clap.provider._(), 'converter')
      let to_append = map(to_append, 'g:clap.provider._().converter(v:val)')
    endif

    call g:clap.display.append_lines(to_append)
    let s:loaded_size = len(to_append)
    let s:preload_is_complete = v:true
    let s:did_preload = v:true
  endfunction

  function! s:update_indicator() abort
    if s:preload_is_complete
      let matches_count = s:loaded_size + len(g:clap.display.cache)
    else
      let matches_count = g:clap.display.line_count()
    endif

    call clap#indicator#set_matches('['.matches_count.']')
  endfunction

  function! s:post_check() abort
    if !s:preload_is_complete
      call s:append_output(s:vim_output)
    endif
    if s:check_if_no_matches() is v:false
      call clap#sign#toggle_cursorline()
    endif
    call clap#spinner#set_idle()
    call s:update_indicator()
  endfunction

  function! s:out_cb(channel, message) abort
    if s:preload_is_complete
      call add(g:clap.display.cache, a:message)
    else
      call add(s:vim_output, a:message)
      if len(s:vim_output) >= g:clap.display.preload_capacity
        call s:append_output(s:vim_output)
      endif
    endif
  endfunction

  function! s:err_cb(channel, message) abort
    call g:clap.abort(['channel: '.a:channel, 'message: '.a:message, 'cmd: '.s:executed_cmd])
  endfunction

  function! s:close_cb(_channel) abort
    call s:post_check()
  endfunction

  function! s:exit_cb(_job, _exit_code) abort
    call s:post_check()
  endfunction

  function! s:job_start(cmd) abort
    let s:jobid = job_start(['bash', '-c', a:cmd], {
          \ 'in_io': 'null',
          \ 'err_cb': function('s:err_cb'),
          \ 'out_cb': function('s:out_cb'),
          \ 'exit_cb': function('s:exit_cb'),
          \ 'close_cb': function('s:close_cb'),
          \ 'noblock': 1,
          \ })
  endfunction

  function! s:jobstop() abort
    if exists('s:jobid')
      " Kill it!
      silent! call jobstop(s:jobid, 'kill')
      unlet s:jobid
    endif
  endfunction

endif

function! s:check_if_no_matches() abort
  if g:clap.display.is_empty()
    call g:clap.display.set_lines([g:clap_no_matches_msg])
    call clap#indicator#set_matches('[0]')
    call clap#sign#disable_cursorline()
    return v:true
  else
    return v:false
  endif
endfunction

" Start a job given the command.
function! clap#dispatcher#jobstart(cmd) abort
  let s:cache_size = 0
  let s:loaded_size = 0
  let s:preload_is_complete = v:false

  let s:vim_output = []
  let g:clap.display.cache = []

  call s:jobstop()

  call s:job_start(a:cmd)
  let s:executed_cmd = strftime("%Y-%m-%d %H:%M:%S").' '.a:cmd
endfunction

function! clap#dispatcher#jobstop() abort
  call s:jobstop()
endfunction

let &cpo = s:save_cpo
unlet s:save_cpo
