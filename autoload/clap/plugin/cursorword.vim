" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Highlight the cursor word and the occurrences

let s:save_cpo = &cpoptions
set cpoptions&vim

hi ClapUnderline gui=underline cterm=underline

hi default link ClapCursorWord      IncSearch
hi default link ClapCursorWordTwins ClapUnderline

augroup VimClapCursorword
  autocmd!

  autocmd ColorScheme * call clap#client#notify('cursorword.__defineHighlights', [+expand('<abuf>')])
augroup END

function! clap#plugin#cursorword#add_keyword_highlights(keyword_highlights) abort
  let match_ids = []
  for hl in a:keyword_highlights
    let match_id = matchaddpos('Error', [[hl.line_number, hl.col+1, hl.length]])
    if match_id > -1
      call add(match_ids, match_id)
    endif
  endfor
  return match_ids
endfunction

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

function! clap#plugin#cursorword#define_highlights(highlights, twins_highlights) abort
  let [ctermbg, guibg] = a:highlights
  let [twins_ctermbg, twins_guibg] = a:twins_highlights

  execute printf('highlight ClapCursorWord       ctermbg=%d guibg=%s', ctermbg, guibg)
  execute printf('highlight ClapCursorWordTwins  ctermbg=%d guibg=%s', twins_ctermbg, twins_guibg)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
