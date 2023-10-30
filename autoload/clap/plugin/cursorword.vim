" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Highlight the cursor word and the occurrences

let s:save_cpo = &cpoptions
set cpoptions&vim

hi ClapUnderline gui=underline cterm=underline

hi default link ClapCursorWord      IncSearch
hi default link ClapCursorWordTwins ClapUnderline

function! clap#plugin#cursorword#add_highlights(word_highlights) abort
  let cword_len = a:word_highlights.cword_len
  let match_ids = []
  let [lnum, col] = a:word_highlights.cword_highlight
  let match_id = matchaddpos('ClapCursorWord', [[lnum, col+1, cword_len]])
  if match_id > -1
    call add(match_ids, match_id)
  endif
  for [lnum, col] in a:word_highlights.twins_words_highlight
    let match_id = matchaddpos('ClapCursorWordTwins', [[lnum, col+1, cword_len]])
    if match_id > -1
      call add(match_ids, match_id)
    endif
  endfor
  return match_ids
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
