" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Vim client for the daemon job.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:req_id = get(s:, 'req_id', 0)
let s:session_id = get(s:, 'session_id', 0)

let s:handlers = get(s:, 'handlers', {})

function! clap#client#send_request_initialize_global_env() abort
  let s:req_id += 1
  call clap#job#daemon#send_message(json_encode({
        \ 'id': s:req_id,
        \ 'session_id': s:session_id,
        \ 'method': 'initialize_global_env',
        \ 'params': {
        \   'is_nvim': has('nvim') ? v:true : v:false,
        \   'enable_icon': g:clap_enable_icon ? v:true : v:false,
        \   'clap_preview_size': g:clap_preview_size,
        \ }
        \ }))
endfunction

function! s:generic_handle_on_move_result(result) abort
  if has_key(a:result, 'lines')
    try
      call g:clap.preview.show(a:result.lines)
    catch
      return
    endtry
    if has_key(a:result, 'fname')
      call g:clap.preview.set_syntax(clap#ext#into_filetype(a:result.fname))
    endif
    call clap#preview#highlight_header()

    if has_key(a:result, 'hi_lnum')
      call g:clap.preview.add_highlight(a:result.hi_lnum+1)
    endif
  endif
endfunction

function! clap#client#handle(msg) abort
  let decoded = json_decode(a:msg)

  " Only process the latest request, drop the outdated responses.
  if s:req_id != decoded.id
    return
  endif

  if has_key(decoded, 'error')
    " TODO: show the error message in preview window when it's on_move
    call clap#helper#echo_error('[client_handle] '.string(decoded.error))
    return
  endif

  if has_key(s:handlers, decoded.id)
    call s:handlers[decoded.id](decoded.result)
    call remove(s:handlers, decoded.id)
    return
  endif
endfunction

function! clap#client#send_request_on_init(...) abort
  let s:req_id += 1
  let s:session_id += 1
  let msg = {
        \ 'id': s:req_id,
        \ 'session_id': s:session_id,
        \ 'method': 'on_init',
        \ 'params': {
        \   'cwd': clap#rooter#working_dir(),
        \   'winwidth': winwidth(g:clap.provider.id),
        \   'provider_id': g:clap.provider.id,
        \   'source_fpath': expand('#'.g:clap.start.bufnr.':p'),
        \ }
        \ }
  if a:0 > 0
    call extend(msg.params, a:1)
  endif
  call clap#job#daemon#send_message(json_encode(msg))
endfunction

function! clap#client#send_request_on_init_with_callback(callback, ...) abort
  call call(function('clap#client#send_request_on_init'), a:000)
  let s:handlers[s:req_id] = a:callback
endfunction

" Optional argument: Dict, extra params
function! clap#client#send_request_on_move(...) abort
  call call(function('clap#client#send_request_on_move_with_callback'), [function('s:generic_handle_on_move_result')] + a:000)
endfunction

function! clap#client#send_request_on_move_with_callback(callback, ...) abort
  let curline = g:clap.display.getcurline()
  if empty(curline)
    return
  endif
  let s:req_id += 1
  let s:handlers[s:req_id] = a:callback
  let msg = {
      \ 'id': s:req_id,
      \ 'session_id': s:session_id,
      \ 'method': 'on_move',
      \ 'params': {
      \   'curline': curline,
      \ }}
  if a:0 > 0
    call extend(msg.params, a:1)
  endif
  call clap#job#daemon#send_message(json_encode(msg))
endfunction

function! clap#client#send_request_exit() abort
  let s:req_id += 1
  call clap#job#daemon#send_message(json_encode({
        \ 'id': s:req_id,
        \ 'session_id': s:session_id,
        \ 'method': 'exit',
        \ 'params': {}
        \ }))
endfunction

function! clap#client#send_request_filer(callback, params) abort
  let s:req_id += 1
  let s:handlers[s:req_id] = a:callback
  call clap#job#daemon#send_message(json_encode({
        \ 'id': s:req_id,
        \ 'session_id': s:session_id,
        \ 'method': 'filer',
        \ 'params': a:params
        \ }))
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
