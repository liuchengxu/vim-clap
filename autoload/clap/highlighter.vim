" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Functions for adding highlights to the display window.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:default_priority = 10

if has('nvim')
  " lnum is 0-based.
  function! s:add_highlight_at(bufnr, lnum, col, hl_group) abort
    call nvim_buf_add_highlight(a:bufnr, -1, a:hl_group, a:lnum, a:col, a:col+1)
  endfunction

  " lnum and col are 0-based.
  function! s:add_highlight_at_using_matchaddpos(lnum, col, hl_group) abort
    return matchaddpos(a:hl_group, [[a:lnum+1, a:col+1, 1]])
  endfunction

  " The highlight added by nvim_buf_add_highlight() can be overrided
  " by the sign's highlight, therefore matchaddpos() is used for neovim.
  "
  " TODO: Once the default highlight priority of nvim_buf_add_highlight() is
  " higher, we could use the same impl with vim's s:apply_highlight().
  function! s:add_display_highlights_inner(hl_lines) abort
    " We should not use clearmatches() here.
    call g:clap.display.matchdelete()

    let w:clap_match_ids = []

    let lnum = 0
    for indices in a:hl_lines
      let group_idx = 1
      for idx in indices
        if group_idx < g:__clap_fuzzy_matches_hl_group_cnt + 1
          call add(w:clap_match_ids, s:add_highlight_at_using_matchaddpos(lnum, idx, 'ClapFuzzyMatches'.group_idx))
          let group_idx += 1
        else
          call add(w:clap_match_ids, s:add_highlight_at_using_matchaddpos(lnum, idx, g:__clap_fuzzy_last_hl_group))
        endif
      endfor
      let lnum += 1
    endfor
  endfunction

  if exists('*win_execute')
    function! s:add_display_highlights(hl_lines) abort
      call win_execute(g:clap.display.winid, 'call s:add_display_highlights_inner(a:hl_lines)')
    endfunction

    " This is same with g:clap.display.clear_highlight()
    function! clap#highlighter#clear_display() abort
      call win_execute(g:clap.display.winid, 'call g:clap.display.matchdelete()')
    endfunction
  else
    function! s:add_display_highlights(hl_lines) abort
      noautocmd call g:clap.display.goto_win()
      call s:add_display_highlights_inner(a:hl_lines)
      noautocmd call g:clap.input.goto_win()
    endfunction

    " This is same with g:clap.display.clear_highlight()
    function! clap#highlighter#clear_display() abort
      call g:clap.display.clear_highlight()
    endfunction
  endif

else
  " lnum is 0-based.
  function! s:add_highlight_at(bufnr, lnum, col, hl_group) abort
    " 1-based
    call prop_add(a:lnum+1, a:col+1, {'length': 1, 'type': a:hl_group, 'bufnr': a:bufnr})
  endfunction

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
          call s:add_highlight_at(g:clap.display.bufnr, lnum, idx, 'ClapFuzzyMatches'.group_idx)
          let group_idx += 1
        else
          call s:add_highlight_at(g:clap.display.bufnr, lnum, idx, g:__clap_fuzzy_last_hl_group)
        endif
      endfor
      let lnum += 1
    endfor
  endfunction

  function! clap#highlighter#clear_display() abort
  endfunction
endif

function! clap#highlighter#add_highlights(hl_lines) abort
  try
    call s:add_display_highlights(a:hl_lines)
  catch
    return
  endtry
endfunction

function! s:create_highlight_group(group_name, ctermfg, ctermbg, guifg, guibg) abort
  if !hlexists(a:group_name)
    execute printf(
          \ 'hi %s ctermfg=%s guifg=%s ctermbg=%s guibg=%s gui=None cterm=None',
          \ a:group_name,
          \ a:ctermfg,
          \ a:guifg,
          \ 'None',
          \ 'None',
          \ )
          " \ a:ctermbg,
          " \ a:guibg,
  endif
endfunction

function! clap#highlighter#highlight_line(bufnr, lnum, vim_highlights) abort
  for vim_highlight in a:vim_highlights
    call s:create_highlight_group(
          \ vim_highlight.group_name,
          \ vim_highlight.ctermfg,
          \ vim_highlight.ctermbg,
          \ vim_highlight.guifg,
          \ vim_highlight.guibg,
          \ )
    for col in vim_highlight.range
      call s:add_highlight_at(a:bufnr, a:lnum - 1, col, vim_highlight.group_name)
    endfor
  endfor
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
