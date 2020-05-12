" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Dispatch the job via maple.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:job_id = -1
let s:job_timer = -1

let s:maple_delay = get(g:, 'clap_maple_delay', 100)

let s:bin_suffix = has('win32') ? '.exe' : ''

let s:maple_bin_localbuilt = fnamemodify(g:clap#autoload_dir, ':h').'/target/release/maple'.s:bin_suffix
let s:maple_bin_prebuilt = fnamemodify(g:clap#autoload_dir, ':h').'/bin/maple'.s:bin_suffix

" Check the local built.
if executable(s:maple_bin_localbuilt)
  let s:maple_bin = s:maple_bin_localbuilt
" Check the prebuilt binary.
elseif executable(s:maple_bin_prebuilt)
  let s:maple_bin = s:maple_bin_prebuilt
elseif executable('maple')
  let s:maple_bin = 'maple'
else
  let s:maple_bin = v:null
endif

function! clap#maple#binary() abort
  return s:maple_bin
endfunction

function! clap#maple#is_available() abort
  return s:maple_bin isnot v:null
endfunction

function! s:on_complete() abort
  " At initial vim8.2, bufwinid(popup_bufnr) seemingly does not work as expected. Ref #223.
  " if bufwinid(g:clap.display.bufnr) == -1
  "
  " Some long-running jobs can be still running, but the window has been canceled by user.
  if g:clap.display.winid == -1
    return
  endif

  call clap#spinner#set_idle()

  " Skip the job processing if use already clears the input at the moment.
  if empty(g:clap.input.get())
    return
  endif

  if empty(s:chunks)
    return
  endif

  try
    let decoded = json_decode(s:chunks[0])
  catch
    echoerr '[maple]decoded on_complete:'.string(s:chunks)
    return
  endtry
  if has_key(decoded, 'error')
    call g:clap.display.set_lines([
          \ 'The external job runs into some issue:',
          \ 'jobid: '.s:job_id,
          \ 'executable: '.split(s:cmd)[0],
          \ 'args: '.join(split(s:cmd)[1:], ' '),
          \ 'error:',
          \ ] + split(decoded.error, "\n"))
    call clap#indicator#set_matches_number(0)
    call clap#sign#disable_cursorline()
    return
  endif

  if decoded.total == 0
    call g:clap.display.set_lines([g:clap_no_matches_msg])
    call clap#indicator#set_matches_number(0)
    call clap#sign#disable_cursorline()
    call g:clap#display_win.shrink_if_undersize()
    call g:clap.preview.hide()
    return
  endif

  call clap#state#refresh_matches_count(decoded.total)

  if s:has_converter
    call g:clap.display.set_lines(map(decoded.lines, 's:Converter(v:val)'))
  else
    call g:clap.display.set_lines(decoded.lines)
  endif

  if has_key(decoded, 'indices')
    call clap#highlight#add_fuzzy_async(decoded.indices)
  endif

  if has_key(decoded, 'truncated_map')
    let g:__clap_lines_truncated_map = decoded.truncated_map
  endif

  call clap#sign#reset_to_first_line()
  call g:clap#display_win.shrink_if_undersize()
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
      try
        let s:chunks = split(ch_readraw(a:channel), "\n")
        call s:on_complete()
      catch
        call clap#helper#echo_error(v:exception)
        call clap#spinner#set_idle()
      endtry
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

  let s:cmd = a:cmd
  let s:job_timer = timer_start(s:maple_delay, function('s:apply_start'))
  return
endfunction

let s:can_enable_icon = ['files', 'git_files']

function! clap#maple#forerunner_exec_command(cmd) abort
  " No global --number option.
  if g:clap_enable_icon
        \ && index(s:can_enable_icon, g:clap.provider.id) > -1
    let global_opt = ['--icon-painter=File']
  else
    let global_opt = []
  endif

  if has_key(g:clap.context, 'no-cache')
    call add(global_opt, '--no-cache')
  endif

  let subcommand = ['exec', a:cmd, '--cmd-dir', clap#rooter#working_dir(), '--output-threshold', clap#filter#capacity()]

  return [s:maple_bin] + global_opt + subcommand
endfunction

" Returns the filtered results after the input stream is complete.
function! clap#maple#sync_filter_command(query) abort
  let global_opt = ['--number', g:clap.display.preload_capacity, '--winwidth', winwidth(g:clap.display.winid)]

  if g:clap.provider.id ==# 'files' && g:clap_enable_icon
    call add(global_opt, '--icon-painter=File')
  endif

  return [s:maple_bin] + global_opt + ['filter', a:query, '--sync']
endfunction

function! clap#maple#tags_forerunner_command() abort
  let global_opt = has_key(g:clap.context, 'no-cache') ? ['--no-cache'] : []
  return [s:maple_bin] + global_opt + ['tags', '', clap#rooter#working_dir(), '--forerunner']
endfunction

function! clap#maple#ripgrep_forerunner_command() abort
  " TODO: add max_output
  let global_opt = g:clap_enable_icon ? ['--icon-painter=Grep'] : []

  if has_key(g:clap.context, 'no-cache')
    call add(global_opt, '--no-cache')
  endif

  return [s:maple_bin] + global_opt + ['ripgrep-forerunner', '--cmd-dir', clap#rooter#working_dir(), '--output-threshold', clap#filter#capacity()]
endfunction

function! clap#maple#blines_command() abort
  let blines_subcmd = ['--number', g:clap.display.preload_capacity, '--winwidth', winwidth(g:clap.display.winid), 'blines', g:clap.input.get(), expand('#'.g:clap.start.bufnr.':p')]
  return [s:maple_bin] + blines_subcmd
endfunction

function! clap#maple#run_exec(cmd) abort
  let global_opt = ['--number', g:clap.display.preload_capacity]
  if g:clap.provider.id ==# 'files' && g:clap_enable_icon
    call add(global_opt, '--icon-painter=File')
  endif
  let subcommand = ['exec', a:cmd, '--cmd-dir', clap#rooter#working_dir()]
  call clap#maple#job_start([s:maple_bin] + global_opt + subcommand)
endfunction

function! clap#maple#run_sync_grep(cmd, query, enable_icon, glob) abort
  let global_opt = ['--number', g:clap.display.preload_capacity]

  if a:enable_icon
    call add(global_opt, '--icon-painter=Grep')
  endif

  let subcommand = ['grep', a:query, '--sync', '--grep-cmd', a:cmd, '--cmd-dir', clap#rooter#working_dir()]

  if a:glob isnot v:null
    let subcommand += ['--glob', a:glob]
  endif

  call clap#maple#job_start([s:maple_bin] + global_opt + subcommand)
endfunction

function! clap#maple#build_cmd(...) abort
  return [s:maple_bin] + a:000
endfunction

function! clap#maple#build_cmd_list(cmd_list) abort
  return insert(a:cmd_list, s:maple_bin)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
