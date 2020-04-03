" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Change state of current filtering, e.g., matches count.

let s:save_cpo = &cpoptions
set cpoptions&vim

" NOTE: some local variable without explicit l:, e.g., count,
" may run into some erratic read-only error.
function! clap#state#refresh_matches_count(cnt_str) abort
  let l:matches_cnt = a:cnt_str
  let s:current_matches = a:cnt_str

  if get(g:clap.display, 'initial_size', -1) > 0
    let l:matches_cnt .= '/'.g:clap.display.initial_size
  endif

  call clap#indicator#set_matches('['.l:matches_cnt.']')
  call clap#sign#reset_to_first_line()
endfunction

function! clap#state#refresh_matches_count_on_forerunner_done() abort
  if exists('s:current_matches')
    call clap#indicator#set_matches(printf('[%s/%s]', s:current_matches, g:clap.display.initial_size))
  endif
endfunction

function! clap#state#handle_message(msg) abort
  let decoded = json_decode(a:msg)

  if has_key(decoded, 'total')
    call clap#state#refresh_matches_count(string(decoded.total))
  endif

  if has_key(decoded, 'lines')
    call g:clap.display.set_lines(decoded.lines)
  endif

  if has_key(decoded, 'truncated_map')
    let g:__clap_lines_truncated_map = decoded.truncated_map
  endif

  if has_key(decoded, 'indices')
    try
      call clap#highlight#add_fuzzy_async(decoded.indices)
    catch
      return
    endtry
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
