let s:filer = {}

function! clap#provider#filer#handle_stdout(line) abort
  let line = a:line
  if line ==# ''
    return
  endif
  try
    let json = json_decode(line)
  catch
    echom string(line)
    return
  endtry
  if has_key(json, 'error')
    call g:clap.display.set_lines([json.error])
  elseif has_key(json, 'data')
    let s:open_file_dict[json.dir] = json.data
    call g:clap.display.set_lines(json.data)
  else
    echom "stdout: ".string(json)
  endif
endfunction

function! clap#provider#filer#bs() abort
  let input = g:clap.input.get()
  if input ==# ''
    let spinner = clap#spinner#get_rpc()
    if spinner[-1:] ==# '/'
      let par = trim(fnamemodify(spinner, ':h:h'))
    else
      let par = trim(fnamemodify(spinner, ':h'))
    endif
    call clap#spinner#set_rpc(par)

    let dir = clap#spinner#get_rpc()
    if has_key(s:open_file_dict, dir)
      let filtered = clap#filter#(g:clap.input.get(), s:open_file_dict[dir])
      call g:clap.display.set_lines(filtered)
      return ''
    endif

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

function! clap#provider#filer#run() abort
  let cmd = clap#maple#run('rpc')
  call clap#spinner#set_rpc(getcwd())
  call g:clap.display.setbufvar('&syntax', 'clap_open_files')
  let s:open_file_dict = {}
  call clap#rpc#job_start(cmd)
endfunction

function! clap#provider#filer#tab() abort
  let curline = g:clap.display.getcurline()
  let curdir = clap#spinner#get_rpc()
  if curdir[-1:] ==# '/'
    let cur_entry = curdir.curline
  else
    let cur_entry = curdir.'/'.curline
  endif
  if filereadable(cur_entry)
    return ''
  endif
  call clap#spinner#set_rpc(cur_entry)
  call g:clap.input.set('')

  let dir = clap#spinner#get_rpc()
  if has_key(s:open_file_dict, dir)
    let filtered = clap#filter#(g:clap.input.get(), s:open_file_dict[dir])
    call g:clap.display.set_lines(filtered)
    return
  endif

  call clap#rpc#send()
  return ''
endfunction

let g:clap#provider#filer# = s:filer
