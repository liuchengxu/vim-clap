" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Modular preview regions for extensible preview window management.

let s:save_cpo = &cpoptions
set cpoptions&vim

" ============================================================================
" Preview Region Configuration
" ============================================================================

" Preview regions configuration
let s:regions = {
      \ 'header': {
      \   'enabled': v:true,
      \   'height': 1,
      \   'highlight': 'Title',
      \ },
      \ 'content': {
      \   'enabled': v:true,
      \ },
      \ 'scrollbar': {
      \   'enabled': get(g:, 'clap_preview_scrollbar_enabled', v:true),
      \   'width': 1,
      \ },
      \ }

" ============================================================================
" Header Region
" ============================================================================

" Format the header line with file info.
" Returns: formatted header string
function! clap#ui#preview_regions#format_header(fpath, ...) abort
  let l:header = a:fpath

  " Add optional line number
  if a:0 > 0 && a:1 > 0
    let l:header .= ':' . a:1
  endif

  " Add file icon if available
  if exists('*clap#icon#get')
    let l:icon = clap#icon#get(a:fpath)
    if !empty(l:icon)
      let l:header = l:icon . ' ' . l:header
    endif
  endif

  return l:header
endfunction

" Format header with additional metadata (line count, language, etc.)
function! clap#ui#preview_regions#format_header_rich(fpath, ...) abort
  let l:parts = [a:fpath]

  " Add line number if provided
  if a:0 > 0 && a:1 > 0
    let l:parts[0] .= ':' . a:1
  endif

  " Add file icon
  if exists('*clap#icon#get')
    let l:icon = clap#icon#get(a:fpath)
    if !empty(l:icon)
      let l:parts = [l:icon] + l:parts
    endif
  endif

  " Add line count if file exists
  if filereadable(a:fpath)
    let l:lines = len(readfile(a:fpath))
    call add(l:parts, l:lines . ' lines')
  endif

  " Add language/filetype
  let l:ext = fnamemodify(a:fpath, ':e')
  if !empty(l:ext)
    call add(l:parts, l:ext)
  endif

  return join(l:parts, '  |  ')
endfunction

" ============================================================================
" Content Region
" ============================================================================

" Prepare content lines with header.
" Returns: list of lines with header at top
function! clap#ui#preview_regions#prepare_content(header, lines) abort
  if s:regions.header.enabled
    return [a:header] + a:lines
  else
    return a:lines
  endif
endfunction

" Get the content start line (1-indexed).
" Returns: line number where actual content starts (after header)
function! clap#ui#preview_regions#content_start_line() abort
  return s:regions.header.enabled ? 2 : 1
endfunction

" Calculate highlight line offset for header.
function! clap#ui#preview_regions#adjust_highlight_line(line) abort
  return s:regions.header.enabled ? a:line + 1 : a:line
endfunction

" ============================================================================
" Scrollbar Region
" ============================================================================

let s:scrollbar_state = {
      \ 'total_lines': 0,
      \ 'visible_lines': 0,
      \ 'scroll_offset': 0,
      \ }

" Update scrollbar state.
function! clap#ui#preview_regions#update_scrollbar_state(total, visible, offset) abort
  let s:scrollbar_state.total_lines = a:total
  let s:scrollbar_state.visible_lines = a:visible
  let s:scrollbar_state.scroll_offset = a:offset
endfunction

" Calculate scrollbar position and length.
" Returns: { 'top': line_offset, 'length': thumb_length }
function! clap#ui#preview_regions#calc_scrollbar_position(preview_height) abort
  let l:total = s:scrollbar_state.total_lines
  let l:visible = s:scrollbar_state.visible_lines
  let l:offset = s:scrollbar_state.scroll_offset

  if l:total <= l:visible || l:total == 0
    " No scrollbar needed
    return { 'top': 0, 'length': a:preview_height }
  endif

  " Calculate thumb length (proportional to visible/total ratio)
  let l:thumb_ratio = (l:visible * 1.0) / l:total
  let l:thumb_length = max([1, float2nr(a:preview_height * l:thumb_ratio)])

  " Calculate thumb position
  let l:scroll_ratio = (l:offset * 1.0) / (l:total - l:visible)
  let l:thumb_top = float2nr((a:preview_height - l:thumb_length) * l:scroll_ratio)

  return { 'top': l:thumb_top, 'length': l:thumb_length }
endfunction

" ============================================================================
" Region Management
" ============================================================================

" Enable/disable a region.
function! clap#ui#preview_regions#set_region_enabled(region, enabled) abort
  if has_key(s:regions, a:region)
    let s:regions[a:region].enabled = a:enabled
  endif
endfunction

" Check if a region is enabled.
function! clap#ui#preview_regions#is_region_enabled(region) abort
  return get(get(s:regions, a:region, {}), 'enabled', v:false)
endfunction

" Get region configuration.
function! clap#ui#preview_regions#get_region(region) abort
  return get(s:regions, a:region, {})
endfunction

" ============================================================================
" Preview Line Numbers (Optional Feature)
" ============================================================================

let s:show_line_numbers = get(g:, 'clap_preview_line_numbers', 0)

" Enable line numbers in preview.
function! clap#ui#preview_regions#enable_line_numbers(winid) abort
  if s:show_line_numbers
    call setwinvar(a:winid, '&number', 1)
    call setwinvar(a:winid, '&relativenumber', 0)
  endif
endfunction

" Toggle line numbers.
function! clap#ui#preview_regions#toggle_line_numbers() abort
  let s:show_line_numbers = !s:show_line_numbers
endfunction

" ============================================================================
" Preview Size Calculation
" ============================================================================

" Calculate available preview height based on layout.
function! clap#ui#preview_regions#calc_available_height(display_opts) abort
  let l:direction = clap#preview#direction()
  if l:direction ==# 'LR'
    return a:display_opts.height
  else
    let l:max = &lines - a:display_opts.row - a:display_opts.height - &cmdheight
    return float2nr(l:max)
  endif
endfunction

" Get the configured preview size for a provider.
function! clap#ui#preview_regions#get_size(provider_id) abort
  return clap#preview#size_of(a:provider_id)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
