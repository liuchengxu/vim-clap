let s:save_cpo = &cpoptions
set cpoptions&vim

let s:job_id = -1
let s:job_timer = -1
let s:maple_delay = get(g:, 'clap_maple_delay', 100)

function! s:on_complete() abort
endfunction

if has('nvim')

  function! s:on_event(job_id, data, event) abort
    " We only process the job that was spawned last time.
    if a:job_id == s:job_id
      if a:event ==# 'stdout'
        call clap#provider#filer#handle_stdout(a:data)
      elseif a:event ==# 'stderr'
        " Ignore the error
      else
        call s:on_complete()
      endif
    endif
  endfunction

  function! s:start_maple() abort
    let s:job_id = jobstart(s:cmd, {
          \ 'on_exit': function('s:on_event'),
          \ 'on_stdout': function('s:on_event'),
          \ 'on_stderr': function('s:on_event'),
          \ })
    echom "job statred: ".s:job_id
    call clap#rpc#send()
  endfunction

else

  function! s:close_cb(channel) abort
    if clap#job#vim8_job_id_of(a:channel) == s:job_id
      let s:chunks = split(ch_readraw(a:channel), "\n")
      call s:on_complete()
    endif
  endfunction

  function! s:start_maple() abort
    let s:job_id = clap#job#start_buffered(s:cmd, function('s:close_cb'))
  endfunction
endif

function! clap#rpc#stop() abort
  if s:job_id > 0
    call clap#job#stop(s:job_id)
    let s:job_id = -1
  endif
endfunction

function! s:apply_start(_timer) abort
  let s:chunks = []

  call g:clap.preview.hide()
  call s:start_maple()
endfunction

function! clap#rpc#job_start(cmd) abort
  if s:job_timer != -1
    call timer_stop(s:job_timer)
  endif

  call clap#rpc#stop()

  let s:cmd = a:cmd
  let s:job_timer = timer_start(s:maple_delay, function('s:apply_start'))
  return
endfunction

function! clap#rpc#send() abort
  let dir = clap#spinner#get_rpc()
  let msg = json_encode({'method': 'open_file', 'params': {'cwd': dir}, 'id': 1})
  call chansend(s:job_id, msg."\n")
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
