" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Initialize the clap theme and provide theme utilities.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')

let s:input_default_hi_group = 'Visual'
let s:display_default_hi_group = 'ClapDefaultPreview'
let s:preview_default_hi_group = 'PmenuSel'

" ============================================================================
" Default Color Constants
" ============================================================================

" These are the fallback colors used when highlight groups don't provide values.
" Organized by semantic meaning for easier maintenance.

let g:clap#themes#defaults = {
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

let s:defaults = g:clap#themes#defaults

" ============================================================================
" Color Extraction Functions
" ============================================================================

" Extract a single attribute from a highlight group.
" Returns empty string if not found.
function! clap#themes#extract(group, what, gui_or_cterm) abort
  return synIDattr(synIDtrans(hlID(a:group)), a:what, a:gui_or_cterm)
endfunction

" Extract an attribute with a fallback default value.
function! clap#themes#extract_or(group, what, gui_or_cterm, default) abort
  let l:value = clap#themes#extract(a:group, a:what, a:gui_or_cterm)
  return empty(l:value) ? a:default : l:value
endfunction

" Extract both gui and cterm colors for an attribute.
" Returns a dict: { 'gui': ..., 'cterm': ... }
function! clap#themes#extract_both(group, what, defaults) abort
  return {
        \ 'gui': clap#themes#extract_or(a:group, a:what, 'gui', a:defaults.gui),
        \ 'cterm': clap#themes#extract_or(a:group, a:what, 'cterm', a:defaults.cterm),
        \ }
endfunction

" Extract background colors from a highlight group.
function! clap#themes#extract_bg(group, defaults) abort
  return clap#themes#extract_both(a:group, 'bg', a:defaults)
endfunction

" Extract foreground colors from a highlight group.
function! clap#themes#extract_fg(group, defaults) abort
  return clap#themes#extract_both(a:group, 'fg', a:defaults)
endfunction

" ============================================================================
" Highlight Group Creation
" ============================================================================

" Create a highlight group with full color specification.
" Props is a dict that can include:
"   - guifg, guibg, ctermfg, ctermbg
"   - gui, cterm (for attributes like bold, italic, etc.)
function! clap#themes#highlight(group, props) abort
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
function! clap#themes#highlight_with_colors(group, fg, bg, ...) abort
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

  call clap#themes#highlight(a:group, l:props)
endfunction

" ============================================================================
" Invisible EndOfBuffer Highlights
" ============================================================================

" Make EndOfBuffer invisible by setting fg to match bg.
function! clap#themes#make_eob_invisible(eob_group, source_group, default_key) abort
  let l:defaults = get(g:clap#themes#defaults, a:default_key, g:clap#themes#defaults.display_bg)
  let l:bg = clap#themes#extract_bg(a:source_group, l:defaults)
  call clap#themes#highlight(a:eob_group, {
        \ 'guifg': l:bg.gui,
        \ 'ctermfg': l:bg.cterm,
        \ })
endfunction

" ============================================================================
" Window Highlight Strings
" ============================================================================

" Generate winhl string for a window with specific highlight groups.
function! clap#themes#winhl(normal, eob, ...) abort
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
let g:clap#themes#winhl = {
      \ 'shadow': 'Normal:ClapShadow,NormalNC:ClapShadow,EndOfBuffer:ClapShadow',
      \ 'display': clap#themes#winhl('ClapDisplay', 'ClapDisplayInvisibleEndOfBuffer'),
      \ 'preview': clap#themes#winhl('ClapPreview', 'ClapPreviewInvisibleEndOfBuffer'),
      \ }

" ============================================================================
" Background-aware Colors
" ============================================================================

" Get appropriate colors based on &background setting.
function! clap#themes#background_aware(dark_value, light_value) abort
  return &background ==# 'dark' ? a:dark_value : a:light_value
endfunction

" Get display background colors based on &background.
function! clap#themes#display_bg() abort
  return clap#themes#background_aware(
        \ g:clap#themes#defaults.display_bg,
        \ g:clap#themes#defaults.display_bg_light)
endfunction

" ============================================================================
" Theme Initialization Functions
" ============================================================================

