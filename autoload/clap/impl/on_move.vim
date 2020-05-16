let s:on_move_timer = -1
let s:req_id = get(s:, 'req_id', 0)
let s:on_move_delay = get(g:, 'clap_on_move_delay', 300)

function! s:into_filename(line) abort
  if g:clap_enable_icon
    return a:line[4:]
  else
    return a:line
  endif
endfunction

function! clap#impl#on_move#daemon_handle(msg) abort
  let decoded = json_decode(a:msg)

  if s:req_id == decoded.id
    if has_key(decoded, 'lines')
      let lines = decoded.lines
      call g:clap.preview.show(lines)
      " call g:clap.preview.set_syntax(clap#ext#into_filetype(decoded.fname))
      call clap#preview#highlight_header()

      if has_key(decoded, 'hi_lnum')
        call g:clap.preview.add_highlight(decoded.hi_lnum+1)
      endif
    elseif has_key(decoded, 'error')
      echoerr decoded.error
    endif
  endif
endfunction

function! s:send_request() abort
  " if !clap#job#daemon#is_running()
    " call clap#job#daemon#start(function('s:daemon_handle'))
  " endif
  let s:req_id += 1
  let curline = s:into_filename(g:clap.display.getcurline())
  let msg = json_encode({
      \ 'id': s:req_id,
      \ 'method': 'client.on_move',
      \ 'params': {
      \   'curline': curline,
      \   'cwd': clap#rooter#working_dir(),
      \   'enable_icon': g:clap_enable_icon ? v:true : v:false,
      \   'provider_id': g:clap.provider.id
      \ },
      \ })
  call clap#job#daemon#send_message(msg)
endfunction

function! s:sync_run_with_delay() abort
  if s:on_move_timer != -1
    call timer_stop(s:on_move_timer)
  endif
  let s:on_move_timer = timer_start(s:on_move_delay, { -> g:clap.provider._().on_move() })
endfunction

function! clap#impl#on_move#invoke() abort
  if get(g:, '__clap_has_no_matches', v:false)
    return
  endif
  if has_key(g:clap.provider._(), 'on_move')
    if g:clap.provider.id ==# 'files' || g:clap.provider.id ==# 'grep'
      return s:send_request()
    endif
    call s:sync_run_with_delay()
  endif
endfunction
