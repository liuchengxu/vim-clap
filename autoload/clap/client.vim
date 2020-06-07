" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Vim client for the daemon job.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:req_id = get(s:, 'req_id', 0)
let s:session_id = get(s:, 'session_id', 0)

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

function! s:handle_on_move_result(result) abort
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

  if decoded.provider_id ==# 'filer'
    call clap#impl#on_move#filer_handle(decoded)
    return
  endif

  call s:handle_on_move_result(decoded.result)
endfunction

let s:should_send_source_fpath = ['tags', 'blines']

function! clap#client#send_request_on_move() abort
  let s:req_id += 1
  let curline = g:clap.display.getcurline()
  let msg = {
      \ 'id': s:req_id,
      \ 'session_id': s:session_id,
      \ 'method': 'on_move',
      \ 'params': {
      \   'cwd': g:clap.provider.id ==# 'filer' ? clap#provider#filer#current_dir() : clap#rooter#working_dir(),
      \   'curline': curline,
      \   'provider_id': g:clap.provider.id,
      \ }}
  if index(s:should_send_source_fpath, g:clap.provider.id) > -1
    let msg.params.source_fpath = expand('#'.g:clap.start.bufnr.':p')
  endif
  call clap#job#daemon#send_message(json_encode(msg))
endfunction

function! clap#client#send_request_filer(params) abort
  let s:req_id += 1
  call clap#job#daemon#send_message(json_encode({
        \ 'id': s:req_id,
        \ 'session_id': s:session_id,
        \ 'method': 'filer',
        \ 'params': a:params
        \ }))
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
