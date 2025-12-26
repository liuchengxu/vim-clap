" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Initialize the clap theme.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')

let s:input_default_hi_group = 'Visual'
let s:display_default_hi_group = 'ClapDefaultPreview'
let s:preview_default_hi_group = 'PmenuSel'

" Use centralized theme utilities
let s:defaults = g:clap#theme_utils#defaults

" Shorthand for theme_utils functions
function! s:extract_or(group, what, gui_or_cterm, default) abort
  return clap#theme_utils#extract_or(a:group, a:what, a:gui_or_cterm, a:default)
endfunction

" Try to sync the spinner bg with input window.
function! s:hi_spinner() abort
  let l:bg = clap#theme_utils#extract_bg(s:input_default_hi_group, s:defaults.input_bg)
  let l:fg = clap#theme_utils#extract_fg('Function', s:defaults.function_fg)

  call clap#theme_utils#highlight_with_colors('ClapSpinner', l:fg, l:bg, 'bold')
endfunction

function! s:hi_clap_symbol() abort
  " Symbol uses input bg as fg, and normal bg as bg (creating contrast)
  let l:input_bg = clap#theme_utils#extract_bg('ClapInput', s:defaults.input_bg)
  let l:normal_bg = clap#theme_utils#extract_bg('Normal', s:defaults.normal_fg)

  " Swap: input_bg becomes fg, normal_bg becomes bg
  call clap#theme_utils#highlight('ClapSymbol', {
        \ 'guifg': l:input_bg.gui,
        \ 'ctermfg': l:input_bg.cterm,
        \ 'guibg': l:normal_bg.gui,
        \ 'ctermbg': l:normal_bg.cterm,
        \ })
endfunction

function! s:hi_clap_float_title() abort
  let l:bg = clap#theme_utils#extract_bg('ClapPreview', s:defaults.input_bg)
  let l:fg = clap#theme_utils#extract_fg('Title', s:defaults.function_fg)

  call clap#theme_utils#highlight_with_colors('FloatTitle', l:fg, l:bg)
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
    let l:bg = clap#theme_utils#extract_bg(s:input_default_hi_group, s:defaults.input_bg)
    let l:fg = clap#theme_utils#extract_fg('Normal', s:defaults.normal_fg)

    call clap#theme_utils#highlight_with_colors('ClapSearchText', l:fg, l:bg, 'bold')
  endif

  " Default selection highlights with improved visibility
  " Multi-selected items: subtle underline with teal color
  call clap#theme_utils#highlight('ClapDefaultSelected', {
        \ 'guifg': s:defaults.selected_fg.gui,
        \ 'ctermfg': s:defaults.selected_fg.cterm,
        \ 'gui': 'bold,underline',
        \ 'cterm': 'bold,underline',
        \ })

  " Current selection: visible background with bright text
  " Uses a subtle background highlight for better visibility
  if &background ==# 'dark'
    call clap#theme_utils#highlight('ClapDefaultCurrentSelection', {
          \ 'guifg': '#f5e0dc',
          \ 'ctermfg': '224',
          \ 'guibg': '#45475a',
          \ 'ctermbg': '238',
          \ 'gui': 'bold',
          \ 'cterm': 'bold',
          \ })
    " Current selection sign: arrow indicator
    call clap#theme_utils#highlight('ClapDefaultCurrentSelectionSign', {
          \ 'guifg': '#89b4fa',
          \ 'ctermfg': '75',
          \ 'guibg': '#45475a',
          \ 'ctermbg': '238',
          \ 'gui': 'bold',
          \ 'cterm': 'bold',
          \ })
  else
    call clap#theme_utils#highlight('ClapDefaultCurrentSelection', {
          \ 'guifg': '#4c4f69',
          \ 'ctermfg': '239',
          \ 'guibg': '#ccd0da',
          \ 'ctermbg': '252',
          \ 'gui': 'bold',
          \ 'cterm': 'bold',
          \ })
    call clap#theme_utils#highlight('ClapDefaultCurrentSelectionSign', {
          \ 'guifg': '#1e66f5',
          \ 'ctermfg': '27',
          \ 'guibg': '#ccd0da',
          \ 'ctermbg': '252',
          \ 'gui': 'bold',
          \ 'cterm': 'bold',
          \ })
  endif

  hi default link ClapPreview ClapDefaultPreview
  hi default link ClapSelected ClapDefaultSelected
  hi default link ClapCurrentSelection ClapDefaultCurrentSelection
  hi default link ClapSelectedSign WarningMsg
  hi default link ClapCurrentSelectionSign ClapDefaultCurrentSelectionSign

  execute 'hi default link ClapInput' s:input_default_hi_group
  execute 'hi default link ClapDisplay' s:display_default_hi_group
  hi default link ClapIndicator ClapInput

  " Dimmed path prefix styling for better file readability
  " Path prefix (directory) is dimmed, filename is bright
  if &background ==# 'dark'
    call clap#theme_utils#highlight('ClapDefaultPathPrefix', {
          \ 'guifg': '#6e738d',
          \ 'ctermfg': '242',
          \ })
    call clap#theme_utils#highlight('ClapDefaultFileName', {
          \ 'guifg': '#cdd6f4',
          \ 'ctermfg': '255',
          \ 'gui': 'bold',
          \ 'cterm': 'bold',
          \ })
  else
    call clap#theme_utils#highlight('ClapDefaultPathPrefix', {
          \ 'guifg': '#9399b2',
          \ 'ctermfg': '246',
          \ })
    call clap#theme_utils#highlight('ClapDefaultFileName', {
          \ 'guifg': '#4c4f69',
          \ 'ctermfg': '239',
          \ 'gui': 'bold',
          \ 'cterm': 'bold',
          \ })
  endif

  hi default link ClapPathPrefix ClapDefaultPathPrefix
  hi default link ClapFileName ClapDefaultFileName
