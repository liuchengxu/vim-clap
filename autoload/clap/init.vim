" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Initialize the plugin, including making a compatible API layer
" and flexiable highlight groups.

let s:save_cpo = &cpo
set cpo&vim

let s:input_default_hi_group = 'Visual'
let s:display_default_hi_group = 'Pmenu'
let s:preview_defaualt_hi_group = 'PmenuSel'

function! s:extract(group, what, gui_or_cterm) abort
  return synIDattr(synIDtrans(hlID(a:group)), a:what, a:gui_or_cterm)
endfunction

function! s:extract_or(group, what, gui_or_cterm, default) abort
  let v = s:extract(a:group, a:what, a:gui_or_cterm)
  if empty(v)
    return a:default
  endif
  return v
endfunction

function! s:hi_display_invisible() abort
  " People can use their own display highlight group, so can't use s:display_default_hi_group here.
  let guibg = s:extract_or(s:display_group, 'bg', 'gui', '#544a65')
  let ctermbg = s:extract_or(s:display_group, 'bg', 'cterm', 60)
  execute printf(
        \ "hi ClapDisplayInvisibleEndOfBuffer ctermfg=%s guifg=%s",
        \ ctermbg,
        \ guibg
        \ )
endfunction

function! s:hi_preview_invisible() abort
  let guibg = s:extract_or(s:preview_group, 'bg', 'gui', '#5e5079')
  let ctermbg = s:extract_or(s:preview_group, 'bg', 'cterm', '60')
  execute printf(
        \ "hi ClapPreviewInvisibleEndOfBuffer ctermfg=%s guifg=%s",
        \ ctermbg,
        \ guibg
        \ )
endfunction

" Try to sync the spinner bg with input window.
function! s:hi_spinner() abort
  let vis_ctermbg = s:extract_or(s:input_default_hi_group, 'bg', 'cterm', '60')
  let vis_guibg = s:extract_or(s:input_default_hi_group, 'bg', 'gui', '#544a65')
  let fn_ctermfg = s:extract_or('Function', 'fg', 'cterm', '170')
  let fn_guifg = s:extract_or('Function', 'fg', 'gui', '#bc6ec5')
  execute printf(
        \ "hi ClapSpinner guifg=%s ctermfg=%s ctermbg=%s guibg=%s gui=bold cterm=bold",
        \ fn_guifg,
        \ fn_ctermfg,
        \ vis_ctermbg,
        \ vis_guibg,
        \ )
endfunction

function! s:init_hi_submatches() abort
  let clap_sub_matches = [
        \ [173 , '#e18254'] ,
        \ [196 , '#f2241f'] ,
        \ [184 , '#e5d11c'] ,
        \ [32  , '#4f97d7'] ,
        \ [170 , '#bc6ec5'] ,
        \ [178 , '#ffbb7d'] ,
        \ [136 , '#b1951d'] ,
        \ [29  , '#2d9574'] ,
        \ ]

  let pmenu_ctermbg = s:extract_or(s:display_default_hi_group, 'bg', 'cterm', '60')
  let pmenu_guibg = s:extract_or(s:display_default_hi_group, 'bg', 'gui', '#544a65')

  let idx = 1
  for g in clap_sub_matches
    execute printf(
          \ "hi ClapMatches%s guifg=%s ctermfg=%s ctermbg=%s guibg=%s gui=bold cterm=bold", idx,
          \ g[1],
          \ g[0],
          \ pmenu_ctermbg,
          \ pmenu_guibg,
          \ )
    let idx += 1
  endfor
endfunction

function! s:ensure_hl_exists(group, default) abort
  if !hlexists(a:group)
    execute 'hi default link' a:group a:default
  endif
endfunction

function! s:init_hi_groups() abort
  if !hlexists('ClapSpinner')
    call s:hi_spinner()
    autocmd ColorScheme * call s:hi_spinner()
  endif

  call s:ensure_hl_exists('ClapInput', s:input_default_hi_group)
  if !hlexists('ClapQuery')
    " A bit repeatation code here in case of ClapSpinner is defined explicitly.
    let vis_ctermbg = s:extract_or(s:input_default_hi_group, 'bg', 'cterm', '60')
    let vis_guibg = s:extract_or(s:input_default_hi_group, 'bg', 'gui', '#544a65')
    let ident_ctermfg = s:extract_or('Normal', 'fg', 'cterm', '249')
    let ident_guifg = s:extract_or('Normal', 'fg', 'gui', '#b2b2b2')
    execute printf(
          \ "hi ClapQuery guifg=%s ctermfg=%s ctermbg=%s guibg=%s cterm=bold gui=bold",
          \ ident_guifg,
          \ ident_ctermfg,
          \ vis_ctermbg,
          \ vis_guibg,
          \ )
  endif

  if !hlexists('ClapDisplay')
    execute 'hi default link ClapDisplay' s:display_default_hi_group
    let s:display_group = s:display_default_hi_group
  else
    let s:display_group = 'ClapDisplay'
  endif

  call s:hi_display_invisible()
  autocmd ColorScheme * call s:hi_display_invisible()

  hi ClapDefaultPreview ctermbg=237 guibg=#3E4452

  if !hlexists('ClapPreview')
    hi default link ClapPreview ClapDefaultPreview
    let s:preview_group = 'ClapDefaultPreview'
  else
    let s:preview_group = 'ClapPreview'
  endif
  call s:hi_preview_invisible()
  autocmd ColorScheme * call s:hi_preview_invisible()

  " For the found matches highlight
  call s:ensure_hl_exists('ClapMatches', 'Search')
  call s:ensure_hl_exists('ClapNoMatchesFound', 'ErrorMsg')

  hi ClapDefaultSelected         cterm=bold,underline gui=bold,underline ctermfg=80 guifg=#5fd7d7
  hi ClapDefaultCurrentSelection cterm=bold gui=bold ctermfg=224 guifg=#ffd7d7

  call s:ensure_hl_exists('ClapSelected', 'ClapDefaultSelected')
  call s:ensure_hl_exists('ClapCurrentSelection', 'ClapDefaultCurrentSelection')

  call s:init_hi_submatches()
endfunction

function! clap#init#() abort
  call clap#api#bake()
  call s:init_hi_groups()
endfunction

let &cpo = s:save_cpo
unlet s:save_cpo
