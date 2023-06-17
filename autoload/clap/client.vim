" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Vim client talking to the Rust backend.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:req_id = get(s:, 'req_id', 0)
let s:handlers = get(s:, 'handlers', {})
let s:session_id = get(s:, 'session_id', 0)

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
        call clap#rpc#send_ok_response(decoded.id, result)
      endif
    catch
      call clap#helper#echo_error(v:exception.', throwpoint:'.v:throwpoint)
      if has_key(decoded, 'id')
        call clap#rpc#send_error_response(decoded.id, v:exception)
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

function! s:notify_provider(method, params) abort
  if clap#job#daemon#is_running()
    let params = a:params
    let params['session_id'] = s:session_id
    call clap#rpc#notify(a:method, params)
  endif
endfunction

function! s:request_async(method, params) abort
  if clap#job#daemon#is_running()
    let s:req_id += 1
    call clap#rpc#request(s:req_id, a:method, a:params)
  endif
endfunction

" Recommended API
" Optional argument: params: v:null, List, Dict
function! clap#client#notify_provider(method, ...) abort
  call s:notify_provider(a:method, get(a:000, 0, {}))
endfunction

function! clap#client#notify(method, ...) abort
  if clap#job#daemon#is_running()
    call clap#rpc#notify(a:method, get(a:000, 0, v:null))
  endif
endfunction

" Optional argument: params: v:null, List, Dict
function! clap#client#request_async(method, callback, ...) abort
  call s:request_async(a:method, get(a:000, 0, v:null))
  if a:callback isnot v:null
    let s:handlers[s:req_id] = a:callback
  endif
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

""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""
""  Deprecated and unused in clap repo, but keep them to not break the users using old version.
""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""
function! clap#client#call_preview_file(extra) abort
  call clap#client#request_async('preview/file', function('clap#impl#on_move#handler'), clap#preview#maple_opts(a:extra))
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
  " on_move callback is unused as it's already handled by Rust backend.
  call s:notify_provider(a:method, params)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
