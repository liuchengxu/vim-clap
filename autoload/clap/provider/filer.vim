let s:filer = {}

let s:open_file_dict = {}

function! s:handle_round_message(message) abort
  try
    let decoded = json_decode(a:message)
  catch
    echom 'JSON decode in: '.v:exception.', ----'.string(a:message)
    return
  endtry

  if has_key(decoded, 'error')
    call g:clap.display.set_lines([decoded.error])

  elseif has_key(decoded, 'data')
    let s:open_file_dict[decoded.dir] = decoded.data
    call g:clap.display.set_lines(decoded.data)
    call clap#sign#reset_to_first_line()
    call clap#impl#refresh_matches_count(string(decoded.total))
    call g:clap#display_win.compact_if_undersize()

  else
    echom 'stdout: '.string(decoded)
  endif
endfunction

let s:round_message = ''
let s:content_length = 0
function! clap#provider#filer#handle_stdout(lines) abort
  while !empty(a:lines)
    let line = remove(a:lines, 0)

    if line ==# ''
      continue
    elseif s:content_length == 0
      if line =~# '^Content-length:'
        let s:content_length = str2nr(matchstr(line, '\d\+$'))
      else
        echom 'Warning: '.line
      endif
      continue
    endif

    if s:content_length < strlen(l:line)
      let s:round_message .= strpart(line, 0, s:content_length)
      call insert(a:lines, strpart(line, s:content_length))
      let s:content_length = 0
    else
      let s:round_message .= line
      let s:content_length -= strlen(l:line)
    endif

    " The message for this round is still incomplete, contintue to read more.
    if s:content_length > 0
      continue
    endif

    try
      call s:handle_round_message(trim(s:round_message))
    catch
      echom 'ERROR in handle round message'
    finally
      let s:round_message = ''
    endtry

  endwhile
endfunction

function! clap#provider#filer#bs() abort
  call g:clap.display.matchdelete()
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
  call g:clap.display.matchdelete()
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
    return ''
  endif

  call g:clap#display_win.compact_if_undersize()
  call clap#rpc#send()
  return ''
endfunction

function! clap#provider#filer#on_typed() abort
  let curdir = clap#spinner#get_rpc()
  let query = g:clap.input.get()
  call g:clap.display.matchdelete()
  if has_key(s:open_file_dict, curdir)
    let l:lines = call(function('clap#filter#'), [query, s:open_file_dict[curdir]])

    if empty(l:lines)
      let l:lines = [g:clap_no_matches_msg]
      let g:__clap_has_no_matches = v:true
      call g:clap.display.set_lines_lazy(lines)
      " In clap#impl#refresh_matches_count() we reset the sign to the first line,
      " But the signs are seemingly removed when setting the lines, so we should
      " postpone the sign update.
      call clap#impl#refresh_matches_count('0')
      call g:clap.preview.hide()
    else
      call g:clap.display.set_lines_lazy(lines)
      call clap#impl#refresh_matches_count(string(len(l:lines)))
    endif

    call g:clap#display_win.compact_if_undersize()
    call clap#spinner#set_idle()

    if exists('g:__clap_fuzzy_matched_indices')
      call clap#highlight#add_fuzzy_sync()
    endif

    return
  endif
endfunction

let g:clap#provider#filer# = s:filer
