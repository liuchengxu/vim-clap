let s:palette = {}

let s:palette.display = { 'guibg': '#272B3D' }
let s:palette.spinner = { 'cterm': 'bold', 'ctermfg': '184','gui': 'bold', 'guifg':'#ffe920', 'guibg': '#4E5379'}
let s:palette.search_text = { 'guibg': '#4E5379', 'guifg': '#A5AACD', 'cterm': 'bold', 'gui': 'bold' }
let s:palette.input = { 'guibg': '#4E5379' }
let s:palette.preview = { 'ctermbg': '237', 'guibg': '#3E4452' }

let s:palette.selected = { 'cterm': 'bold,underline', 'gui': 'bold,underline', 'ctermfg': '80', 'guifg': '#5fd7d7' }
let s:palette.current_selection = {'cterm': 'bold', 'gui': 'bold'}

let g:clap#themes#material_design_dark#palette = s:palette