" Try to sync the spinner bg with input window.
function! s:hi_spinner() abort
  let l:bg = clap#themes#extract_bg(s:input_default_hi_group, s:defaults.input_bg)
  let l:fg = clap#themes#extract_fg('Function', s:defaults.function_fg)

  call clap#themes#highlight_with_colors('ClapSpinner', l:fg, l:bg, 'bold')
endfunction

function! s:hi_clap_symbol() abort
  " Symbol uses input bg as fg, and normal bg as bg (creating contrast)
  let l:input_bg = clap#themes#extract_bg('ClapInput', s:defaults.input_bg)
  let l:normal_bg = clap#themes#extract_bg('Normal', s:defaults.normal_fg)

  " Swap: input_bg becomes fg, normal_bg becomes bg
  call clap#themes#highlight('ClapSymbol', {
        \ 'guifg': l:input_bg.gui,
        \ 'ctermfg': l:input_bg.cterm,
        \ 'guibg': l:normal_bg.gui,
        \ 'ctermbg': l:normal_bg.cterm,
        \ })
endfunction

function! s:hi_clap_float_title() abort
  let l:bg = clap#themes#extract_bg('ClapPreview', s:defaults.input_bg)
  let l:fg = clap#themes#extract_fg('Title', s:defaults.function_fg)

  call clap#themes#highlight_with_colors('FloatTitle', l:fg, l:bg)
endfunction

" Try the palette, otherwise use the built-in material_design_dark theme.
function! s:highlight_for(group_name, type) abort
  if has_key(s:palette, a:type)
    let props = s:palette[a:type]
  " The exception seems to be silented here.
  elseif has_key(g:clap#themes#material_design_dark#palette, a:type)
    let props = g:clap#themes#material_design_dark#palette[a:type]
  else
    return
  endif
  execute 'hi default' a:group_name join(values(map(copy(props), 'v:key."=".v:val')), ' ')
endfunction

function! s:paint_is_ok() abort
  try
    call s:highlight_for('ClapSpinner', 'spinner')
    " Backward compatible
    if hlexists('ClapQuery')
      hi link ClapSearchText ClapQuery
    else
      call s:highlight_for('ClapSearchText', 'search_text')
    endif
    call s:highlight_for('ClapInput', 'input')
    call s:highlight_for('ClapDisplay', 'display')
    call s:highlight_for('ClapIndicator', 'indicator')
    call s:highlight_for('ClapSelected', 'selected')
    call s:highlight_for('ClapCurrentSelection', 'current_selection')
    call s:highlight_for('ClapSelectedSign', 'selected_sign')
    call s:highlight_for('ClapCurrentSelectionSign', 'current_selection_sign')
    call s:highlight_for('ClapPreview', 'preview')
  catch
    return v:false
  endtry
  return v:true
endfunction

function! s:apply_default_theme() abort
  if !hlexists('ClapSpinner')
    call s:hi_spinner()
    augroup ClapRefreshSpinner
      autocmd!
      autocmd ColorScheme * call s:hi_spinner()
    augroup END
  endif

  if !hlexists('ClapSearchText')
    let l:bg = clap#themes#extract_bg(s:input_default_hi_group, s:defaults.input_bg)
    let l:fg = clap#themes#extract_fg('Normal', s:defaults.normal_fg)

    call clap#themes#highlight_with_colors('ClapSearchText', l:fg, l:bg, 'bold')
  endif

  " Default selection highlights using centralized colors
  call clap#themes#highlight('ClapDefaultSelected', {
        \ 'guifg': s:defaults.selected_fg.gui,
        \ 'ctermfg': s:defaults.selected_fg.cterm,
        \ 'gui': 'bold,underline',
        \ 'cterm': 'bold,underline',
        \ })

  call clap#themes#highlight('ClapDefaultCurrentSelection', {
        \ 'guifg': s:defaults.current_selection_fg.gui,
        \ 'ctermfg': s:defaults.current_selection_fg.cterm,
        \ 'gui': 'bold',
        \ 'cterm': 'bold',
        \ })

  hi default link ClapPreview ClapDefaultPreview
  hi default link ClapSelected ClapDefaultSelected
  hi default link ClapCurrentSelection ClapDefaultCurrentSelection
  hi default link ClapSelectedSign WarningMsg
  hi default link ClapCurrentSelectionSign WarningMsg

  execute 'hi default link ClapInput' s:input_default_hi_group
  execute 'hi default link ClapDisplay' s:display_default_hi_group
  hi default link ClapIndicator ClapInput
