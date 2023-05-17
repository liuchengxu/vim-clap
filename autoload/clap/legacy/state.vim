" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Change state of current filtering, e.g., matches count.

let s:save_cpo = &cpoptions
set cpoptions&vim

" NOTE: some local variable without explicit l:, e.g., count,
" may run into some erratic read-only error.
function! clap#legacy#state#refresh_matches_count(cnt) abort
  call clap#indicator#update_matched(a:cnt)
  call clap#sign#reset_to_first_line()
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
