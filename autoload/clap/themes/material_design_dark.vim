" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Clap theme based on the material_design_dark theme.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:palette = {}

let s:palette.display = { 'guibg': '#272d3D' }

let s:bg0 = {'guibg': '#3e4461'}

let s:palette.input = s:bg0
let s:palette.spinner = extend({ 'ctermfg': '184', 'guifg':'#ffe500', 'cterm': 'bold', 'gui': 'bold'}, s:bg0)
let s:palette.search_text = extend({ 'guifg': '#CADFF3', 'cterm': 'bold', 'gui': 'bold' }, s:bg0)

let s:palette.preview = { 'ctermbg': '237', 'guibg': '#363c55' }

let s:palette.selected = { 'cterm': 'bold,underline', 'gui': 'bold,underline', 'ctermfg': '80', 'guifg': '#5fd7d7' }
let s:palette.current_selection = {'cterm': 'bold', 'gui': 'bold', 'guibg': '#31364D'}

let g:clap#themes#material_design_dark#palette = s:palette

let &cpoptions = s:save_cpo
unlet s:save_cpo