endfunction

function! s:make_display_EndOfBuffer_invisible() abort
  let l:display_group = hlexists('ClapDisplay') ? 'ClapDisplay' : s:display_default_hi_group
  call clap#themes#make_eob_invisible('ClapDisplayInvisibleEndOfBuffer', l:display_group, 'input_bg')
endfunction

function! s:make_preview_EndOfBuffer_invisible() abort
  let l:preview_group = hlexists('ClapPreview') ? 'ClapPreview' : 'ClapDefaultPreview'
  call clap#themes#make_eob_invisible('ClapPreviewInvisibleEndOfBuffer', l:preview_group, 'preview_bg')
endfunction

function! s:reverse_PopupCursor() abort
  if !hlexists('ClapSearchText')
    return
  endif
  let l:bg = clap#themes#extract_bg('ClapSearchText', s:defaults.input_bg)
  let l:fg = clap#themes#extract_fg('ClapSearchText', s:defaults.normal_fg)

  call clap#themes#highlight_with_colors('ClapPopupCursor', l:fg, l:bg, 'bold,reverse')
endfunction

function! s:init_theme() abort
  call clap#themes#highlight('ClapDefaultShadow', { 'guibg': s:defaults.shadow_bg.gui })
  hi default link ClapShadow ClapDefaultShadow
  hi default link FloatBorder ClapPreview

  " Background-aware preview colors
  let l:preview_bg = clap#themes#display_bg()
  call clap#themes#highlight('ClapDefaultPreview', {
        \ 'guibg': l:preview_bg.gui,
        \ 'ctermbg': l:preview_bg.cterm,
        \ })

  " Path prefix dimming - always define these (regardless of theme palette)
  if &background ==# 'dark'
    call clap#themes#highlight('ClapDefaultPathPrefix', {
          \ 'guifg': '#6c7086',
          \ 'ctermfg': '242',
          \ })
    call clap#themes#highlight('ClapDefaultFileName', {
          \ 'guifg': '#cdd6f4',
          \ 'ctermfg': '255',
          \ })
  else
    call clap#themes#highlight('ClapDefaultPathPrefix', {
          \ 'guifg': '#9399b2',
          \ 'ctermfg': '246',
          \ })
    call clap#themes#highlight('ClapDefaultFileName', {
          \ 'guifg': '#4c4f69',
          \ 'ctermfg': '239',
          \ })
  endif
  hi default link ClapPathPrefix ClapDefaultPathPrefix
  hi default link ClapFileName ClapDefaultFileName

  if !exists('s:palette') || !s:paint_is_ok()
    call s:apply_default_theme()
  endif

  if !s:is_nvim && get(g:, 'clap_popup_cursor_shape', '') ==# ''
    " block cursor
    call s:reverse_PopupCursor()
  endif

  call s:hi_clap_symbol()
  call s:hi_clap_float_title()
  call s:make_display_EndOfBuffer_invisible()
  call s:make_preview_EndOfBuffer_invisible()
  call clap#icon#define_normal_color_components()
endfunction

function! clap#themes#init() abort
  hi default link ClapMatches        Search
  hi default link ClapNoMatchesFound ErrorMsg
  hi default link ClapPopupCursor    Type

  if exists('g:clap_theme')
    " If anything is wrong, just use the default theme.
    if type(g:clap_theme) == v:t_string
      try
        let s:palette = g:clap#themes#{g:clap_theme}#palette
      catch
        let s:palette = g:clap#themes#material_design_dark#palette
      endtry
    elseif type(g:clap_theme) == v:t_dict
      let s:palette = g:clap_theme
    else
      let s:palette = g:clap#themes#material_design_dark#palette
    endif
  elseif exists('g:colors_name')
    try
      let s:palette = g:clap#themes#{g:colors_name}#palette
    catch
    endtry
  endif

  call s:init_theme()

  augroup ClapReloadTheme
    autocmd!
    autocmd ColorScheme * call s:init_theme()
  augroup END
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
