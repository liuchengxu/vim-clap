" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Modular layout builder for flexible window arrangement.

let s:save_cpo = &cpoptions
set cpoptions&vim

" ============================================================================
" Layout Configuration
" ============================================================================

" Layout presets
let s:presets = {
      \ 'center': {
      \   'row': 'center',
      \   'col': 'center',
      \   'width': '60%',
      \   'height': 'auto',
      \ },
      \ 'top': {
      \   'row': '10%',
      \   'col': 'center',
      \   'width': '70%',
      \   'height': 'auto',
      \ },
      \ 'bottom': {
      \   'row': 'bottom',
      \   'col': 'center',
      \   'width': '100%',
      \   'height': '30%',
      \ },
      \ }

" Current layout state
let s:current_layout = {}

" ============================================================================
" Dimension Calculation
" ============================================================================

" Parse a dimension value (can be number, percentage, or keyword).
" Returns: absolute pixel value
function! s:parse_dimension(value, max_value) abort
  if type(a:value) == v:t_number
    return a:value
  endif

  if type(a:value) == v:t_string
    if a:value =~# '%$'
      let l:percent = str2nr(substitute(a:value, '%$', '', ''))
      return float2nr((a:max_value * l:percent) / 100.0)
    elseif a:value ==# 'center'
      return -1  " Special value, handled by position calculation
    elseif a:value ==# 'auto'
      return -1  " Special value, handled by auto-sizing
    elseif a:value ==# 'bottom'
      return a:max_value - 10  " Near bottom
    endif
  endif

  return a:value
endfunction

" Calculate width based on configuration.
function! clap#ui#layout_builder#calc_width(config) abort
  let l:width = get(a:config, 'width', '60%')
  let l:parsed = s:parse_dimension(l:width, &columns)

  " Apply min/max constraints
  let l:min_width = get(a:config, 'min_width', 40)
  let l:max_width = get(a:config, 'max_width', &columns - 4)

  return max([l:min_width, min([l:max_width, l:parsed])])
endfunction

" Calculate height based on configuration.
function! clap#ui#layout_builder#calc_height(config) abort
  let l:height = get(a:config, 'height', 'auto')

  if l:height ==# 'auto' || l:height == -1
    " Auto height: use display_height setting or default
    return get(g:, 'clap_layout', {}).height
          \ ? g:clap_layout.height
          \ : float2nr(&lines * 0.3)
  endif

  let l:parsed = s:parse_dimension(l:height, &lines)

  " Apply min/max constraints
  let l:min_height = get(a:config, 'min_height', 3)
  let l:max_height = get(a:config, 'max_height', &lines - 4)

  return max([l:min_height, min([l:max_height, l:parsed])])
endfunction

" Calculate row position (vertical).
function! clap#ui#layout_builder#calc_row(config, height) abort
  let l:row = get(a:config, 'row', 'center')

  if l:row ==# 'center' || l:row == -1
    return float2nr((&lines - a:height) / 2.0)
  elseif l:row ==# 'top'
    return get(a:config, 'margin_top', 2)
  elseif l:row ==# 'bottom'
    return &lines - a:height - get(a:config, 'margin_bottom', 2) - &cmdheight
  endif

  return s:parse_dimension(l:row, &lines)
endfunction

" Calculate column position (horizontal).
function! clap#ui#layout_builder#calc_col(config, width) abort
  let l:col = get(a:config, 'col', 'center')

  if l:col ==# 'center' || l:col == -1
    return float2nr((&columns - a:width) / 2.0)
  elseif l:col ==# 'left'
    return get(a:config, 'margin_left', 2)
  elseif l:col ==# 'right'
    return &columns - a:width - get(a:config, 'margin_right', 2)
  endif

  return s:parse_dimension(l:col, &columns)
endfunction

" ============================================================================
" Layout Building
" ============================================================================

" Build a complete layout configuration.
" Returns: { row, col, width, height, ... }
function! clap#ui#layout_builder#build(config) abort
  let l:width = clap#ui#layout_builder#calc_width(a:config)
  let l:height = clap#ui#layout_builder#calc_height(a:config)
  let l:row = clap#ui#layout_builder#calc_row(a:config, l:height)
  let l:col = clap#ui#layout_builder#calc_col(a:config, l:width)

  let l:layout = {
        \ 'row': l:row,
        \ 'col': l:col,
        \ 'width': l:width,
        \ 'height': l:height,
        \ 'relative': 'editor',
        \ 'style': 'minimal',
        \ }

  " Add border if configured
  if has('nvim-0.5') && g:clap_popup_border !=? 'nil'
    let l:layout.border = g:clap_popup_border
  endif

  let s:current_layout = l:layout
  return l:layout
