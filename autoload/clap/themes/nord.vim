" Author: Ihor Kalnytskyi <ihor@kalnytskyi.com>
" Description: Clap theme based on Nord theme.

let s:save_cpo = &cpoptions
set cpoptions&vim

" colors from arcticicestudio/nord-vim
let s:nord0_gui = '#2E3440'
let s:nord1_gui = '#3B4252'
let s:nord2_gui = '#434C5E'
let s:nord3_gui = '#4C566A'
let s:nord4_gui = '#D8DEE9'
let s:nord5_gui = '#E5E9F0'
let s:nord6_gui = '#ECEFF4'
let s:nord7_gui = '#8FBCBB'
let s:nord8_gui = '#88C0D0'
let s:nord9_gui = '#81A1C1'
let s:nord10_gui = '#5E81AC'
let s:nord11_gui = '#BF616A'
let s:nord12_gui = '#D08770'
let s:nord13_gui = '#EBCB8B'
let s:nord14_gui = '#A3BE8C'
let s:nord15_gui = '#B48EAD'

let s:nord1_term = '0'
let s:nord3_term = '8'
let s:nord5_term = '7'
let s:nord6_term = '15'
let s:nord7_term = '14'
let s:nord8_term = '6'
let s:nord9_term = '4'
let s:nord10_term = '12'
let s:nord11_term = '1'
let s:nord12_term = '11'
let s:nord13_term = '3'
let s:nord14_term = '2'
let s:nord15_term = '5'

let s:palette = {}
let s:palette.display = {
  \ 'guibg': s:nord1_gui,
  \ 'ctermbg': s:nord1_term,
  \ 'guifg': s:nord4_gui,
  \ 'ctermfg': 'NONE',
\ }
let s:palette.input = s:palette.display
let s:palette.spinner = extend(
  \ {
    \ 'guifg': s:nord9_gui,
    \ 'ctermfg': s:nord9_term,
    \ 'gui': 'bold',
    \ 'cterm': 'bold',
  \ },
  \ s:palette.input,
  \ 'keep'
\ )
let s:palette.search_text = s:palette.input
let s:palette.selected = {
  \ 'guibg': s:nord2_gui,
  \ 'guifg': s:nord4_gui,
  \ 'ctermbg': s:nord3_term,
  \ 'ctermfg': 'NONE',
\ }
let s:palette.selected_sign = s:palette.selected
let s:palette.current_selection = extend(
  \ {
    \ 'gui': 'bold',
    \ 'cterm': 'bold',
  \ },
  \ s:palette.selected,
  \ 'keep'
\ )
let s:palette.current_selection_sign = s:palette.current_selection
let s:palette.preview = {
  \ 'guibg': s:nord2_gui,
  \ 'ctermbg': s:nord3_term
\ }

let g:clap#themes#nord#palette = s:palette
let g:clap_fuzzy_match_hl_groups = [
  \ [s:nord8_term, s:nord8_gui],
\ ]

let &cpoptions = s:save_cpo
unlet s:save_cpo
