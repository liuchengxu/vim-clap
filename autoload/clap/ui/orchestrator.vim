" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: UI orchestrator that coordinates all clap UI components.

let s:save_cpo = &cpoptions
set cpoptions&vim

" ============================================================================
" State Management
" ============================================================================

" UI state
let s:state = {
      \ 'visible': v:false,
      \ 'backend': '',
      \ 'layout': {},
      \ }

" Get current UI state.
function! clap#ui#orchestrator#get_state() abort
  return s:state
endfunction

" ============================================================================
" Component Coordination
" ============================================================================

" List of UI components in open order
let s:component_open_order = [
      \ 'display',
      \ 'border_left',
      \ 'spinner',
      \ 'input',
      \ 'preview',
      \ 'shadow',
      \ 'indicator',
      \ 'border_right',
      \ ]

" List of UI components in close order (reverse)
let s:component_close_order = [
      \ 'border_right',
      \ 'border_left',
      \ 'shadow',
      \ 'preview',
      \ 'indicator',
      \ 'spinner',
      \ 'input',
      \ 'display',
      \ ]

" ============================================================================
" Event Hooks
" ============================================================================

" Hook registry: event_name -> [callback_functions]
let s:hooks = {
      \ 'before_open': [],
      \ 'after_open': [],
      \ 'before_close': [],
      \ 'after_close': [],
      \ 'on_resize': [],
      \ 'on_result_update': [],
      \ }

" Register a hook callback.
function! clap#ui#orchestrator#on(event, callback) abort
  if has_key(s:hooks, a:event)
    call add(s:hooks[a:event], a:callback)
  endif
endfunction

" Unregister a hook callback.
function! clap#ui#orchestrator#off(event, callback) abort
  if has_key(s:hooks, a:event)
    call filter(s:hooks[a:event], 'v:val !=# a:callback')
  endif
endfunction

" Trigger hooks for an event.
function! s:trigger_hooks(event, ...) abort
  if has_key(s:hooks, a:event)
    for l:Callback in s:hooks[a:event]
      try
        if a:0 > 0
          call l:Callback(a:1)
        else
          call l:Callback()
        endif
      catch
        " Log error but don't break the flow
        echohl WarningMsg
        echom 'Clap UI hook error: ' . v:exception
        echohl None
      endtry
    endfor
  endif
endfunction

" ============================================================================
" Layout Management
" ============================================================================

" Update layout and notify components.
function! clap#ui#orchestrator#update_layout(layout) abort
  let s:state.layout = a:layout
  call s:trigger_hooks('on_resize', a:layout)
endfunction

" Get current layout.
function! clap#ui#orchestrator#get_layout() abort
  return s:state.layout
endfunction

" ============================================================================
" Result Count Updates
" ============================================================================

" Update indicator when results change.
function! clap#ui#orchestrator#on_results_updated(matched, ...) abort
  " Update indicator components
  if a:0 > 0
    call clap#ui#indicator_components#update_match_count(a:matched, a:1)
  else
    call clap#ui#indicator_components#update_match_count(a:matched)
  endif
  call clap#ui#indicator_components#render()

  " Handle dynamic height if enabled
  if clap#ui#layout_builder#is_dynamic_height()
    let l:new_height = clap#ui#layout_builder#dynamic_height(a:matched)
    if l:new_height != s:state.layout.height
      " Trigger resize
      call s:trigger_hooks('on_result_update', {
            \ 'matched': a:matched,
            \ 'height': l:new_height,
            \ })
    endif
  endif
endfunction

" ============================================================================
" Visibility Control
" ============================================================================

" Check if UI is visible.
function! clap#ui#orchestrator#is_visible() abort
  return s:state.visible
endfunction

" Set visibility state.
function! clap#ui#orchestrator#set_visible(visible) abort
  let s:state.visible = a:visible
endfunction

" ============================================================================
" Backend Detection
" ============================================================================

