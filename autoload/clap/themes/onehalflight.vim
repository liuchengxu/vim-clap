" Author: kerunaru <kerunaru@icloud.com>
" Description: Clap theme based on the onehalflight theme.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:palette = {}

let s:palette.display = { 'ctermbg': '255', 'guibg': '#f0f0f0' } " cursor_line

" Let ClapInput, ClapSpinner and ClapSearchText use the same background.
let s:bg0 = { 'ctermbg': '252', 'guibg': '#e5e5e5' } " non_text
let s:palette.input = s:bg0
let s:palette.indicator = extend({ 'ctermfg': '247', 'guifg':'#a0a1a7' }, s:bg0) " comment_fg
let s:palette.spinner = extend({ 'ctermfg': '166', 'guifg':'#c18401', 'cterm': 'bold', 'gui': 'bold'}, s:bg0) " yellow
let s:palette.search_text = extend({ 'ctermfg': '237', 'guifg': '#383a42', 'cterm': 'bold', 'gui': 'bold' }, s:bg0) " black

let s:palette.preview = { 'ctermbg': '252', 'guibg': '#e5e5e5' } " non_text

let s:palette.selected = { 'ctermfg': '31', 'guifg': '#0997b3', 'cterm': 'bold,underline', 'gui': 'bold,underline' } " cyan
let s:palette.current_selection = { 'ctermbg': '231', 'guibg': '#fafafa', 'cterm': 'bold', 'gui': 'bold' } " gutter_bg

let s:palette.selected_sign = { 'ctermfg': '167', 'guifg': '#e45649' } " red
let s:palette.current_selection_sign = s:palette.selected_sign

" blue
let g:clap_fuzzy_match_hl_groups = [
  \ ['75', '#61afef'],
\ ]

let g:clap#themes#onehalflight#palette = s:palette

let &cpoptions = s:save_cpo
unlet s:save_cpo
