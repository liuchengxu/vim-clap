" Author: kerunaru <kerunaru@icloud.com>
" Description: Clap theme based on the onehalfdark theme.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:palette = {}

let s:palette.display = { 'ctermbg': '237', 'guibg': '#313640' } " cursor_line

" Let ClapInput, ClapSpinner and ClapSearchText use the same background.
let s:bg0 = { 'ctermbg': '239', 'guibg': '#373C45' } " non_text
let s:palette.input = s:bg0
let s:palette.indicator = extend({ 'ctermfg': '247', 'guifg':'#919baa' }, s:bg0) " gutter_fg
let s:palette.spinner = extend({ 'ctermfg': '180', 'guifg':'#e5c07b', 'cterm': 'bold', 'gui': 'bold'}, s:bg0) " yellow
let s:palette.search_text = extend({ 'ctermfg': '188', 'guifg': '#dcdfe4', 'cterm': 'bold', 'gui': 'bold' }, s:bg0) " white

let s:palette.preview = { 'ctermbg': '239', 'guibg': '#373C45' } " non_text

let s:palette.selected = { 'ctermfg': '73', 'guifg': '#56b6c2', 'cterm': 'bold,underline', 'gui': 'bold,underline' } " cyan
let s:palette.current_selection = { 'ctermbg': '236', 'guibg': '#282c34', 'cterm': 'bold', 'gui': 'bold' } " gutter_bg

let s:palette.selected_sign = { 'ctermfg': '168', 'guifg': '#e06c75' } " red
let s:palette.current_selection_sign = s:palette.selected_sign

" blue
let g:clap_fuzzy_match_hl_groups = [
  \ ['75', '#61afef'],
\ ]

let g:clap#themes#onehalfdark#palette = s:palette

let &cpoptions = s:save_cpo
unlet s:save_cpo
