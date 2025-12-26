" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Centralized buffer and window lifecycle management for Neovim floating windows.

let s:save_cpo = &cpoptions
set cpoptions&vim

" ============================================================================
" Buffer Management
" ============================================================================

" Registry of managed buffers with their metadata.
let s:buffers = {}

" Create or get a scratch buffer for clap UI component.
" component: string identifier (e.g., 'display', 'input', 'spinner', 'preview')
function! clap#ui#window_manager#get_buffer(component) abort
  if has_key(s:buffers, a:component)
    let l:bufnr = s:buffers[a:component].bufnr
    if nvim_buf_is_valid(l:bufnr)
      return l:bufnr
    endif
  endif

  " Create new scratch buffer
  let l:bufnr = nvim_create_buf(v:false, v:true)
  let s:buffers[a:component] = { 'bufnr': l:bufnr }
  return l:bufnr
endfunction

" Get buffer number if it exists and is valid, otherwise return -1.
function! clap#ui#window_manager#try_get_buffer(component) abort
  if has_key(s:buffers, a:component)
    let l:bufnr = s:buffers[a:component].bufnr
    if nvim_buf_is_valid(l:bufnr)
      return l:bufnr
    endif
  endif
  return -1
endfunction

" Check if a buffer exists and is valid.
function! clap#ui#window_manager#buffer_valid(component) abort
  return clap#ui#window_manager#try_get_buffer(a:component) != -1
endfunction

" ============================================================================
" Window Management
" ============================================================================

" Registry of managed windows with their metadata.
let s:windows = {}

" Open or get a floating window for a component.
" component: string identifier
" bufnr: buffer to display
" opts: nvim_open_win options
" Returns: window id
function! clap#ui#window_manager#open_window(component, bufnr, opts) abort
  " Close existing window if present
  call clap#ui#window_manager#close_window(a:component)

  " Open new floating window
  silent let l:winid = nvim_open_win(a:bufnr, v:false, a:opts)

  " Store window reference
  let s:windows[a:component] = {
        \ 'winid': l:winid,
        \ 'bufnr': a:bufnr,
        \ }

  return l:winid
endfunction

" Open a floating window and focus it.
function! clap#ui#window_manager#open_window_focused(component, bufnr, opts) abort
  call clap#ui#window_manager#close_window(a:component)

  let l:focus_opts = copy(a:opts)
  silent let l:winid = nvim_open_win(a:bufnr, v:true, l:focus_opts)

  let s:windows[a:component] = {
        \ 'winid': l:winid,
        \ 'bufnr': a:bufnr,
        \ }

  return l:winid
endfunction

" Get window id for a component, or -1 if not exists/valid.
function! clap#ui#window_manager#get_window(component) abort
  if has_key(s:windows, a:component)
    let l:winid = s:windows[a:component].winid
    if nvim_win_is_valid(l:winid)
      return l:winid
    endif
  endif
  return -1
endfunction

" Check if window exists and is valid.
function! clap#ui#window_manager#window_valid(component) abort
  return clap#ui#window_manager#get_window(a:component) != -1
endfunction

" Close a window by component name.
function! clap#ui#window_manager#close_window(component) abort
  if has_key(s:windows, a:component)
    let l:winid = s:windows[a:component].winid
    if nvim_win_is_valid(l:winid)
      call clap#util#nvim_win_close_safe(l:winid)
    endif
    unlet s:windows[a:component]
  endif
endfunction

" Close all managed windows.
function! clap#ui#window_manager#close_all() abort
  for l:component in keys(s:windows)
    call clap#ui#window_manager#close_window(l:component)
  endfor
endfunction

" ============================================================================
" Window Configuration Helpers
" ============================================================================

" Apply common window settings (winhl, spell off, etc.)
function! clap#ui#window_manager#setup_window(winid, winhl, ...) abort
  call setwinvar(a:winid, '&winhl', a:winhl)
  call setwinvar(a:winid, '&spell', 0)

  " Apply additional options if provided
  if a:0 > 0
    for [l:key, l:val] in items(a:1)
      call setwinvar(a:winid, l:key, l:val)
    endfor
  endif
endfunction

" Apply minimal buffer styling.
function! clap#ui#window_manager#setup_buffer(bufnr, filetype) abort
  call setbufvar(a:bufnr, '&filetype', a:filetype)
  call setbufvar(a:bufnr, '&signcolumn', 'no')
  call setbufvar(a:bufnr, '&foldcolumn', 0)
endfunction

" Disable auto-completion and auto-pairs for input buffer.
function! clap#ui#window_manager#setup_input_buffer(bufnr) abort
  call setbufvar(a:bufnr, '&filetype', 'clap_input')
  call setbufvar(a:bufnr, 'coc_suggest_disable', 1)
  call setbufvar(a:bufnr, 'coc_pairs_disabled', ['"', "'", '(', ')', '<', '>', '[', ']', '{', '}', '`'])
  call setbufvar(a:bufnr, 'autopairs_loaded', 1)
  call setbufvar(a:bufnr, 'autopairs_enabled', 0)
  call setbufvar(a:bufnr, 'pear_tree_enabled', 0)
  call setbufvar(a:bufnr, 'ale_enabled', 0)
endfunction

" ============================================================================
" Window Config Adjustment Helpers
" ============================================================================

" Get current window config and modify it.
function! clap#ui#window_manager#modify_config(component, modifier) abort
  let l:winid = clap#ui#window_manager#get_window(a:component)
  if l:winid == -1
    return v:false
  endif

  let l:config = nvim_win_get_config(l:winid)
  call a:modifier(l:config)
  call nvim_win_set_config(l:winid, l:config)
  return v:true
endfunction

" Update window height.
function! clap#ui#window_manager#set_height(component, height) abort
  let l:winid = clap#ui#window_manager#get_window(a:component)
  if l:winid != -1
    call nvim_win_set_height(l:winid, a:height)
  endif
endfunction

" Update window width.
function! clap#ui#window_manager#set_width(component, width) abort
  let l:winid = clap#ui#window_manager#get_window(a:component)
  if l:winid != -1
    call nvim_win_set_width(l:winid, a:width)
  endif
endfunction

" ============================================================================
" Derived Config Helpers
" ============================================================================

" Create config relative to another window.
" base_component: component to derive from
" adjustments: dict with keys to adjust (row, col, width, height)
function! clap#ui#window_manager#derive_config(base_component, adjustments) abort
  let l:winid = clap#ui#window_manager#get_window(a:base_component)
  if l:winid == -1
    return {}
  endif

  let l:config = nvim_win_get_config(l:winid)

  " Apply adjustments
  for [l:key, l:val] in items(a:adjustments)
    if l:key ==# 'row_offset'
      let l:config.row += l:val
    elseif l:key ==# 'col_offset'
      let l:config.col += l:val
    elseif has_key(l:config, l:key)
      let l:config[l:key] = l:val
    endif
  endfor

  return l:config
endfunction

" Create a base config for floating windows.
function! clap#ui#window_manager#base_config(row, col, width, height, ...) abort
  let l:config = {
        \ 'relative': 'editor',
        \ 'style': 'minimal',
        \ 'row': a:row,
        \ 'col': a:col,
        \ 'width': a:width,
        \ 'height': a:height,
        \ }

  " Apply focusable (default false for non-input windows)
  let l:config.focusable = a:0 > 0 ? a:1 : v:false

  " Apply zindex if nvim-0.5+
  if has('nvim-0.5')
    let l:config.zindex = 1000
  endif

  return l:config
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
