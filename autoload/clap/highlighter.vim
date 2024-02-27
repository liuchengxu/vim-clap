" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Functions for adding highlights to the display window.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:default_priority = 10

if has('nvim')
  let s:tree_sitter_ns_id = nvim_create_namespace('clap_tree_sitter_highlight')
else
  let s:ts_prop_types = []
endif

if has('nvim')
  " lnum is 0-based.
  function! s:add_highlight_at(bufnr, lnum, col, length, hl_group) abort
    call nvim_buf_add_highlight(a:bufnr, -1, a:hl_group, a:lnum, a:col, a:col+a:length)
  endfunction

  " lnum is 0-based.
  function! s:add_ts_highlight_at(bufnr, lnum, col, length, hl_group) abort
    call nvim_buf_add_highlight(a:bufnr, s:tree_sitter_ns_id, a:hl_group, a:lnum, a:col, a:col+a:length)
  endfunction

  " lnum and col are 0-based.
  function! s:add_highlight_at_using_matchaddpos(lnum, col, hl_group) abort
    return matchaddpos(a:hl_group, [[a:lnum+1, a:col+1, 1]])
  endfunction

  " The highlight added by nvim_buf_add_highlight() can be overridden
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
  function! s:add_highlight_at(bufnr, lnum, col, length, hl_group) abort
    call prop_add(a:lnum+1, a:col+1, {'length': a:length, 'type': a:hl_group, 'bufnr': a:bufnr})
  endfunction

  " lnum is 0-based.
  function! s:add_ts_highlight_at(bufnr, lnum, col, length, hl_group) abort
    try
      call prop_add(a:lnum+1, a:col+1, {'length': a:length, 'type': a:hl_group, 'bufnr': a:bufnr})
    catch
      " Not sure why, but I keep run into error: Invalid line number, neovim
      " does not have this issue.
    endtry
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
          call s:add_highlight_at(g:clap.display.bufnr, lnum, idx, 1, 'ClapFuzzyMatches'.group_idx)
          let group_idx += 1
        else
          call s:add_highlight_at(g:clap.display.bufnr, lnum, idx, 1, g:__clap_fuzzy_last_hl_group)
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

function! s:create_token_highlight_group(token_highlight) abort
  execute printf(
        \ 'highlight %s ctermfg=%s guifg=%s cterm=%s gui=%s',
        \ a:token_highlight.group_name,
        \ a:token_highlight.ctermfg,
        \ a:token_highlight.guifg,
        \ a:token_highlight.cterm,
        \ a:token_highlight.gui,
        \ )

  if !has('nvim')
    call prop_type_add(a:token_highlight.group_name, {'highlight': a:token_highlight.group_name})
  endif
endfunction

" Highlight all the tokens at a specific line.
"
" lnum is 1-based.
function! clap#highlighter#highlight_line(bufnr, lnum, token_highlights) abort
  for token_highlight in a:token_highlights
    if !hlexists(token_highlight.group_name)
      call s:create_token_highlight_group(token_highlight)
    endif
    call s:add_highlight_at(a:bufnr, a:lnum - 1, token_highlight.col_start, token_highlight.length, token_highlight.group_name)
  endfor
endfunction

" Highlight a list of lines.
function! clap#highlighter#add_sublime_highlights(bufnr, line_highlights) abort
  for [lnum, line_highlight] in a:line_highlights
    call clap#highlighter#highlight_line(a:bufnr, lnum, line_highlight)
  endfor
endfunction

function! clap#highlighter#disable_tree_sitter(bufnr) abort
  if has('nvim')
    call nvim_buf_clear_namespace(a:bufnr, s:tree_sitter_ns_id, 0, -1)
  elseif !empty(s:ts_prop_types)
    call prop_remove({ 'types': s:ts_prop_types, 'all': v:true, 'bufnr': a:bufnr } )
  endif
endfunction

function! clap#highlighter#add_ts_highlights(bufnr, to_replace_line_ranges, highlights) abort
  if has('nvim')
    " All old highlights need to be replaced.
    if empty(a:to_replace_line_ranges)
      call nvim_buf_clear_namespace(a:bufnr, s:tree_sitter_ns_id, 0, -1)
    else
      for [start, end] in a:to_replace_line_ranges
        call nvim_buf_clear_namespace(a:bufnr, s:tree_sitter_ns_id, start, end)
      endfor
    endif
  elseif !empty(s:ts_prop_types)
    if empty(a:to_replace_line_ranges)
      call prop_remove({ 'types': s:ts_prop_types, 'all': v:true, 'bufnr': a:bufnr } )
    else
      for [start, end] in a:to_replace_line_ranges
        " start is 0-based
        call prop_remove({ 'types': s:ts_prop_types, 'bufnr': a:bufnr }, start+1, end)
      endfor
    endif
  endif

  for [line_number, highlights] in a:highlights
    for [column_start, length, group_name] in highlights
      if !has('nvim')
        if index(s:ts_prop_types, group_name) == -1
          call add(s:ts_prop_types, group_name)
          call prop_type_add(group_name, {'highlight': group_name})
        endif
      endif
      call s:add_ts_highlight_at(a:bufnr, line_number, column_start, length, group_name)
    endfor
  endfor
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
