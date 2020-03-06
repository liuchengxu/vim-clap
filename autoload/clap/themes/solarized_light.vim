" Author: Jesse Cooke <clap@relativepath.io>
" Description: Clap theme based on the Solarized Light theme.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:palette = {}

" base3 bg
" base03 fg
let s:palette.display = { 'ctermbg': '230', 'guibg': '#fdf6e3', 'guifg': '#002b36', 'ctermfg': '234' }

" Let ClapInput, ClapSpinner and ClapSearchText use the same backgound.
" base2
let s:bg0 = { 'ctermbg': '254', 'guibg': '#eee8d5' }
let s:palette.input = s:bg0
" base01
let s:palette.spinner = extend({ 'ctermfg': '240', 'guifg':'#586e75', 'cterm': 'bold', 'gui': 'bold'}, s:bg0)
" base02
let s:palette.search_text = extend({ 'ctermfg': '235', 'guifg': '#073642', 'cterm': 'bold', 'gui': 'bold' }, s:bg0)

let s:palette.preview = s:bg0

" base02 bg
let s:selected = { 'ctermbg': '235', 'guibg': '#073642', 'cterm': 'bold', 'gui': 'bold' }
let s:palette.selected = s:selected
let s:palette.selected_sign = s:selected
" base2 bg
" base02 fg
let s:palette.current_selection = { 'ctermbg': '254', 'guibg': '#eee8d5', 'ctermfg': '235', 'guifg': '#073642', 'cterm': 'bold', 'gui': 'bold' }
" red
let s:palette.current_selection_sign = extend({ 'ctermfg': '160', 'guifg': '#dc322f' }, s:palette.current_selection)

let g:clap#themes#solarized_light#palette = s:palette

let &cpoptions = s:save_cpo
unlet s:save_cpo
