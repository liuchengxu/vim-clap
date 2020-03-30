" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Dynamic update version of maple filter.

let s:job_id = -1

function! s:out_cb(channel, message) abort
  if s:job_id > 0 && clap#job#vim8_job_id_of(a:channel) == s:job_id
    try
      call s:MessageHandler(a:message)
    catch
      call clap#helper#echo_error('[dyn]Failed to handle message:'.a:message.', exception:'.v:exception.', '.v:throwpoint)
    endtry
  endif
endfunction

function! s:err_cb(channel, message) abort
  if s:job_id > 0 && clap#job#vim8_job_id_of(a:channel) == s:job_id
    call clap#helper#echo_error(a:message)
  endif
endfunction

function! s:handle_message(msg) abort
  if !g:clap.display.win_is_valid()
        \ || g:clap.input.get() !=# s:last_query
    return
  endif

  let decoded = json_decode(a:msg)

  if has_key(decoded, 'total')
    call clap#impl#refresh_matches_count(string(decoded.total))
  endif

  if has_key(decoded, 'lines')
    let g:lines = decoded.lines
    call g:clap.display.set_lines(decoded.lines)
  endif

  if has_key(decoded, 'indices')
    call clap#highlight#add_fuzzy_async(decoded.indices)
  endif
endfunction

function! s:job_stop() abort
  if s:job_id > 0
    call clap#job#stop(s:job_id)
    let s:job_id = -1
  endif
endfunction

let s:MessageHandler = function('s:handle_message')

function! clap#filter#async#dyn#start() abort
  call s:job_stop()

  let s:last_query = g:clap.input.get()
  let filter_cmd = printf('--enable-icon --number 100 filter --input %s "%s"', g:__clap_forerunner_tempfile, g:clap.input.get())
  let maple_cmd = clap#maple#run(filter_cmd)
  let job = job_start(clap#job#wrap_cmd(maple_cmd), {
        \ 'err_cb': function('s:err_cb'),
        \ 'out_cb': function('s:out_cb'),
        \ 'noblock': 1,
        \ })
  let s:job_id = clap#job#parse_vim8_job_id(string(job))
endfunction
