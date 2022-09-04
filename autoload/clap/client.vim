" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Vim client talking to the Rust backend.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:req_id = get(s:, 'req_id', 0)
let s:handlers = get(s:, 'handlers', {})
let s:session_id = get(s:, 'session_id', 0)

let s:last_recent_file = v:null

function! clap#client#handle(msg) abort
  let decoded = json_decode(a:msg)

  if has_key(decoded, 'deprecated_method')
    call call(decoded.deprecated_method, [decoded])
    return
  endif

  " Handle the request from Rust backend.
  if has_key(decoded, 'method')
    let params = get(decoded, 'params', [])
    try
      let result = clap#api#call(decoded.method, params)
      if has_key(decoded, 'id')
        call clap#job#daemon#send_message(json_encode({ 'id': decoded.id, 'result': result }))
      endif
    catch
      call clap#helper#echo_error(v:exception.', throwpoint:'.v:throwpoint)
      if has_key(decoded, 'id')
        call clap#job#daemon#send_message(json_encode({ 'id': decoded.id, 'error': {'code': -32603, 'message': string(v:exception) }}))
      endif
    endtry
    return
  endif

  if has_key(decoded, 'id') && has_key(s:handlers, decoded.id)
    let Handler = remove(s:handlers, decoded.id)
    call Handler(get(decoded, 'result', v:null), get(decoded, 'error', v:null))
    return
  endif
endfunction

function! s:send_notification(method, params) abort
  call clap#job#daemon#send_message(json_encode({
        \ 'method': a:method,
        \ 'params': a:params,
        \ 'session_id': s:session_id,
        \ }))
endfunction

function! s:send_method_call(method, params) abort
  let s:req_id += 1
  call clap#job#daemon#send_message(json_encode({
        \ 'id': s:req_id,
        \ 'method': a:method,
        \ 'params': a:params,
        \ 'session_id': s:session_id,
        \ }))
endfunction

" Recommended API
" Optional argument: params: v:null, List, Dict
function! clap#client#notify(method, ...) abort
  call s:send_notification(a:method, get(a:000, 0, v:null))
endfunction

" Optional argument: params: v:null, List, Dict
function! clap#client#call(method, callback, ...) abort
  call s:send_method_call(a:method, get(a:000, 0, v:null))
  if a:callback isnot v:null
    let s:handlers[s:req_id] = a:callback
  endif
endfunction

function! clap#client#notify_on_init(method, ...) abort
  call clap#rooter#try_set_cwd()
  let s:session_id += 1
  let params = {
        \   'provider_id': g:clap.provider.id,
        \   'cwd': clap#rooter#working_dir(),
        \   'icon': get(g:clap.provider._(), 'icon', 'Null'),
        \   'debounce': get(g:clap.provider._(), 'debounce', v:true),
        \   'no_cache': has_key(g:clap.context, 'no-cache') ? v:true : v:false,
        \   'source_fpath': expand('#'.g:clap.start.bufnr.':p'),
        \   'input': { 'bufnr': g:clap.input.bufnr, 'winid': g:clap.input.winid },
        \   'start': { 'bufnr': g:clap.start.bufnr, 'winid': g:clap.start.winid },
        \   'display': { 'bufnr': g:clap.display.bufnr, 'winid': g:clap.display.winid },
        \   'preview': { 'bufnr': g:clap.preview.bufnr, 'winid': g:clap.preview.winid },
        \ }
  if a:0 > 0
    call extend(params, a:1)
  endif
  call s:send_notification(a:method, params)
endfunction

function! clap#client#notify_recent_file() abort
  if &buftype ==# 'nofile'
    return
  endif
  let file = expand(expand('<afile>:p'))
  if s:last_recent_file isnot v:null && s:last_recent_file == file
    return
  endif
  call s:send_notification('note_recent_files', [file])
  let s:last_recent_file = file
endfunction

""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""
""  Deprecated and unused in clap repo, but keep them to not break the users using old version.
""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""
function! clap#client#call_preview_file(extra) abort
  call clap#client#call('preview/file', function('clap#impl#on_move#handler'), clap#preview#maple_opts(a:extra))
endfunction

" One optional argument: Dict, extra params
function! clap#client#call_on_move(method, callback, ...) abort
  let curline = g:clap.display.getcurline()
  if empty(curline)
    return
  endif
  let params = {'curline': curline}
  if a:0 > 0
    call extend(params, a:1)
  endif
  call clap#client#call(a:method, a:callback, params)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
