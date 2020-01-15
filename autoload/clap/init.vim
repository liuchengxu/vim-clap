" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Initialize the plugin, including making a compatible API layer
" and flexiable highlight groups.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')

let s:input_default_hi_group = 'Visual'
let s:display_default_hi_group = 'Pmenu'
let s:preview_defaualt_hi_group = 'PmenuSel'

function! s:extract(group, what, gui_or_cterm) abort
  return synIDattr(synIDtrans(hlID(a:group)), a:what, a:gui_or_cterm)
endfunction

function! s:extract_or(group, what, gui_or_cterm, default) abort
  let v = s:extract(a:group, a:what, a:gui_or_cterm)
  return empty(v) ? a:default : v
endfunction

function! s:hi_display_invisible() abort
  " People can use their own display highlight group, so can't use s:display_default_hi_group here.
  let guibg = s:extract_or(s:display_group, 'bg', 'gui', '#544a65')
  let ctermbg = s:extract_or(s:display_group, 'bg', 'cterm', '60')
  execute printf(
        \ 'hi ClapDisplayInvisibleEndOfBuffer ctermfg=%s guifg=%s',
        \ ctermbg,
        \ guibg
        \ )
endfunction

function! s:hi_preview_invisible() abort
  let guibg = s:extract_or(s:preview_group, 'bg', 'gui', '#5e5079')
  let ctermbg = s:extract_or(s:preview_group, 'bg', 'cterm', '60')
  execute printf(
        \ 'hi ClapPreviewInvisibleEndOfBuffer ctermfg=%s guifg=%s',
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
        \ 'hi ClapSpinner guifg=%s ctermfg=%s ctermbg=%s guibg=%s gui=bold cterm=bold',
        \ fn_guifg,
        \ fn_ctermfg,
        \ vis_ctermbg,
        \ vis_guibg,
        \ )
endfunction

function! s:or_hi(group_name, cermfg, guifg) abort
  if !hlexists(a:group_name)
    execute printf(
          \ 'hi %s ctermfg=%s guifg=%s ctermbg=%s guibg=%s gui=bold cterm=bold',
          \ a:group_name,
          \ a:cermfg,
          \ a:guifg,
          \ 'NONE',
          \ 'NONE',
          \ )
  endif
endfunction

function! s:init_submatches_hl_group() abort
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

  " idx from 1
  call map(clap_sub_matches, 's:or_hi("ClapMatches".(v:key+1), v:val[0], v:val[1])')
endfunction

function! s:add_fuzzy_match_hl_group(idx, ctermfg, guifg) abort
  let group_name = 'ClapFuzzyMatches'.a:idx
  call s:or_hi(group_name, a:ctermfg, a:guifg)
  if !s:is_nvim
    call prop_type_add(group_name, {'highlight': group_name})
  endif
endfunction

function! s:init_fuzzy_match_hl_groups() abort
  if exists('g:clap_fuzzy_match_hl_groups')
    let clap_fuzzy_matches = g:clap_fuzzy_match_hl_groups
  else
    let clap_fuzzy_matches = [
          \ [118 , '#87ff00'] ,
          \ [82  , '#5fff00'] ,
          \ [46  , '#00ff00'] ,
          \ [47  , '#00ff5f'] ,
          \ [48  , '#00ff87'] ,
          \ [49  , '#00ffaf'] ,
          \ [50  , '#00ffd7'] ,
          \ [51  , '#00ffff'] ,
          \ [87  , '#5fffff'] ,
          \ [123 , '#87ffff'] ,
          \ [159 , '#afffff'] ,
          \ [195 , '#d7ffff'] ,
          \ ]
  endif

  " idx from 1
  call map(clap_fuzzy_matches, 's:add_fuzzy_match_hl_group(v:key+1, v:val[0], v:val[1])')

  let g:__clap_fuzzy_matches_hl_group_cnt = len(clap_fuzzy_matches)
  let g:__clap_fuzzy_last_hl_group = 'ClapFuzzyMatches'.g:__clap_fuzzy_matches_hl_group_cnt
endfunction

function! s:ensure_hl_exists(group, default) abort
  if !hlexists(a:group)
    execute 'hi default link' a:group a:default
  endif
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

function! s:colorschme_adaptive() abort
  call s:hi_display_invisible()
  call s:hi_preview_invisible()
  call s:hi_clap_symbol()
  call clap#icon#def_color_components()
endfunction

function! s:init_hi_groups() abort
  if !hlexists('ClapSpinner')
    call s:hi_spinner()
    augroup ClapRefreshSpinner
      autocmd!
      autocmd ColorScheme * call s:hi_spinner()
    augroup END
  endif

  if !hlexists('ClapQuery')
    " A bit repeatation code here in case of ClapSpinner is defined explicitly.
    let vis_ctermbg = s:extract_or(s:input_default_hi_group, 'bg', 'cterm', '60')
    let vis_guibg = s:extract_or(s:input_default_hi_group, 'bg', 'gui', '#544a65')
    let ident_ctermfg = s:extract_or('Normal', 'fg', 'cterm', '249')
    let ident_guifg = s:extract_or('Normal', 'fg', 'gui', '#b2b2b2')
    execute printf(
          \ 'hi ClapQuery guifg=%s ctermfg=%s ctermbg=%s guibg=%s cterm=bold gui=bold',
          \ ident_guifg,
          \ ident_ctermfg,
          \ vis_ctermbg,
          \ vis_guibg,
          \ )
  endif

  call s:hi_clap_symbol()

  let s:display_group = hlexists('ClapDisplay') ? 'ClapDisplay' : s:display_default_hi_group
  call s:hi_display_invisible()

  let s:preview_group = hlexists('ClapPreview') ? 'ClapPreview' : 'ClapDefaultPreview'
  call s:hi_preview_invisible()

  augroup ClapColorSchemeAdaptive
    autocmd!
    autocmd ColorScheme * call s:colorschme_adaptive()
  augroup END

  hi ClapDefaultPreview          ctermbg=237 guibg=#3E4452
  hi ClapDefaultSelected         ctermfg=80  guifg=#5fd7d7 cterm=bold,underline gui=bold,underline
  hi ClapDefaultCurrentSelection ctermfg=224 guifg=#ffd7d7 cterm=bold gui=bold

  hi default link ClapMatches Search
  hi default link ClapPreview ClapDefaultPreview
  hi default link ClapSelected ClapDefaultSelected
  hi default link ClapPopupCursor Type
  hi default link ClapNoMatchesFound ErrorMsg
  hi default link ClapCurrentSelection ClapDefaultCurrentSelection

  execute 'hi default link ClapInput' s:input_default_hi_group
  execute 'hi default link ClapDisplay' s:display_default_hi_group

  call s:init_submatches_hl_group()
  call s:init_fuzzy_match_hl_groups()
endfunction

function! clap#init#() abort
  call clap#api#bake()
  call s:init_hi_groups()

  " This augroup should be retained after closing vim-clap for the benefit
  " of next run.
  if !exists('#ClapResize')
    augroup ClapResize
      autocmd!
      autocmd VimResized * call clap#layout#on_resize()
    augroup END
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
