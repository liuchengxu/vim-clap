" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Dispatch the job via maple.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:job_id = -1
let s:job_timer = -1
let s:maple_delay = get(g:, 'clap_maple_delay', 100)

let s:maple_bin = fnamemodify(g:clap#autoload_dir, ':h').'/target/release/maple'

if executable(s:maple_bin)
  let s:maple_filter_cmd = s:maple_bin.' "%s"'
  let s:empty_filter_cmd = printf(s:maple_filter_cmd, '')
elseif executable('maple')
  let s:maple_filter_cmd = 'maple "%s"'
  let s:empty_filter_cmd = 'maple ""'
else
  let s:maple_filter_cmd = v:null
endif

function! clap#maple#is_available() abort
  return s:maple_filter_cmd isnot v:null
endfunction

function! clap#maple#filter_cmd_fmt() abort
  return s:maple_filter_cmd
endfunction

function! s:on_complete() abort
  " Some long-running jobs can be still running, but the window has been canceled by user.
  if bufwinid(g:clap.display.bufnr) == -1
    return
  endif

  call clap#spinner#set_idle()

  if empty(s:chunks)
    return
  endif

  let decoded = json_decode(s:chunks[0])
  if has_key(decoded, 'error')
    call g:clap.display.set_lines([
          \ 'The external job runs into some issue:',
          \ 'jobid: '.s:job_id,
          \ 'executable: '.split(s:cmd)[0],
          \ 'args: '.join(split(s:cmd)[1:], ' '),
          \ 'error:',
          \ ] + split(decoded.error, "\n"))
    call clap#indicator#set_matches('[0]')
    call clap#sign#disable_cursorline()
    return
  endif

  if decoded.total == 0
    call g:clap.display.set_lines([g:clap_no_matches_msg])
    call clap#indicator#set_matches('[0]')
    call clap#sign#disable_cursorline()
    call g:clap#display_win.compact_if_undersize()
    call g:clap.preview.hide()
    return
  endif

  call clap#impl#refresh_matches_count(string(decoded.total))

  if s:has_converter
    call g:clap.display.set_lines(map(decoded.lines, 's:Converter(v:val)'))
  else
    call g:clap.display.set_lines(decoded.lines)
  endif

  if has_key(decoded, 'indices')
    call clap#impl#add_highlight_for_fuzzy_indices(decoded.indices)
  endif

  call clap#sign#reset_to_first_line()
  call g:clap#display_win.compact_if_undersize()
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
    let s:job_id = clap#job#start_buffered(s:cmd, function('s:on_event'))
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

function! clap#maple#stop() abort
  if s:job_id > 0
    call clap#job#stop(s:job_id)
    let s:job_id = -1
  endif
endfunction

function! s:apply_start(_timer) abort
  let s:chunks = []

  if has_key(g:clap.provider._(), 'converter')
    let s:has_converter = v:true
    let s:Converter = g:clap.provider._().converter
  else
    let s:has_converter = v:false
  endif

  call g:clap.preview.hide()
  call s:start_maple()
endfunction

function! clap#maple#job_start(cmd) abort
  if s:job_timer != -1
    call timer_stop(s:job_timer)
  endif

  call clap#maple#stop()

  let s:cmd = a:cmd.' --number '.g:clap.display.preload_capacity

  if g:clap.provider.id ==# 'files' && g:clap_enable_icon
    let s:cmd .= ' --enable-icon'
  endif

  let s:job_timer = timer_start(s:maple_delay, function('s:apply_start'))
  return
endfunction

let s:can_enable_icon = ['files', 'git_files']

function! clap#maple#try_enable_icon(cmd) abort
  if g:clap_enable_icon
        \ && index(s:can_enable_icon, g:clap.provider.id) > -1
    return a:cmd . ' --enable-icon'
  else
    return a:cmd
  endif
endfunction

" Run the command via maple to minimalize the payload of this job.
"
" Call clap#rooter#try_set_cwd() if neccessary so that the cmd working dir is
" right.
function! clap#maple#execute(cmd) abort
  let cmd_dir = clap#rooter#working_dir()
  let cmd = printf('%s --cmd "%s" --cmd-dir "%s"',
        \ s:empty_filter_cmd,
        \ a:cmd,
        \ cmd_dir,
        \ )

  call clap#maple#job_start(cmd)
endfunction

function! clap#maple#grep(bare_cmd, query) abort
  let cmd_dir = clap#rooter#working_dir()
  let cmd = printf('%s --grep-cmd "%s" --grep-query "%s" --cmd-dir "%s"',
        \ s:empty_filter_cmd,
        \ a:bare_cmd,
        \ a:query,
        \ cmd_dir,
        \ )
  call clap#maple#job_start(cmd)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
