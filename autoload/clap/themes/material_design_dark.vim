" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Clap theme based on the material_design_dark theme.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:palette = {}

let s:palette.display = { 'ctermbg': '68', 'guibg': '#272d3D' }

" Let ClapInput, ClapSpinner and ClapSearchText use the same backgound.
let s:bg0 = { 'ctermbg': '111', 'guibg': '#3e4461' }
let s:palette.input = s:bg0
let s:palette.spinner = extend({ 'ctermfg': '11', 'guifg':'#ffe500', 'cterm': 'bold', 'gui': 'bold'}, s:bg0)
let s:palette.search_text = extend({ 'ctermfg': '195', 'guifg': '#CADFF3', 'cterm': 'bold', 'gui': 'bold' }, s:bg0)

let s:palette.preview = { 'ctermbg': '74', 'guibg': '#363c55' }

let s:palette.selected = { 'ctermfg': '81', 'guifg': '#5FD7d7', 'cterm': 'bold,underline', 'gui': 'bold,underline' }
let s:palette.current_selection = { 'ctermbg': '98', 'guibg': '#31364D', 'cterm': 'bold', 'gui': 'bold' }

let g:clap#themes#material_design_dark#palette = s:palette

let &cpoptions = s:save_cpo
unlet s:save_cpo
