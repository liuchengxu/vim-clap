" Author: GoldsteinE <mouse-art@ya.ru>
" Description: Clap theme based on the atom-dark theme.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:palette = {}

let s:palette.display = { 'ctermbg': '239', 'guibg': '#2d3135', 'guifg': '#f8f8f2', 'ctermfg': '123' }

" Let ClapInput, ClapSpinner and ClapSearchText use the same backgound.
let s:bg0 = { 'ctermbg': '59', 'guibg': '#403d3d' }
let s:palette.input = s:bg0
let s:palette.spinner = extend({ 'ctermfg': '229', 'guifg':'#dad085', 'cterm': 'bold', 'gui': 'bold'}, s:bg0)
let s:palette.search_text = extend({ 'ctermfg': '249', 'guifg': '#f8f8f2', 'cterm': 'bold', 'gui': 'bold' }, s:bg0)

let s:palette.preview = { 'ctermbg': '238', 'guibg': '#292b2d' }

let s:palette.selected = { 'ctermbg': '59', 'guibg': '#2d3a3d', 'cterm': 'bold', 'gui': 'bold' }
let s:palette.selected_sign = s:palette.selected
let s:palette.current_selection = { 'ctermbg': '242', 'guibg': '#334043', 'cterm': 'bold', 'gui': 'bold' }
let s:palette.current_selection_sign = s:palette.current_selection

let g:clap#themes#atom_dark#palette = s:palette

let &cpoptions = s:save_cpo
unlet s:save_cpo
