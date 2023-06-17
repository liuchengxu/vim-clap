" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Abstracted RPC interfaces.

let s:save_cpo = &cpoptions
set cpoptions&vim

function! clap#rpc#request(id, method, params) abort
  call clap#job#daemon#send_raw(json_encode({
        \ 'id': a:id,
        \ 'method': a:method,
        \ 'params': a:params,
        \ }))
endfunction

function! clap#rpc#notify(method, params) abort
  call clap#job#daemon#send_raw(json_encode({
        \ 'method': a:method,
        \ 'params': a:params,
        \ }))
endfunction

function! clap#rpc#send_ok_response(id, result) abort
  call clap#job#daemon#send_raw(json_encode({ 'id': a:id, 'result': a:result }))
endfunction

function! clap#rpc#send_error_response(id, error_msg) abort
  call clap#job#daemon#send_raw(json_encode({ 'id': a:id, 'error': {'code': -32603, 'message': a:error_msg }}))
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