endfunction

function! s:make_display_EndOfBuffer_invisible() abort
  let l:display_group = hlexists('ClapDisplay') ? 'ClapDisplay' : s:display_default_hi_group
  call clap#theme_utils#make_eob_invisible('ClapDisplayInvisibleEndOfBuffer', l:display_group, 'input_bg')
endfunction

function! s:make_preview_EndOfBuffer_invisible() abort
  let l:preview_group = hlexists('ClapPreview') ? 'ClapPreview' : 'ClapDefaultPreview'
  call clap#theme_utils#make_eob_invisible('ClapPreviewInvisibleEndOfBuffer', l:preview_group, 'preview_bg')
endfunction

function! s:reverse_PopupCursor() abort
  if !hlexists('ClapSearchText')
    return
  endif
  let l:bg = clap#theme_utils#extract_bg('ClapSearchText', s:defaults.input_bg)
  let l:fg = clap#theme_utils#extract_fg('ClapSearchText', s:defaults.normal_fg)

  call clap#theme_utils#highlight_with_colors('ClapPopupCursor', l:fg, l:bg, 'bold,reverse')
endfunction

function! s:init_theme() abort
  call clap#theme_utils#highlight('ClapDefaultShadow', { 'guibg': s:defaults.shadow_bg.gui })
  hi default link ClapShadow ClapDefaultShadow

  " Improved border styling with subtle contrast
  if &background ==# 'dark'
    call clap#theme_utils#highlight('ClapDefaultBorder', {
          \ 'guifg': '#585b70',
          \ 'ctermfg': '240',
          \ 'guibg': 'NONE',
          \ 'ctermbg': 'NONE',
          \ })
    call clap#theme_utils#highlight('ClapDefaultBorderText', {
          \ 'guifg': '#cba6f7',
          \ 'ctermfg': '183',
          \ 'guibg': 'NONE',
          \ 'ctermbg': 'NONE',
          \ 'gui': 'bold',
          \ 'cterm': 'bold',
          \ })
  else
    call clap#theme_utils#highlight('ClapDefaultBorder', {
          \ 'guifg': '#9ca0b0',
          \ 'ctermfg': '248',
          \ 'guibg': 'NONE',
          \ 'ctermbg': 'NONE',
          \ })
    call clap#theme_utils#highlight('ClapDefaultBorderText', {
          \ 'guifg': '#8839ef',
          \ 'ctermfg': '129',
          \ 'guibg': 'NONE',
          \ 'ctermbg': 'NONE',
          \ 'gui': 'bold',
          \ 'cterm': 'bold',
          \ })
  endif

  hi default link ClapBorder ClapDefaultBorder
  hi default link ClapBorderText ClapDefaultBorderText
  hi default link FloatBorder ClapBorder

  " Background-aware preview colors
  let l:preview_bg = clap#theme_utils#display_bg()
  call clap#theme_utils#highlight('ClapDefaultPreview', {
        \ 'guibg': l:preview_bg.gui,
        \ 'ctermbg': l:preview_bg.cterm,
        \ })

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
