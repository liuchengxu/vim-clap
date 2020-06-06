" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Vim client for the daemon job.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:req_id = get(s:, 'req_id', 0)

" Note: must use v:true/v:false for json_encode
let s:enable_icon = g:clap_enable_icon ? v:true : v:false

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

  if has_key(decoded, 'lines')
    try
      call g:clap.preview.show(decoded.lines)
    catch
      return
    endtry
    if has_key(decoded, 'fname')
      call g:clap.preview.set_syntax(clap#ext#into_filetype(decoded.fname))
    endif
    call clap#preview#highlight_header()

    if has_key(decoded, 'hi_lnum')
      call g:clap.preview.add_highlight(decoded.hi_lnum+1)
    endif
  endif
endfunction

function! clap#client#send_request_on_move() abort
  let s:req_id += 1
  let curline = g:clap.display.getcurline()
  let msg = json_encode({
      \ 'id': s:req_id,
      \ 'method': 'client.on_move',
      \ 'params': {
      \   'cwd': g:clap.provider.id ==# 'filer' ? clap#provider#filer#current_dir() : clap#rooter#working_dir(),
      \   'curline': curline,
      \   'enable_icon': s:enable_icon,
      \   'provider_id': g:clap.provider.id,
      \   'preview_size': clap#preview#size_of(g:clap.provider.id),
      \ },
      \ })
  call clap#job#daemon#send_message(msg)
endfunction

function! clap#client#send_request_filer(params) abort
  let s:req_id += 1
  call clap#job#daemon#send_message(json_encode({
        \ 'id': s:req_id,
        \ 'method': 'filer',
        \ 'params': a:params
        \ }))
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
