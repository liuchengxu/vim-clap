" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Default implementation for various hooks.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')
let s:async_threshold = 10000

function! s:init_fuzzy_matches_hl_group() abort
  let clap_fuzzy_matches = [
        \ [46, '#00ff00'],
        \ [47, '#00ff5f'],
        \ [48, '#00ff87'],
        \ [49, '#00ffaf'],
        \ [50, '#00ffd7'],
        \ [51, '#00ffff'],
        \ [40, '#00d700'],
        \ [41, '#00d75f'],
        \ [42, '#00d787'],
        \ [43, '#00d7af'],
        \ [44, '#00d7d7'],
        \ [45, '#00d7ff'],
        \ ]

  let idx = 1
  for g in clap_fuzzy_matches
    if !hlexists('ClapFuzzyMatches'.idx)
      execute printf(
            \ 'hi ClapFuzzyMatches%s guifg=%s ctermfg=%s ctermbg=%s guibg=%s gui=bold cterm=bold', idx,
            \ g[1],
            \ g[0],
            \ 'NONE',
            \ 'NONE',
            \ )
      if !has('nvim')
        call prop_type_add('ClapFuzzyMatches'.idx, {'highlight': 'ClapFuzzyMatches'.idx})
      endif
    endif
    let idx += 1
  endfor

  let s:fuzzy_matches_hi_group_cnt = len(clap_fuzzy_matches)
endfunction

call s:init_fuzzy_matches_hl_group()

function! s:on_typed_sync_impl() abort
  call g:clap.display.clear_highlight()

  let l:cur_input = g:clap.input.get()

  if empty(l:cur_input)
    call g:clap.display.set_lines_lazy(g:clap.provider.get_source())
    let l:matches_cnt = g:clap.display.line_count() + len(g:clap.display.cache)
    call clap#indicator#set_matches('['.l:matches_cnt.']')
    call clap#sign#toggle_cursorline()
    call g:clap#display_win.compact_if_undersize()
    return
  endif

  call clap#spinner#set_busy()

  if get(g:, '__clap_should_refilter', v:false)
        \ || get(g:, '__clap_do_not_use_cache', v:false)
    let l:lines = g:clap.provider.get_source()
    let g:__clap_should_refilter = v:false
    let g:__clap_do_not_use_cache = v:false
  else
    " Assuming in the middle of typing, we are continuing to filter.
    let l:lines = g:clap.display.get_lines() + g:clap.display.cache

    " If there is no matches for the current filtered result, restore to the original source.
    if l:lines == [g:clap_no_matches_msg]
      let l:lines = g:clap.provider.get_source()
    endif
  endif

  let l:has_no_matches = v:false

  let l:lines = call(g:clap.provider.filter(), [l:cur_input, l:lines])

  if empty(l:lines)
    let l:lines = [g:clap_no_matches_msg]
    let l:has_no_matches = v:true
  endif

  call g:clap.display.set_lines_lazy(lines)

  " NOTE: some local variable without explicit l:, e.g., count,
  " may run into some erratic read-only error.
  if l:has_no_matches
    if get(g:clap.display, 'initial_size', -1) > 0
      let l:count = '0/'.g:clap.display.initial_size
    else
      let l:count = '0'
    endif
    call clap#indicator#set_matches('['.l:count.']')
    call clap#sign#disable_cursorline()
  else
    let l:matches_cnt = string(len(lines))
    if get(g:clap.display, 'initial_size', -1) > 0
      let l:matches_cnt .= '/'.g:clap.display.initial_size
    endif
    call clap#indicator#set_matches('['.l:matches_cnt.']')
    call clap#sign#reset_to_first_line()
  endif

  call g:clap#display_win.compact_if_undersize()
  call clap#spinner#set_idle()

  if !l:has_no_matches
    if exists('g:__clap_fuzzy_matched_indices')
      call s:add_highlight_for_fuzzy_matched()
    else
      call g:clap.display.add_highlight(l:cur_input)
    endif
  endif
endfunction

function! s:add_highlight_for_fuzzy_matched() abort
  " Due the cache strategy, g:__clap_fuzzy_matched_indices may be oversize
  " than the actual display buffer, the rest highlight indices of g:__clap_fuzzy_matched_indices
  " belong to the cached lines.
  " TODO: also add highlights for the cached lines?
  let hl_lines = g:__clap_fuzzy_matched_indices[:g:clap.display.line_count()-1]

  let lnum = 0

  for indices in hl_lines
    let group_idx = 1
    for idx in indices
      if group_idx < s:fuzzy_matches_hi_group_cnt + 1
        call clap#util#add_highlight_at(lnum, idx, 'ClapFuzzyMatches'.group_idx)
        let group_idx += 1
      else
        call clap#util#add_highlight_at(lnum, idx, 'ClapMatches')
      endif
    endfor
    let lnum += 1
  endfor
endfunction

function! s:apply_source_async() abort
  let cmd = g:clap.provider.source_async_or_default()
  call clap#dispatcher#job_start(cmd)
  call clap#spinner#set_busy()
endfunction

function! s:on_typed_async_impl() abort
  call g:clap.display.clear_highlight()
  let l:cur_input = g:clap.input.get()

  if empty(l:cur_input)
    return
  endif

  call g:clap.display.clear()

  call clap#util#run_rooter(function('s:apply_source_async'))

  call g:clap.display.add_highlight(l:cur_input)
endfunction

" Choose the suitable way according to the source size.
function! s:should_switch_to_async() abort
  if g:clap.provider.is_pure_async()
        \ || g:clap.provider.type == g:__t_string
        \ || g:clap.provider.type == g:__t_func_string
    return v:true
  endif

  let Source = g:clap.provider._().source

  if g:clap.provider.type == g:__t_list
    let s:cur_source = Source
  elseif g:clap.provider.type == g:__t_func_list
    let s:cur_source = Source()
  endif

  if len(s:cur_source) > s:async_threshold
    return v:true
  endif

  return v:false
endfunction

"                          filter
"                       /  (sync)
"             on_typed -
"           /           \
"          /              dispatcher
" on_enter                 (async)        --> on_exit
"          \
"           \
"             on_move
"
function! clap#impl#on_typed() abort
  if g:clap.provider.can_async()
    " Run async explicitly
    if get(g:clap.context, 'async') is v:true
      call s:on_typed_async_impl()
    elseif s:should_switch_to_async()
      call s:on_typed_async_impl()
    else
      call s:on_typed_sync_impl()
    endif
  else
    call s:on_typed_sync_impl()
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
