" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Centralized theme utilities for color extraction and highlight management.

let s:save_cpo = &cpoptions
set cpoptions&vim

" ============================================================================
" Default Color Constants
" ============================================================================

" These are the fallback colors used when highlight groups don't provide values.
" Organized by semantic meaning for easier maintenance.

let g:clap#theme_utils#defaults = {
      \ 'input_bg': { 'gui': '#544a65', 'cterm': '60' },
      \ 'display_bg': { 'gui': '#3E4452', 'cterm': '237' },
      \ 'display_bg_light': { 'gui': '#ecf5ff', 'cterm': '7' },
      \ 'preview_bg': { 'gui': '#5e5079', 'cterm': '60' },
      \ 'normal_fg': { 'gui': '#b2b2b2', 'cterm': '249' },
      \ 'function_fg': { 'gui': '#bc6ec5', 'cterm': '170' },
      \ 'shadow_bg': { 'gui': '#000000', 'cterm': '0' },
      \ 'scrollbar_fg': { 'gui': '#e18254', 'cterm': '173' },
      \ 'selected_fg': { 'gui': '#5fd7d7', 'cterm': '80' },
      \ 'current_selection_fg': { 'gui': '#ffd7d7', 'cterm': '224' },
      \ }

" ============================================================================
" Color Extraction Functions
" ============================================================================

" Extract a single attribute from a highlight group.
" Returns empty string if not found.
function! clap#theme_utils#extract(group, what, gui_or_cterm) abort
  return synIDattr(synIDtrans(hlID(a:group)), a:what, a:gui_or_cterm)
endfunction

" Extract an attribute with a fallback default value.
function! clap#theme_utils#extract_or(group, what, gui_or_cterm, default) abort
  let l:value = clap#theme_utils#extract(a:group, a:what, a:gui_or_cterm)
  return empty(l:value) ? a:default : l:value
endfunction

" Extract both gui and cterm colors for an attribute.
" Returns a dict: { 'gui': ..., 'cterm': ... }
function! clap#theme_utils#extract_both(group, what, defaults) abort
  return {
        \ 'gui': clap#theme_utils#extract_or(a:group, a:what, 'gui', a:defaults.gui),
        \ 'cterm': clap#theme_utils#extract_or(a:group, a:what, 'cterm', a:defaults.cterm),
        \ }
endfunction

" Extract background colors from a highlight group.
function! clap#theme_utils#extract_bg(group, defaults) abort
  return clap#theme_utils#extract_both(a:group, 'bg', a:defaults)
endfunction

" Extract foreground colors from a highlight group.
function! clap#theme_utils#extract_fg(group, defaults) abort
  return clap#theme_utils#extract_both(a:group, 'fg', a:defaults)
endfunction

" ============================================================================
" Highlight Group Creation
" ============================================================================

" Create a highlight group with full color specification.
" Props is a dict that can include:
"   - guifg, guibg, ctermfg, ctermbg
"   - gui, cterm (for attributes like bold, italic, etc.)
function! clap#theme_utils#highlight(group, props) abort
  let l:parts = []
  for [l:key, l:val] in items(a:props)
    if !empty(l:val)
      call add(l:parts, l:key . '=' . l:val)
    endif
  endfor
  if !empty(l:parts)
    execute 'hi' a:group join(l:parts, ' ')
  endif
endfunction

" Create a highlight group using fg/bg dicts from extract_both().
" Optional attrs can be 'bold', 'italic', 'bold,underline', etc.
function! clap#theme_utils#highlight_with_colors(group, fg, bg, ...) abort
  let l:attrs = a:0 > 0 ? a:1 : ''
  let l:props = {}

  if type(a:fg) == v:t_dict
    let l:props.guifg = a:fg.gui
    let l:props.ctermfg = a:fg.cterm
  endif

  if type(a:bg) == v:t_dict
    let l:props.guibg = a:bg.gui
    let l:props.ctermbg = a:bg.cterm
  endif

  if !empty(l:attrs)
    let l:props.gui = l:attrs
    let l:props.cterm = l:attrs
  endif

  call clap#theme_utils#highlight(a:group, l:props)
endfunction

" Create a highlight group that syncs with another group's colors.
" Useful for spinner, symbols, etc. that match input/display backgrounds.
function! clap#theme_utils#highlight_synced(group, fg_source, bg_source, fg_attr, bg_attr, ...) abort
  let l:attrs = a:0 > 0 ? a:1 : ''
  let l:fg_defaults = get(g:clap#theme_utils#defaults, a:fg_attr, g:clap#theme_utils#defaults.normal_fg)
  let l:bg_defaults = get(g:clap#theme_utils#defaults, a:bg_attr, g:clap#theme_utils#defaults.input_bg)

  let l:fg = clap#theme_utils#extract_fg(a:fg_source, l:fg_defaults)
  let l:bg = clap#theme_utils#extract_bg(a:bg_source, l:bg_defaults)

  call clap#theme_utils#highlight_with_colors(a:group, l:fg, l:bg, l:attrs)
endfunction

" ============================================================================
" Invisible EndOfBuffer Highlights
" ============================================================================

" Make EndOfBuffer invisible by setting fg to match bg.
function! clap#theme_utils#make_eob_invisible(eob_group, source_group, default_key) abort
  let l:defaults = get(g:clap#theme_utils#defaults, a:default_key, g:clap#theme_utils#defaults.display_bg)
  let l:bg = clap#theme_utils#extract_bg(a:source_group, l:defaults)
  call clap#theme_utils#highlight(a:eob_group, {
        \ 'guifg': l:bg.gui,
        \ 'ctermfg': l:bg.cterm,
        \ })
endfunction

" ============================================================================
" Window Highlight Strings
" ============================================================================

" Generate winhl string for a window with specific highlight groups.
function! clap#theme_utils#winhl(normal, eob, ...) abort
  let l:winhl = 'Normal:' . a:normal . ',EndOfBuffer:' . a:eob
  let l:winhl .= ',SignColumn:' . a:normal . ',ColorColumn:' . a:normal
  if a:0 > 0
    for [l:key, l:val] in items(a:1)
      let l:winhl .= ',' . l:key . ':' . l:val
    endfor
  endif
  return l:winhl
endfunction

" Pre-defined winhl strings for common window types.
let g:clap#theme_utils#winhl = {
      \ 'shadow': 'Normal:ClapShadow,NormalNC:ClapShadow,EndOfBuffer:ClapShadow',
      \ 'display': clap#theme_utils#winhl('ClapDisplay', 'ClapDisplayInvisibleEndOfBuffer'),
      \ 'preview': clap#theme_utils#winhl('ClapPreview', 'ClapPreviewInvisibleEndOfBuffer'),
      \ }

" ============================================================================
" Background-aware Colors
" ============================================================================

" Get appropriate colors based on &background setting.
function! clap#theme_utils#background_aware(dark_value, light_value) abort
  return &background ==# 'dark' ? a:dark_value : a:light_value
endfunction

" Get display background colors based on &background.
function! clap#theme_utils#display_bg() abort
  return clap#theme_utils#background_aware(
        \ g:clap#theme_utils#defaults.display_bg,
        \ g:clap#theme_utils#defaults.display_bg_light)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
