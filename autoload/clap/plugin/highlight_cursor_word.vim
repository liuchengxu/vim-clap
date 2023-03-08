" Author: liuchengxu <xuliuchengxlc@gmail.com>

let s:save_cpo = &cpoptions
set cpoptions&vim

hi default link ClapCurrentWord      IncSearch
hi default link ClapCurrentWordTwins Search

function! clap#plugin#highlight_cursor_word#add_highlights(word_highlights) abort
  let cword_len = a:word_highlights.cword_len
  let match_ids = []
  let [lnum, col] = a:word_highlights.cword_highlight
  let match_id = matchaddpos('ClapCurrentWord', [[lnum, col+1, cword_len]])
  if match_id > -1
    call add(match_ids, match_id)
  endif
  for [lnum, col] in a:word_highlights.other_words_highlight
    let match_id = matchaddpos('ClapCurrentWordTwins', [[lnum, col+1, cword_len]])
    if match_id > -1
      call add(match_ids, match_id)
    endif
  endfor
  return match_ids
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
