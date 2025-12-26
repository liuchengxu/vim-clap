" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Modular indicator components for extensible status display.

let s:save_cpo = &cpoptions
set cpoptions&vim

" ============================================================================
" Component Registry
" ============================================================================

" Each component is a dict with:
"   - id: unique identifier
"   - priority: lower = more left (0-100)
"   - render: function that returns string to display
"   - width: function that returns component width (optional)

let s:components = []

" Register a new indicator component.
function! clap#ui#indicator_components#register(id, priority, render_fn, ...) abort
  let l:component = {
        \ 'id': a:id,
        \ 'priority': a:priority,
        \ 'render': a:render_fn,
        \ }
  if a:0 > 0
    let l:component.width = a:1
  endif

  " Remove existing component with same id
  call filter(s:components, 'v:val.id !=# a:id')
  call add(s:components, l:component)

  " Sort by priority
  call sort(s:components, {a, b -> a.priority - b.priority})
endfunction

" Unregister a component by id.
function! clap#ui#indicator_components#unregister(id) abort
  call filter(s:components, 'v:val.id !=# a:id')
endfunction

" ============================================================================
" Match Count Component
" ============================================================================

let s:match_state = { 'matched': 0, 'processed': -1 }

function! s:match_count_render() abort
  let l:selected = clap#sign#current_selections_count()
  if s:match_state.processed == -1
    return printf('%d [%d]', s:match_state.matched, l:selected)
  else
    return printf('%d/%d [%d]', s:match_state.matched, s:match_state.processed, l:selected)
  endif
endfunction

function! clap#ui#indicator_components#update_match_count(matched, ...) abort
  let s:match_state.matched = a:matched
  if a:0 > 0
    let s:match_state.processed = a:1
  endif
endfunction

function! clap#ui#indicator_components#reset_match_count() abort
  let s:match_state.matched = 0
  let s:match_state.processed = -1
endfunction

function! clap#ui#indicator_components#decrement_match_count() abort
  let s:match_state.matched -= 1
  if s:match_state.processed > 0
    let s:match_state.processed -= 1
  endif
endfunction

" Register the match count component by default
call clap#ui#indicator_components#register('match_count', 10, function('s:match_count_render'))

" ============================================================================
" Keybind Hints Component (Optional)
" ============================================================================

let s:show_keybind_hints = get(g:, 'clap_show_keybind_hints', 0)

" Default keybind hints - can be customized
let s:keybind_hints = get(g:, 'clap_keybind_hints', {
      \ 'Tab': 'preview',
      \ 'CR': 'open',
      \ 'C-t': 'tab',
      \ 'C-x': 'split',
      \ 'C-v': 'vsplit',
      \ })

function! s:keybind_hints_render() abort
  if !s:show_keybind_hints
    return ''
  endif

  let l:hints = []
  for [l:key, l:action] in items(s:keybind_hints)
    call add(l:hints, l:key . ':' . l:action)
  endfor
  return ' ' . join(l:hints, ' ')
endfunction

" Enable/disable keybind hints display.
function! clap#ui#indicator_components#toggle_keybind_hints() abort
  let s:show_keybind_hints = !s:show_keybind_hints
  call clap#ui#indicator_components#render()
endfunction

function! clap#ui#indicator_components#set_keybind_hints(hints) abort
  let s:keybind_hints = a:hints
endfunction

" Register keybind hints component (lower priority = more right)
if s:show_keybind_hints
  call clap#ui#indicator_components#register('keybind_hints', 50, function('s:keybind_hints_render'))
endif

" ============================================================================
" Provider Mode Component (Optional)
" ============================================================================

function! s:provider_mode_render() abort
  if exists('g:clap') && has_key(g:clap, 'provider') && has_key(g:clap.provider, 'mode')
    let l:mode = g:clap.provider.mode()
    if l:mode !=# 'full'
      return ' [' . l:mode . ']'
    endif
  endif
  return ''
endfunction

" Optionally register provider mode component
if get(g:, 'clap_show_provider_mode', 0)
  call clap#ui#indicator_components#register('provider_mode', 20, function('s:provider_mode_render'))
endif

" ============================================================================
" Rendering
" ============================================================================

" Render all components into a single string.
function! clap#ui#indicator_components#render_string() abort
  let l:parts = []
  for l:component in s:components
    let l:rendered = l:component.render()
    if !empty(l:rendered)
      call add(l:parts, l:rendered)
    endif
  endfor
  return join(l:parts, '')
endfunction

" Get the total width of all rendered components.
function! clap#ui#indicator_components#total_width() abort
  return strlen(clap#ui#indicator_components#render_string())
endfunction

" Padding helper for right-aligned display.
function! s:pad_left(text, target_width) abort
  let l:text_len = strlen(a:text)
  if l:text_len < a:target_width
    return repeat(' ', a:target_width - l:text_len) . a:text
  endif
  return a:text
endfunction

" Render to the indicator buffer/window.
function! clap#ui#indicator_components#render() abort
  if g:clap_disable_matches_indicator
    return
  endif

  let l:content = clap#ui#indicator_components#render_string()
  let l:padded = s:pad_left(l:content, get(g:, '__clap_indicator_winwidth', 0))

  if has('nvim')
    if bufexists(g:__clap_indicator_bufnr)
      call setbufline(g:__clap_indicator_bufnr, 1, l:padded)
    endif
  else
    if exists('g:__clap_indicator_winid')
      call popup_settext(g:__clap_indicator_winid, l:padded)
    endif
  endif
endfunction

" Clear the indicator display.
function! clap#ui#indicator_components#clear() abort
  let l:blank = repeat(' ', &columns) . ' for eliminating the trailing char'
  if has('nvim')
    if bufexists(g:__clap_indicator_bufnr)
      call setbufline(g:__clap_indicator_bufnr, 1, l:blank)
    endif
  else
    if exists('g:__clap_indicator_winid')
      call popup_settext(g:__clap_indicator_winid, l:blank)
    endif
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
