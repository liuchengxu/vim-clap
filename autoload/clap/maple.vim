" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Minimalize the payload of external filter using maple's
" --number option, showing the top N items only.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:job_id = -1

" Use "%s" instead of bare %s in case of the query containing ';',
" e.g., rg --files | maple hello;world, world can be misinterpreted as a
" command.
let s:maple_bin = fnamemodify(g:clap#autoload_dir, ':h').'/target/release/maple'

if executable(s:maple_bin)
  let s:cmd_maple = s:maple_bin.' "%s"'
elseif executable('maple')
  let s:cmd_maple = 'maple "%s"'
else
  let s:cmd_maple = v:null
endif

function! clap#maple#is_available() abort
  return s:cmd_maple isnot v:null
endfunction

function! clap#maple#filter_cmd_fmt() abort
  return s:cmd_maple
endfunction

function! s:on_complete() abort
  call clap#spinner#set_idle()
  let decoded = json_decode(s:chunks[0])
  call clap#impl#refresh_matches_count(string(decoded.total))
  call g:clap.display.set_lines(decoded.lines)
  call clap#impl#add_highlight_for_fuzzy_indices(decoded.indices)
endfunction

if has('nvim')

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

  function! s:start_maple() abort
    let s:job_id = jobstart(s:cmd, {
          \ 'on_exit': function('s:on_event'),
          \ 'on_stdout': function('s:on_event'),
          \ 'on_stderr': function('s:on_event'),
          \ 'stdout_buffered': v:true,
          \ })
  endfunction

else

  function! s:close_cb(channel) abort
    if clap#job#vim8_job_id_of(a:channel) == s:job_id
      let s:chunks = split(ch_readraw(a:channel), "\n")
      call s:on_complete()
    endif
  endfunction

  function! s:start_maple() abort
    let job = job_start(clap#job#wrap_cmd(s:cmd), {
          \ 'in_io': 'null',
          \ 'close_cb': function('s:close_cb'),
          \ 'noblock': 1,
          \ 'mode': 'raw',
          \ })
    let s:job_id = clap#job#parse_vim8_job_id(string(job))
  endfunction
endif

function! clap#maple#stop() abort
  if s:job_id > 0
    call clap#job#stop(s:job_id)
    let s:job_id = -1
  endif
endfunction

function! clap#maple#job_start(cmd) abort
  call clap#maple#stop()
  let s:chunks = []
  let s:cmd = a:cmd.' --number '.g:clap.display.preload_capacity
  call s:start_maple()
  return
endfunction

function! clap#maple#exec(cmd) abort
  call clap#maple#stop()
  let s:chunks = []
  let s:cmd = a:cmd.' --cmd "%s"'.g:clap.display.preload_capacity
  call s:start_maple()
  return
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
