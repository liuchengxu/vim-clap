" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Functions for adding highlights to the display window.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:default_priority = 10

" Add highlight for the substring matches using `matchadd()`.
function! clap#legacy#highlighter#highlight_substring(patterns) abort
  let w:clap_match_ids = []
  " Clap grep
  " \{ -> E888
  try
    call add(w:clap_match_ids, matchadd('ClapMatches', a:patterns[0], s:default_priority))
  catch
    " Sometimes we may run into some pattern errors in that the query is not a
    " valid vim pattern. Just ignore them as the highlight is not critical, we
    " care more about the searched results IMO.
    return
  endtry

  " As most 8 submatches, ClapMatches[1-8]
  try
    call map(a:patterns[1:8], 'add(w:clap_match_ids, matchadd("ClapMatches".(v:key+1), v:val, s:default_priority - 1))')
  catch
    return
  endtry
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
