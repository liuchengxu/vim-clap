" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Functions for adding highlights to the display window.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:default_priority = 10

hi DefaultCurrentWord           ctermbg=60 guibg=#544a65
hi DefaultCurrentWordTwins      ctermbg=238 guibg=#444444

hi default link ClapCurrentWord      ErrorMsg
hi default link ClapCurrentWordTwins Search

function! clap#highlight#add_cursor_word_highlight(word_highlights) abort
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

if has('nvim')
  " lnum and col are 0-based.
  function! s:matchadd_char_at(lnum, col, hl_group) abort
    return matchaddpos(a:hl_group, [[a:lnum+1, a:col+1, 1]])
  endfunction

  " The highlight added by nvim_buf_add_highlight() can be overrided
  " by the sign's highlight, therefore matchaddpos() is used for neovim.
  function! s:add_display_highlights_impl(hl_lines) abort
    " We should not use clearmatches() here.
    call g:clap.display.matchdelete()

    let w:clap_match_ids = []

    let lnum = 0
    for indices in a:hl_lines
      let group_idx = 1
      for idx in indices
        if group_idx < g:__clap_fuzzy_matches_hl_group_cnt + 1
          call add(w:clap_match_ids, s:matchadd_char_at(lnum, idx, 'ClapFuzzyMatches'.group_idx))
          let group_idx += 1
        else
          call add(w:clap_match_ids, s:matchadd_char_at(lnum, idx, g:__clap_fuzzy_last_hl_group))
        endif
      endfor
      let lnum += 1
    endfor
  endfunction

  if exists('*win_execute')
    function! s:add_display_highlights(hl_lines) abort
      call win_execute(g:clap.display.winid, 'call s:add_display_highlights_impl(a:hl_lines)')
    endfunction

    " This is same with g:clap.display.clear_highlight()
    function! clap#highlight#clear() abort
      call win_execute(g:clap.display.winid, 'call g:clap.display.matchdelete()')
    endfunction
  else
    function! s:add_display_highlights(hl_lines) abort
      " Once the default highlight priority of nvim_buf_add_highlight() is
      " higher, we could use the same impl with vim's s:apply_highlight().

      noautocmd call g:clap.display.goto_win()
      call s:add_display_highlights_impl(a:hl_lines)
      noautocmd call g:clap.input.goto_win()
    endfunction

    " This is same with g:clap.display.clear_highlight()
    function! clap#highlight#clear() abort
      noautocmd call g:clap.display.goto_win()
      call g:clap.display.matchdelete()
      noautocmd call g:clap.input.goto_win()
    endfunction
  endif

  function! clap#highlight#add_highlight_at(lnum, col, hl_group) abort
    " 0-based
    call nvim_buf_add_highlight(g:clap.display.bufnr, -1, a:hl_group, a:lnum, a:col, a:col+1)
  endfunction

else
  function! s:add_display_highlights(hl_lines) abort
    " Avoid the error invalid buf
    if !bufexists(g:clap.display.bufnr)
      return
    endif
    " We do not have to clear the previous matches like neovim
    " as the previous lines have been deleted, and the associated text_props have also been removed.
    let lnum = 0
    for indices in a:hl_lines
      let group_idx = 1
      for idx in indices
        if group_idx < g:__clap_fuzzy_matches_hl_group_cnt + 1
          call clap#highlight#add_highlight_at(lnum, idx, 'ClapFuzzyMatches'.group_idx)
          let group_idx += 1
        else
          call clap#highlight#add_highlight_at(lnum, idx, g:__clap_fuzzy_last_hl_group)
        endif
      endfor
      let lnum += 1
    endfor
  endfunction

  function! clap#highlight#clear() abort
  endfunction

  function! clap#highlight#add_highlight_at(lnum, col, hl_group) abort
    " 1-based
    call prop_add(a:lnum+1, a:col+1, {'length': 1, 'type': a:hl_group, 'bufnr': g:clap.display.bufnr})
  endfunction

endif

function! clap#highlight#add_highlights(hl_lines) abort
  try
    call s:add_display_highlights(a:hl_lines)
  catch
    return
  endtry
endfunction

let s:highlight_delay_timer = -1
function! clap#highlight#add_highlights_with_delay(hl_lines) abort
  if s:highlight_delay_timer > 0
    call timer_stop(s:highlight_delay_timer)
  endif
  let s:highlight_delay_timer = timer_start(100, { -> clap#highlight#add_highlights(a:hl_lines)})
endfunction

" Add highlight for the substring matches.
function! clap#highlight#matchadd_substr(patterns) abort
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
