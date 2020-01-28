" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Default implementation for various hooks.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')

" =======================================
" sync implementation
" =======================================
function! s:reset_on_empty_input() abort
  call g:clap.display.set_lines_lazy(s:get_cache_or_raw_source())
  call clap#indicator#set_matches('['.g:__clap_initial_source_size.']')
  call clap#sign#toggle_cursorline()
  call g:clap#display_win.shrink_if_undersize()
  call g:clap.preview.hide()
endfunction

" g:__clap_forerunner_result is fetched in async.
" g:clap.provider.get_source() is sync.
function! s:get_cache_or_raw_source() abort
  if exists('g:__clap_forerunner_result')
    if !exists('g:__clap_initial_source_size')
      let g:__clap_initial_source_size = g:clap.display.initial_size
    endif
    return g:__clap_forerunner_result
  endif
  if !exists('g:__clap_raw_source')
    let g:__clap_raw_source = g:clap.provider.get_source()
    let g:__clap_initial_source_size = len(g:__clap_raw_source)
  endif
  return g:__clap_raw_source
endfunction

function! s:get_source() abort
  if get(g:, '__clap_should_refilter', v:false)
        \ || get(g:, '__clap_do_not_use_cache', v:false)
    let l:lines = s:get_cache_or_raw_source()
    let g:__clap_should_refilter = v:false
    let g:__clap_do_not_use_cache = v:false
  else
    " Assuming in the middle of typing, we are continuing to filter.
    let l:lines = g:clap.display.get_lines() + g:clap.display.cache

    " If there is no matches for the current filtered result, restore to the original source.
    if l:lines == [g:clap_no_matches_msg]
      let l:lines = s:get_cache_or_raw_source()
    endif
  endif
  return l:lines
endfunction

" NOTE: some local variable without explicit l:, e.g., count,
" may run into some erratic read-only error.
function! clap#impl#refresh_matches_count(cnt_str) abort
  let l:matches_cnt = a:cnt_str

  if get(g:clap.display, 'initial_size', -1) > 0
    let l:matches_cnt .= '/'.g:clap.display.initial_size
  endif

  call clap#indicator#set_matches('['.l:matches_cnt.']')
  call clap#sign#reset_to_first_line()
endfunction

function! s:on_typed_sync_impl() abort
  call g:clap.display.clear_highlight()

  let l:cur_input = g:clap.input.get()

  if empty(l:cur_input)
    call s:reset_on_empty_input()
    return
  endif

  call clap#spinner#set_busy()

  " Do not use get(g:, '__clap_forerunner_result', s:get_source()) as vim
  " evaluates the default value of get(...) any how.
  if exists('g:__clap_forerunner_result')
    let l:raw_lines = g:__clap_forerunner_result
  else
    let l:raw_lines = s:get_source()
  endif

  call clap#filter#on_typed(g:clap.provider.filter(), l:cur_input, l:raw_lines)
endfunction

" =======================================
" async implementation
" =======================================
function! s:on_typed_async_impl() abort
  call g:clap.display.clear_highlight()
  let l:cur_input = g:clap.input.get()

  if empty(l:cur_input)
    if exists('g:__clap_raw_source')
      call g:clap.display.set_lines_lazy(g:__clap_raw_source)
      call clap#indicator#set_matches('['.g:__clap_initial_source_size.']')
      call clap#sign#toggle_cursorline()
      call g:clap#display_win.shrink_if_undersize()
      call g:clap.preview.hide()
    endif
    call clap#highlight#clear()
    return
  endif

  " Do not clear the outdated content as it would cause the annoying flicker.
  " call g:clap.display.clear()

  let cmd = g:clap.provider.source_async_or_default()

  if clap#filter#external#using_maple()
    call clap#rooter#run(function('clap#maple#job_start'), cmd)
  else
    call clap#rooter#run(function('clap#dispatcher#job_start'), cmd)
  endif

  call clap#spinner#set_busy()
endfunction

" Choose the suitable way according to the source size.
function! s:detect_should_switch_to_async() abort
  " Optimze for blines provider.
  if g:clap.provider.id ==# 'blines'
        \ && g:clap.display.initial_size > 100000
    return v:true
  endif

  if g:clap.provider.is_pure_async()
        \ || g:clap.provider.source_type == g:__t_string
        \ || g:clap.provider.source_type == g:__t_func_string
    return v:true
  endif

  let Source = g:clap.provider._().source

  if g:clap.provider.source_type == g:__t_list
    let s:cur_source = Source
  elseif g:clap.provider.source_type == g:__t_func_list
    let s:cur_source = Source()
  endif

  let g:__clap_raw_source = s:cur_source
  let g:__clap_initial_source_size = len(g:__clap_raw_source)

  if clap#filter#beyond_capacity(g:__clap_initial_source_size)
    return v:true
  endif

  return v:false
endfunction

function! s:should_switch_to_async() abort
  if has_key(g:clap.provider, 'should_switch_to_async')
    return g:clap.provider.should_switch_to_async
  else
    let should_switch_to_async = s:detect_should_switch_to_async()
    let g:clap.provider.should_switch_to_async = should_switch_to_async
    return should_switch_to_async
  endif
endfunction

"                          filter
"                       /  (sync/async)
"             on_typed -
"           /           \
"          /              dispatcher
" on_enter                 (async)        --> on_exit
"          \
"           \
"             on_move
"
function! clap#impl#on_typed() abort
  " If user explicitly uses the external filter, just use the async impl then,
  " even the forerunner job is finished already.
  if clap#api#has_externalfilter()
    call s:on_typed_async_impl()
    return
  endif
  if exists('g:__clap_forerunner_result')
    call s:on_typed_sync_impl()
    return
  endif
  if g:clap.provider.can_async() &&
        \ (get(g:clap.context, 'async') is v:true || s:should_switch_to_async())
    call s:on_typed_async_impl()
  else
    call s:on_typed_sync_impl()
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
