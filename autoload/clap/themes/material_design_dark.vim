let s:palette = {}

let s:palette.search_text = []

let s:palette.current_selection = {'ctermbg': 224, 'guifg': '#C1E987', 'guibg': '#4E5379', 'cterm': 'bold', 'gui': 'bold'}

let s:palette.current_selection = {'ctermbg': 224, 'guibg': '#4E5379', 'cterm': 'bold', 'gui': 'bold'}

let s:palette.display = { 'guibg': '#272B3D' }

let s:palette.input = { 'guibg': '#272B3D' }

let s:palette.spinner = { 'guibg': '#272B3D', 'guifg': '#C1E987', 'cterm': 'bold', 'gui': 'bold' }

let s:palette.query = { 'guibg': '#272B3D', 'guifg': '#A5AACD', 'cterm': 'bold', 'gui': 'bold' }

let g:clap#themes#material_design_dark#palette = s:palette
