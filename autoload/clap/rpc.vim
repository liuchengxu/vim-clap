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

function! s:on_complete() abort
  " At initial vim8.2, bufwinid(popup_bufnr) seemingly does not work as expected. Ref #223.
  " if bufwinid(g:clap.display.bufnr) == -1
  "
  " Some long-running jobs can be still running, but the window has been canceled by user.
  if g:clap.display.winid == -1
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

        for line in a:data
          if line ==# ''
            continue
          endif
          try
            let json = json_decode(line)
          catch
            echom string(line)
            continue
          endtry
          if has_key(json, 'error')
            call g:clap.display.set_lines([json.error])
          elseif has_key(json, 'data')
            let s:open_file_dict[json.dir] = json.data
            call g:clap.display.set_lines(json.data)
          else
            echom "stdout: ".string(json)
          endif
        endfor
      elseif a:event ==# 'stderr'
        " Ignore the error
      else
        echom "On complete"
        " call s:on_complete()
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
  if has_key(s:open_file_dict, dir)
    let filtered = clap#filter#(g:clap.input.get(), s:open_file_dict[dir])
    call g:clap.display.set_lines(filtered)
    return
  endif
  let msg = json_encode({'method': 'open_file', 'params': {'cwd': dir}, 'id': 1})
  echom "job_id: ".s:job_id.", send: ".string(msg)
  call chansend(s:job_id, msg."\n")
endfunction

function! clap#rpc#bs() abort
  let input = g:clap.input.get()
  if input ==# ''
    let spinner = clap#spinner#get_rpc()
    if spinner[-1:] ==# '/'
      let par = trim(fnamemodify(spinner, ':h:h'))
    else
      let par = trim(fnamemodify(spinner, ':h'))
    endif
    call clap#spinner#set_rpc(par)
    call clap#rpc#send()
  else

    let dir = clap#spinner#get_rpc()
    call g:clap.input.set(input[:-2])
    if has_key(s:open_file_dict, dir)
      let filtered = clap#filter#(g:clap.input.get(), s:open_file_dict[dir])
      call g:clap.display.set_lines(filtered)
      return ''
    endif
  endif
  return ''
endfunction

function! clap#rpc#tab() abort
  let curline = g:clap.display.getcurline()
  call clap#spinner#set_rpc(curline)
  call g:clap.input.set('')
  call clap#rpc#send()
  return ''
endfunction

function! clap#rpc#run() abort
  let cmd = printf('%s rpc',
        \ s:maple_bin,
        \ )
  call clap#spinner#set_rpc(getcwd())
  call g:clap.display.setbufvar('&syntax', 'clap_open_files')
  let s:open_file_dict = {}
  call clap#rpc#job_start(cmd)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
