" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Vim client talking to the Rust backend.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:req_id = get(s:, 'req_id', 0)
let s:callbacks = get(s:, 'callbacks', {})
let s:session_id = get(s:, 'session_id', 0)

function! clap#client#handle(encoded_response) abort
  let response = json_decode(a:encoded_response)

  " Deprecated
  if has_key(response, 'deprecated_method')
    call call(response.deprecated_method, [response])
    return
  endif

  " Handle the request initiated from Rust backend.
  if has_key(response, 'method')
    try
      " NOTE: This request must not block Vim.
      let result = clap#api#call(response.method, get(response, 'params', []))
      if has_key(response, 'id')
        call clap#job#daemon#send_message(json_encode({ 'id': response.id, 'result': result }))
      endif
    catch
      call clap#helper#echo_error(v:exception.', throwpoint:'.v:throwpoint)
      if has_key(response, 'id')
        call clap#job#daemon#send_message(json_encode({ 'id': response.id, 'error': {'code': -32603, 'message': v:exception }}))
      endif
    endtry
    return
  endif

  " Handle the response of request initiated from Vim.
  if has_key(response, 'id') && has_key(s:callbacks, response.id)
    let Callback = remove(s:callbacks, response.id)
    call Callback(get(response, 'result', v:null), get(response, 'error', v:null))
    return
  endif
endfunction

" params must be Dict.
function! s:notify_provider(method, params) abort
  if clap#job#daemon#is_running()
    if type(a:params) != v:t_dict
      call clap#helper#echo_error('params must be Dict')
      return
    endif
    let params = a:params
    let params['session_id'] = s:session_id
    call clap#job#daemon#send_message(json_encode({
          \ 'method': a:method,
          \ 'params': a:params,
          \ }))
  endif
endfunction

function! s:request_async(method, params) abort
  if clap#job#daemon#is_running()
    let s:req_id += 1
    call clap#job#daemon#send_message(json_encode({
          \ 'id': s:req_id,
          \ 'method': a:method,
          \ 'params': a:params,
          \ }))
  endif
endfunction

" Recommended API
function! clap#client#notify_provider(method) abort
  call s:notify_provider(a:method, {})
endfunction

function! clap#client#notify(method, ...) abort
  if clap#job#daemon#is_running()
    call clap#job#daemon#send_message(json_encode({
          \ 'method': a:method,
          \ 'params': get(a:000, 0, v:null),
          \ }))
  endif
endfunction

" Optional argument: params: v:null, List, Dict
function! clap#client#request_async(method, callback, ...) abort
  call s:request_async(a:method, get(a:000, 0, v:null))
  if a:callback isnot v:null
    let s:callbacks[s:req_id] = a:callback
  endif
endfunction

function! clap#client#request(method, ...) abort
  call s:request_async(a:method, get(a:000, 0, v:null))
endfunction

function! clap#client#notify_on_init(...) abort
  if g:clap.display.winid < 0
    return
  endif
  call clap#rooter#try_set_cwd()
  let s:session_id += 1
  let params = {
        \   'provider_id': g:clap.provider.id,
        \   'input': { 'bufnr': g:clap.input.bufnr, 'winid': g:clap.input.winid },
        \   'start': { 'bufnr': g:clap.start.bufnr, 'winid': g:clap.start.winid },
        \   'display': { 'bufnr': g:clap.display.bufnr, 'winid': g:clap.display.winid },
        \   'cwd': clap#rooter#working_dir(),
        \   'icon': g:clap_enable_icon ? get(g:clap.provider._(), 'icon', 'Null') : 'Null',
        \   'debounce': get(g:clap.provider._(), 'debounce', v:true),
        \   'no_cache': has_key(g:clap.context, 'no-cache') ? v:true : v:false,
        \   'start_buffer_path': expand('#'.g:clap.start.bufnr.':p'),
        \ }
  if a:0 > 0
    call extend(params, a:1)
  endif
  call s:notify_provider('new_session', params)
endfunction

function! clap#client#notify_recent_file() abort
  if &buftype ==# 'nofile'
    return
  endif
  let file = expand(expand('<afile>:p'))
  call clap#client#notify('note_recent_files', [file])
endfunction

""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""
""  Deprecated and unused in clap repo, but keep them to not break the users using old version.
""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""
function! clap#client#call_preview_file(extra) abort
  call clap#client#request_async('preview/file', function('clap#impl#on_move#handler'), clap#preview#maple_opts(a:extra))
endfunction

" One optional argument: Dict, extra params
"
" callback is unused as it's already handled by Rust backend.
function! clap#client#call_on_move(method, _callback, ...) abort
  let curline = g:clap.display.getcurline()
  if empty(curline)
    return
  endif
  let params = {'curline': curline}
  if a:0 > 0
    call extend(params, a:1)
  endif
  call s:notify_provider(a:method, params)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
