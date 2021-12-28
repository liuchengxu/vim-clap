" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Vim client for the daemon job.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:req_id = get(s:, 'req_id', 0)
let s:session_id = get(s:, 'session_id', 0)
let s:handlers = get(s:, 'handlers', {})

let s:last_recent_file = v:null

function! s:set_total_size(msg) abort
  let g:clap.display.initial_size = a:msg.total
  if g:clap.provider.id ==# 'blines'
    call clap#provider#blines#initialize()
  endif
endfunction

function! s:init_display(msg) abort
  if empty(g:clap.input.get())
    if g:clap.provider.id !=# 'blines'
      call g:clap.display.set_lines_lazy(a:msg.lines)
      call g:clap#display_win.shrink_if_undersize()
    endif
  endif

  call clap#indicator#update_matches_on_forerunner_done()
  call clap#sign#ensure_exists()

  let g:__clap_current_forerunner_status = g:clap_forerunner_status_sign.done
  call clap#spinner#refresh()
  call clap#preview#async_open_with_delay()
endfunction

function! s:process_filter_message(msg) abort
  if g:clap.display.win_is_valid()
    if !has_key(a:msg, 'query') || a:msg.query ==# g:clap.input.get()
      call clap#state#process_filter_message(a:msg)
    endif
  endif
endfunction

function! clap#client#handle(msg) abort
  let decoded = json_decode(a:msg)

  if has_key(decoded, 'method')
    call call(decoded.method, [decoded])
    return
  endif

  if has_key(decoded, 'force_execute') && has_key(s:handlers, decoded.id)
    let Handler = remove(s:handlers, decoded.id)
    call Handler(get(decoded, 'result', v:null), get(decoded, 'error', v:null))
    return
  endif

  " Only process the latest request, drop the outdated responses.
  if s:req_id != decoded.id
    return
  endif

  if has_key(s:handlers, decoded.id)
    let Handler = remove(s:handlers, decoded.id)
    call Handler(get(decoded, 'result', v:null), get(decoded, 'error', v:null))
    return
  endif
endfunction

function! s:base_params() abort
  let params = {
        \   'cwd': clap#rooter#working_dir(),
        \   'enable_icon': g:clap_enable_icon ? v:true : v:false,
        \   'provider_id': g:clap.provider.id,
        \   'no_cache': has_key(g:clap.context, 'no-cache') ? v:true : v:false,
        \   'source_fpath': expand('#'.g:clap.start.bufnr.':p'),
        \   'display_winwidth': winwidth(g:clap.display.winid),
        \ }
  if has_key(g:clap.preview, 'winid')
        \ && clap#api#floating_win_is_valid(g:clap.preview.winid)
    let params['preview_winheight'] = winheight(g:clap.preview.winid)
  endif
  if g:clap.provider.id ==# 'help_tags'
    let params['runtimepath'] = &runtimepath
  endif
  return params
endfunction

function! clap#client#notify_on_init(method, ...) abort
  let s:session_id += 1
  let params = s:base_params()
  if a:0 > 0
    call extend(params, a:1)
  endif
  call s:send_notification(a:method, params)
endfunction

function! clap#client#call_on_init(method, callback, ...) abort
  let s:session_id += 1
  let params = s:base_params()
  if a:0 > 0
    call extend(params, a:1)
  endif
  call s:send_method_call(a:method, params)
  if a:callback isnot v:null
    let s:handlers[s:req_id] = a:callback
  endif
endfunction

function! clap#client#init_params(extra) abort
  let opts = {
        \ 'provider_id': g:clap.provider.id,
        \ 'query': has_key(g:clap.context, 'query') ? g:clap.context.query : g:clap.input.get(),
        \ 'source_fpath': expand('#'.g:clap.start.bufnr.':p'),
        \ 'cwd': clap#rooter#working_dir(),
        \ }
  return type(a:extra) == v:t_dict ? extend(opts, a:extra) : opts
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

function! clap#client#notify_recent_file() abort
  if &buftype ==# 'nofile'
    return
  endif
  let file = expand(expand('<afile>:p'))
  if s:last_recent_file isnot v:null && s:last_recent_file == file
    return
  endif
  call s:send_notification('note_recent_files', {'file': file})
  let s:last_recent_file = file
endfunction

function! clap#client#notify(method, params) abort
  call s:send_notification(a:method, a:params)
endfunction

function! clap#client#call(method, callback, params) abort
  call s:send_method_call(a:method, a:params)
  if a:callback isnot v:null
    let s:handlers[s:req_id] = a:callback
  endif
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

function! clap#client#call_with_lnum(method, callback, ...) abort
  let params = {'lnum': g:__clap_display_curlnum}
  if g:clap.provider.id ==# 'grep'
    let params['enable_icon'] = g:clap_provider_grep_enable_icon ? v:true : v:false
  endif
  if a:0 > 0
    call extend(params, a:1)
  endif
  call clap#client#call(a:method, a:callback, params)
endfunction

function! clap#client#call_preview_file(extra) abort
  call clap#client#call('preview/file', function('clap#impl#on_move#handler'), clap#preview#maple_opts(a:extra))
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