" Detect the appropriate UI backend.
function! clap#ui#orchestrator#detect_backend() abort
  if has('nvim')
    return 'floating'
  elseif has('popupwin')
    return 'popup'
  else
    return 'sidebar'
  endif
endfunction

" Get current backend.
function! clap#ui#orchestrator#get_backend() abort
  if empty(s:state.backend)
    let s:state.backend = clap#ui#orchestrator#detect_backend()
  endif
  return s:state.backend
endfunction

" ============================================================================
" High-Level Operations
" ============================================================================

" Prepare to open the UI.
function! clap#ui#orchestrator#prepare_open() abort
  call s:trigger_hooks('before_open')

  " Reset indicator
  call clap#ui#indicator_components#reset_match_count()

  " Calculate layout
  let l:layout = clap#ui#layout_builder#from_preset('center')
  call clap#ui#orchestrator#update_layout(l:layout)

  let s:state.visible = v:true
endfunction

" Finalize opening the UI.
function! clap#ui#orchestrator#finalize_open() abort
  call s:trigger_hooks('after_open')
endfunction

" Prepare to close the UI.
function! clap#ui#orchestrator#prepare_close() abort
  call s:trigger_hooks('before_close')
endfunction

" Finalize closing the UI.
function! clap#ui#orchestrator#finalize_close() abort
  let s:state.visible = v:false
  call clap#ui#indicator_components#clear()
  call s:trigger_hooks('after_close')
endfunction

" ============================================================================
" Window Focus Management
" ============================================================================

" Focus the input window.
function! clap#ui#orchestrator#focus_input() abort
  let l:backend = clap#ui#orchestrator#get_backend()
  if l:backend ==# 'floating'
    if exists('g:clap.input.winid') && nvim_win_is_valid(g:clap.input.winid)
      call nvim_set_current_win(g:clap.input.winid)
    endif
  elseif l:backend ==# 'popup'
    " Popup windows in Vim don't need explicit focus
  endif
endfunction

" Focus the display window.
function! clap#ui#orchestrator#focus_display() abort
  let l:backend = clap#ui#orchestrator#get_backend()
  if l:backend ==# 'floating'
    if exists('g:clap.display.winid') && nvim_win_is_valid(g:clap.display.winid)
      call nvim_set_current_win(g:clap.display.winid)
    endif
  endif
endfunction

" ============================================================================
" Preview Management
" ============================================================================

" Show preview with content.
function! clap#ui#orchestrator#show_preview(lines, ...) abort
  if !clap#preview#is_enabled()
    return
  endif

  let l:header = a:0 > 0 ? a:1 : ''
  let l:syntax = a:0 > 1 ? a:2 : ''
  let l:hi_line = a:0 > 2 ? a:3 : 0

  " Prepare content with header if provided
  if !empty(l:header)
    let l:content = clap#ui#preview_regions#prepare_content(l:header, a:lines)
    let l:hi_line = clap#ui#preview_regions#adjust_highlight_line(l:hi_line)
  else
    let l:content = a:lines
  endif

  " Show in preview window
  call g:clap.preview.show(l:content)

  " Apply syntax if provided
  if !empty(l:syntax)
    call g:clap.preview.set_syntax(l:syntax)
  endif

  " Apply highlight if line specified
  if l:hi_line > 0
    call g:clap.preview.add_highlight(l:hi_line)
  endif

  " Highlight header
  if !empty(l:header)
    call clap#preview#highlight_header()
  endif
endfunction

" Hide preview.
function! clap#ui#orchestrator#hide_preview() abort
  call g:clap#floating_win#preview.hide()
endfunction

" ============================================================================
" Initialization
" ============================================================================

" Initialize the orchestrator with default hooks.
function! clap#ui#orchestrator#init() abort
  " Reset state
  let s:state = {
        \ 'visible': v:false,
        \ 'backend': clap#ui#orchestrator#detect_backend(),
        \ 'layout': {},
        \ }
endfunction

" Auto-initialize on load
call clap#ui#orchestrator#init()

let &cpoptions = s:save_cpo
unlet s:save_cpo
