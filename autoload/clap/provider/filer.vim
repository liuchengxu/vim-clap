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
    call g:clap#display_win.shrink_if_undersize()

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
  call clap#highlight#clear()

  let input = g:clap.input.get()

  if input ==# ''
    if s:current_dir[-1:] ==# '/'
      let parent_dir = trim(fnamemodify(s:current_dir, ':h:h'))
    else
      let parent_dir = trim(fnamemodify(s:current_dir, ':h'))
    endif
    call clap#spinner#set(pathshorten(parent_dir))

    let s:current_dir = parent_dir

    if s:try_filter_is_ok()
      return ''
    endif

    let msg = json_encode({'method': 'open_file', 'params': {'cwd': s:current_dir}, 'id': 1})
    call clap#rpc#send_message(msg)
  else

    call g:clap.input.set(input[:-2])
    if has_key(s:open_file_dict, s:current_dir)
      let filtered = clap#filter#(g:clap.input.get(), s:open_file_dict[s:current_dir])
      call g:clap.display.set_lines(filtered)
      call g:clap#display_win.shrink_if_undersize()
      return ''
    endif
  endif
  return ''
endfunction

function! clap#provider#filer#run() abort
  let s:open_file_dict = {}
  let s:current_dir = getcwd()
  call clap#spinner#set(pathshorten(s:current_dir))
  call g:clap.display.setbufvar('&syntax', 'clap_open_files')
  let cmd = clap#maple#run('rpc')
  call clap#rpc#job_start(cmd)
  let msg = json_encode({'method': 'open_file', 'params': {'cwd': s:current_dir}, 'id': 1})
  call clap#rpc#send_message(msg)
endfunction

function! s:try_filter_is_ok() abort
  if has_key(s:open_file_dict, s:current_dir)
    let query = g:clap.input.get()
    let l:lines = call(function('clap#filter#'), [query, s:open_file_dict[s:current_dir]])

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

    call g:clap#display_win.shrink_if_undersize()
    call clap#spinner#set_idle()

    if exists('g:__clap_fuzzy_matched_indices')
      call clap#highlight#add_fuzzy_sync()
    endif

    return v:true
  endif
  return v:false
endfunction

function! clap#provider#filer#tab() abort
  call clap#highlight#clear()

  let curline = g:clap.display.getcurline()

  if s:current_dir[-1:] ==# '/'
    let cur_entry = s:current_dir.curline
  else
    let cur_entry = s:current_dir.'/'.curline
  endif
  if filereadable(cur_entry)
    return ''
  endif
  let s:current_dir = cur_entry

  call clap#spinner#set(pathshorten(s:current_dir))
  call g:clap.input.set('')

  if s:try_filter_is_ok()
    return ''
  endif

  let msg = json_encode({'method': 'open_file', 'params': {'cwd': s:current_dir}, 'id': 1})
  call clap#rpc#send_message(msg)

  return ''
endfunction

function! clap#provider#filer#on_typed() abort
  call clap#highlight#clear()

  call s:try_filter_is_ok()
  return ''
endfunction

let g:clap#provider#filer# = s:filer
