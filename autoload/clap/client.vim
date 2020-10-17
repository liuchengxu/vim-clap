" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Vim client for the daemon job.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:req_id = get(s:, 'req_id', 0)
let s:session_id = get(s:, 'session_id', 0)
let s:handlers = get(s:, 'handlers', {})

function! clap#client#handle(msg) abort
  let decoded = json_decode(a:msg)

  " Only process the latest request, drop the outdated responses.
  if s:req_id != decoded.id
    return
  endif

  if has_key(s:handlers, decoded.id)
    call s:handlers[decoded.id](get(decoded, 'result', v:null), get(decoded, 'error', v:null))
    call remove(s:handlers, decoded.id)
    return
  endif
endfunction

function! clap#client#notify_on_init(method, ...) abort
  let s:session_id += 1
  let params = {
        \   'cwd': clap#rooter#working_dir(),
        \   'winwidth': winwidth(g:clap.display.winid),
        \   'provider_id': g:clap.provider.id,
        \   'source_fpath': expand('#'.g:clap.start.bufnr.':p'),
        \ }
  if a:0 > 0
    call extend(params, a:1)
  endif
  call clap#client#notify(a:method, params)
endfunction

function! clap#client#call_on_init(method, callback, ...) abort
  call call(function('clap#client#notify_on_init'), [a:method] + a:000)
  let s:handlers[s:req_id] = a:callback
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

function! clap#client#notify(method, params) abort
  let s:req_id += 1
  call clap#job#daemon#send_message(json_encode({
        \ 'id': s:req_id,
        \ 'session_id': s:session_id,
        \ 'method': a:method,
        \ 'params': a:params,
        \ }))
endfunction

function! clap#client#call(method, callback, params) abort
  call clap#client#notify(a:method, a:params)
  let s:handlers[s:req_id] = a:callback
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
