" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Functions for highlighting the fuzzy matched items.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:related_maple_providers = ['files', 'git_files']
let s:related_builtin_providers = ['tags', 'buffers', 'files', 'git_files', 'history', 'filer']

let s:default_priority = 10

" The icon can interfer the matched indices of fuzzy filter, but not the
" substring filter.
function! s:should_check_offset() abort
  return g:clap_enable_icon && stridx(g:clap.input.get(), ' ') == -1
endfunction

function! s:builtin_fuzzy_idx_offset() abort
  if s:should_check_offset()
        \ && index(s:related_builtin_providers, g:clap.provider.id) > -1
      return 2
  else
    return 0
  endif
endfunction

function! clap#highlight#provider_has_offset() abort
  return s:builtin_fuzzy_idx_offset() > 0
endfunction

function! s:maple_fuzzy_idx_offset() abort
  if s:should_check_offset()
        \ && index(s:related_maple_providers, g:clap.provider.id) > -1
      return 4
  else
    return 0
  endif
endfunction

if has('nvim')
  function! s:apply_add_highlight(hl_lines, offset) abort
    " Currently neovim does not have win_execute()
    " and the highlight added by nvim_buf_add_highlight()
    " can be overrided by the sign's highlight.
    "
    " Once the default highlight priority of nvim_buf_add_highlight() is
    " higher, we could use the same impl with vim's s:apply_highlight().

    call g:clap.display.goto_win()
    " We should not use clearmatches() here.
    call g:clap.display.matchdelete()

    let w:clap_match_ids = []

    let lnum = 0
    for indices in a:hl_lines
      let group_idx = 1
      for idx in indices
        if group_idx < g:__clap_fuzzy_matches_hl_group_cnt + 1
          call add(w:clap_match_ids, clap#highlight#matchadd_char_at(lnum, idx+a:offset, 'ClapFuzzyMatches'.group_idx))
          let group_idx += 1
        else
          call add(w:clap_match_ids, clap#highlight#matchadd_char_at(lnum, idx+a:offset, g:__clap_fuzzy_last_hl_group))
        endif
      endfor
      let lnum += 1
    endfor

    call g:clap.input.goto_win()
  endfunction

  " This is same with g:clap.display.clear_highlight()
  function! clap#highlight#clear() abort
    call g:clap.display.goto_win()
    call g:clap.display.matchdelete()
    call g:clap.input.goto_win()
  endfunction

  function! clap#highlight#add_highlight_at(lnum, col, hl_group) abort
    " 0-based
    call nvim_buf_add_highlight(g:clap.display.bufnr, -1, a:hl_group, a:lnum, a:col, a:col+1)
  endfunction

else
  function! s:apply_add_highlight(hl_lines, offset) abort
    " We do not have to clear the previous matches like neovim
    " as the previous lines have been deleted, and the associated text_props have also been removed.
    let lnum = 0
    for indices in a:hl_lines
      let group_idx = 1
      for idx in indices
        if group_idx < g:__clap_fuzzy_matches_hl_group_cnt + 1
          call clap#highlight#add_highlight_at(lnum, idx+a:offset, 'ClapFuzzyMatches'.group_idx)
          let group_idx += 1
        else
          call clap#highlight#add_highlight_at(lnum, idx+a:offset, g:__clap_fuzzy_last_hl_group)
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

" Used by the built-in sync filter.
function! clap#highlight#add_fuzzy_sync() abort
  " Due the cache strategy, g:__clap_fuzzy_matched_indices may be oversize
  " than the actual display buffer, the rest highlight indices of g:__clap_fuzzy_matched_indices
  " belong to the cached lines.
  "
  " TODO: also add highlights for the cached lines?
  let hl_lines = g:__clap_fuzzy_matched_indices[:g:clap.display.line_count()-1]
  let offset = s:builtin_fuzzy_idx_offset()

  call s:apply_add_highlight(hl_lines, offset)
endfunction

" Used by the async job.
function! clap#highlight#add_fuzzy_async(hl_lines) abort
  let offset = s:maple_fuzzy_idx_offset()
  call s:apply_add_highlight(a:hl_lines, offset)
endfunction

function! clap#highlight#fg_only(group_name, cermfg, guifg) abort
  if !hlexists(a:group_name)
    execute printf(
          \ 'hi %s ctermfg=%s guifg=%s ctermbg=%s guibg=%s gui=bold cterm=bold',
          \ a:group_name,
          \ a:cermfg,
          \ a:guifg,
          \ 'NONE',
          \ 'NONE',
          \ )
  endif
endfunction

" lnum and col are 0-based.
function! clap#highlight#matchadd_char_at(lnum, col, hl_group) abort
  return matchaddpos(a:hl_group, [[a:lnum+1, a:col+1, 1]])
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
    call map(a:patterns[1:8],
          \ {key, val -> add(w:clap_match_ids, matchadd('ClapMatches'.(key+1), val, s:default_priority -1))})
  catch
    return
  endtry
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
