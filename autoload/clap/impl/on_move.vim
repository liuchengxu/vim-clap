" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: CursorMoved handler

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:on_move_timer = -1
let s:req_id = get(s:, 'req_id', 0)
let s:on_move_delay = get(g:, 'clap_on_move_delay', 300)
" Note: must use v:true/v:false for json_encode
let s:enable_icon = g:clap_enable_icon ? v:true : v:false
let s:handle = {'filer': function('clap#provider#filer#daemon_handle')}

function! s:into_filename(line) abort
  if g:clap_enable_icon
    return a:line[4:]
  else
    return a:line
  endif
endfunction

function! s:filer_handle(decoded) abort
  if has_key(a:decoded, 'type') && a:decoded.type ==# 'preview'
    if empty(a:decoded.lines)
      call g:clap.preview.show(['Empty entries'])
    else
      call g:clap.preview.show(a:decoded.lines)
      if has_key(a:decoded, 'is_dir')
        call g:clap.preview.set_syntax('clap_filer')
        call clap#preview#clear_header_highlight()
      else
        if has_key(a:decoded, 'fname')
          call g:clap.preview.set_syntax(clap#ext#into_filetype(a:decoded.fname))
        endif
        call clap#preview#highlight_header()
      endif
    endif
  else
    call clap#provider#filer#daemon_handle(a:decoded)
  endif
endfunction

function! clap#impl#on_move#daemon_handle(msg) abort
  let decoded = json_decode(a:msg)

  " Only process the latest request, drop the outdated responses.
  if s:req_id != decoded.id
    return
  endif

  if decoded.provider_id ==# 'filer'
    call s:filer_handle(decoded)
    return
  endif

  if has_key(decoded, 'error')
    echoerr decoded.error
    return
  endif

  if has_key(decoded, 'lines')
    call g:clap.preview.show(decoded.lines)
    if has_key(decoded, 'fname')
      call g:clap.preview.set_syntax(clap#ext#into_filetype(decoded.fname))
    endif
    call clap#preview#highlight_header()

    if has_key(decoded, 'hi_lnum')
      call g:clap.preview.add_highlight(decoded.hi_lnum+1)
    endif
  endif
endfunction

function! s:send_request() abort
  let s:req_id += 1
  let curline = s:into_filename(g:clap.display.getcurline())
  let msg = json_encode({
      \ 'id': s:req_id,
      \ 'method': 'client.on_move',
      \ 'params': {
      \   'cwd': g:clap.provider.id ==# 'filer' ? clap#provider#filer#current_dir() : clap#rooter#working_dir(),
      \   'curline': curline,
      \   'enable_icon': s:enable_icon,
      \   'provider_id': g:clap.provider.id,
      \   'preview_size': 5,
      \ },
      \ })
  call clap#job#daemon#send_message(msg)
endfunction

function! clap#impl#on_move#send_params(params) abort
  let s:req_id += 1
  let params = a:params
  let params.id = s:req_id
  let msg = json_encode(params)
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
    if index(['filer', 'files', 'grep', 'grep2'], g:clap.provider.id) > -1
      return s:send_request()
    endif
    call s:sync_run_with_delay()
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