endfunction

" Build layout from a preset name.
function! clap#ui#layout_builder#from_preset(preset_name) abort
  let l:preset = get(s:presets, a:preset_name, s:presets.center)
  return clap#ui#layout_builder#build(l:preset)
endfunction

" Get the current layout.
function! clap#ui#layout_builder#current() abort
  return s:current_layout
endfunction

" ============================================================================
" Dynamic Height Support
" ============================================================================

let s:dynamic_height_enabled = get(g:, 'clap_dynamic_height', 0)

" Calculate dynamic height based on result count.
function! clap#ui#layout_builder#dynamic_height(result_count) abort
  if !s:dynamic_height_enabled
    return s:current_layout.height
  endif

  let l:min_height = get(g:, 'clap_dynamic_height_min', 3)
  let l:max_height = s:current_layout.height

  " Add 2 for input row and some padding
  let l:desired = a:result_count + 2

  return max([l:min_height, min([l:max_height, l:desired])])
endfunction

" Check if dynamic height is enabled.
function! clap#ui#layout_builder#is_dynamic_height() abort
  return s:dynamic_height_enabled
endfunction

" Toggle dynamic height.
function! clap#ui#layout_builder#toggle_dynamic_height() abort
  let s:dynamic_height_enabled = !s:dynamic_height_enabled
endfunction

" ============================================================================
" Sub-Window Layout
" ============================================================================

" Calculate spinner window config relative to display.
function! clap#ui#layout_builder#spinner_config(display_config, spinner_width) abort
  let l:config = copy(a:display_config)
  let l:symbol_width = strdisplaywidth(g:__clap_search_box_border_symbol.right)

  let l:config.col += l:symbol_width
  let l:config.row -= 1
  let l:config.width = a:spinner_width
  let l:config.height = 1
  let l:config.focusable = v:false

  if has('nvim-0.5')
    let l:config.zindex = 1000
  endif

  return l:config
endfunction

" Calculate input window config relative to spinner.
function! clap#ui#layout_builder#input_config(spinner_config, indicator_width) abort
  let l:config = copy(a:spinner_config)
  let l:symbol_width = strdisplaywidth(g:__clap_search_box_border_symbol.right)

  let l:config.col += a:spinner_config.width
  let l:config.width = s:current_layout.width - a:spinner_config.width - l:symbol_width * 2 - a:indicator_width
  let l:config.focusable = v:true

  " Ensure minimum width
  if l:config.width < 1
    let l:config.width = 1
  endif

  return l:config
endfunction

" Calculate indicator window config relative to input.
function! clap#ui#layout_builder#indicator_config(input_config, indicator_width) abort
  let l:config = copy(a:input_config)

  let l:config.col += a:input_config.width
  let l:config.width = a:indicator_width
  let l:config.focusable = v:false

  return l:config
endfunction

" Calculate preview window config relative to display.
function! clap#ui#layout_builder#preview_config(display_config, preview_height) abort
  let l:direction = clap#preview#direction()
  let l:config = copy(a:display_config)

  if l:direction ==# 'LR'
    let l:config.row -= 1
    let l:config.col += l:config.width
    let l:config.height += 1
  else  " 'UD'
    let l:config.row += l:config.height
    let l:config.height = a:preview_height
  endif

  let l:config.style = 'minimal'

  " Add border
  if has('nvim-0.5') && g:clap_popup_border !=? 'nil'
    let l:config.border = g:clap_popup_border
    if l:direction ==# 'UD'
      let l:config.width -= 2
    else
      let l:config.height -= 2
    endif
  endif

  return l:config
endfunction

" ============================================================================
" Layout Presets Management
" ============================================================================

" Register a custom preset.
function! clap#ui#layout_builder#register_preset(name, config) abort
  let s:presets[a:name] = a:config
endfunction

" Get available preset names.
function! clap#ui#layout_builder#list_presets() abort
  return keys(s:presets)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
