" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Initialize the clap theme.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')

let s:input_default_hi_group = 'Visual'
let s:display_default_hi_group = 'Pmenu'
let s:preview_default_hi_group = 'PmenuSel'

function! s:extract(group, what, gui_or_cterm) abort
  return synIDattr(synIDtrans(hlID(a:group)), a:what, a:gui_or_cterm)
endfunction

function! s:extract_or(group, what, gui_or_cterm, default) abort
  let v = s:extract(a:group, a:what, a:gui_or_cterm)
  return empty(v) ? a:default : v
endfunction

" Try to sync the spinner bg with input window.
function! s:hi_spinner() abort
  let vis_ctermbg = s:extract_or(s:input_default_hi_group, 'bg', 'cterm', '60')
  let vis_guibg = s:extract_or(s:input_default_hi_group, 'bg', 'gui', '#544a65')
  let fn_ctermfg = s:extract_or('Function', 'fg', 'cterm', '170')
  let fn_guifg = s:extract_or('Function', 'fg', 'gui', '#bc6ec5')

  execute printf(
        \ 'hi ClapSpinner guifg=%s ctermfg=%s ctermbg=%s guibg=%s gui=bold cterm=bold',
        \ fn_guifg,
        \ fn_ctermfg,
        \ vis_ctermbg,
        \ vis_guibg,
        \ )
endfunction

function! s:hi_clap_symbol() abort
  let input_ctermbg = s:extract_or('ClapInput', 'bg', 'cterm', '60')
  let input_guibg = s:extract_or('ClapInput', 'bg', 'gui', '#544a65')
  let normal_ctermfg = s:extract_or('Normal', 'bg', 'cterm', '249')
  let normal_guifg = s:extract_or('Normal', 'bg', 'gui', '#b2b2b2')
  execute printf(
        \ 'hi ClapSymbol guifg=%s ctermfg=%s ctermbg=%s guibg=%s',
        \ input_guibg,
        \ input_ctermbg,
        \ normal_ctermfg,
        \ normal_guifg,
        \ )
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
  execute 'hi' a:group_name join(values(map(copy(props), 'v:key."=".v:val')), ' ')
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
    " A bit repeatation code here in case of ClapSpinner is defined explicitly.
    let vis_ctermbg = s:extract_or(s:input_default_hi_group, 'bg', 'cterm', '60')
    let vis_guibg = s:extract_or(s:input_default_hi_group, 'bg', 'gui', '#544a65')
    let ident_ctermfg = s:extract_or('Normal', 'fg', 'cterm', '249')
    let ident_guifg = s:extract_or('Normal', 'fg', 'gui', '#b2b2b2')
    execute printf(
          \ 'hi ClapSearchText guifg=%s ctermfg=%s ctermbg=%s guibg=%s cterm=bold gui=bold',
          \ ident_guifg,
          \ ident_ctermfg,
          \ vis_ctermbg,
          \ vis_guibg,
          \ )
  endif

  hi ClapDefaultSelected         ctermfg=80  guifg=#5fd7d7 cterm=bold,underline gui=bold,underline
  hi ClapDefaultCurrentSelection ctermfg=224 guifg=#ffd7d7 cterm=bold gui=bold

  hi default link ClapPreview ClapDefaultPreview
  hi default link ClapSelected ClapDefaultSelected
  hi default link ClapCurrentSelection ClapDefaultCurrentSelection
  hi default link ClapSelectedSign WarningMsg
  hi default link ClapCurrentSelectionSign WarningMsg

  execute 'hi default link ClapInput' s:input_default_hi_group
  execute 'hi default link ClapDisplay' s:display_default_hi_group
endfunction

function! s:make_display_EndOfBuffer_invisible() abort
  let display_group = hlexists('ClapDisplay') ? 'ClapDisplay' : s:display_default_hi_group
  " People can use their own display highlight group, so can't use s:display_default_hi_group here.
  let guibg = s:extract_or(display_group, 'bg', 'gui', '#544a65')
  let ctermbg = s:extract_or(display_group, 'bg', 'cterm', '60')
  execute printf(
        \ 'hi ClapDisplayInvisibleEndOfBuffer ctermfg=%s guifg=%s',
        \ ctermbg,
        \ guibg
        \ )
endfunction

function! s:make_preview_EndOfBuffer_invisible() abort
  let preview_group = hlexists('ClapPreview') ? 'ClapPreview' : 'ClapDefaultPreview'
  let guibg = s:extract_or(preview_group, 'bg', 'gui', '#5e5079')
  let ctermbg = s:extract_or(preview_group, 'bg', 'cterm', '60')
  execute printf(
        \ 'hi ClapPreviewInvisibleEndOfBuffer ctermfg=%s guifg=%s',
        \ ctermbg,
        \ guibg
        \ )
endfunction

function! s:init_theme() abort
  if &background ==# 'dark'
    hi ClapDefaultPreview ctermbg=237 guibg=#3E4452
  else
    hi ClapDefaultPreview ctermbg=7 guibg=#ecf5ff
  endif

  if !exists('s:palette') || !s:paint_is_ok()
    call s:apply_default_theme()
  endif

  call s:hi_clap_symbol()
  call s:make_display_EndOfBuffer_invisible()
  call s:make_preview_EndOfBuffer_invisible()
  call clap#icon#def_color_components()
endfunction

function! clap#themes#init() abort
  hi default link ClapMatches Search
  hi default link ClapNoMatchesFound ErrorMsg
  hi default link ClapPopupCursor Type

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
  endif

  call s:init_theme()

  augroup ClapReloadTheme
    autocmd!
    autocmd ColorScheme * call s:init_theme()
  augroup END
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
