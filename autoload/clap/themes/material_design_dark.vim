let s:palette = {}

" Normal mode                                    " guifg guibg ctermfg ctermbg
let s:palette.search_box = []

let s:palette.search_text = []

let s:palette.current_selection = {'ctermbg': 224, 'guifg': '#C1E987', 'guibg': '#4E5379', 'cterm': 'bold', 'gui': 'bold'}

let s:palette.current_selection = {'ctermbg': 224, 'guibg': '#4E5379', 'cterm': 'bold', 'gui': 'bold'}

let s:palette.display = { 'guibg': "#272B3D" }

let s:palette.spinner = { 'guibg': "#272B3D", 'guifg': '#C1E987', 'cterm': 'bold', 'gui': 'bold' }

let s:palette.query = { 'guibg': "#272B3D", "guifg": '#A5AACD', 'cterm': 'bold', 'gui': 'bold' }

let g:clap#themes#material_design_dark#palette = s:palette

" {
  " "alfredtheme" : {
    " "result" : {
      " "textSpacing" : 8,
      " "subtext" : {
        " "size" : 12,
        " "colorSelected" : "#A5AACD",
        " "font" : "System Light",
        " "color" : "#676C96"
      " },
      " "shortcut" : {
        " "size" : 14,
        " "colorSelected" : "#FFCD00",
        " "font" : "System",
        " "color" : "#C78EEC"
      " },
      " "backgroundSelected" : "#4E5379",
      " "text" : {
        " "size" : 18,
        " "colorSelected" : "#C1E987",
        " "font" : "System",
        " "color" : "#A6AACD"
      " },
      " "iconPaddingHorizontal" : 10,
      " "paddingVertical" : 10,
      " "iconSize" : 40
    " },

    " "search" : {
      " "paddingVertical" : 2,
      " "background" : "#1B1E2A",
      " "spacing" : 10,
      " "text" : {
        " "size" : 32,
        " "colorSelected" : "#000000",
        " "font" : "System",
        " "color" : "#FEFFFE"
      " },
      " "backgroundSelected" : "#00B0E9"
    " },

    " "window" : {
      " "color" : "#272B3D",
      " "paddingHorizontal" : 0,
      " "width" : 780,
      " "borderPadding" : 0,
      " "borderColor" : "#000000",
      " "blur" : 40,
      " "roundness" : 8,
      " "paddingVertical" : 10
    " },

    " "credit" : "Doug C. Hardester",

    " "separator" : {
      " "color" : "#3A3E57",
      " "thickness" : 2
    " },

    " "scrollbar" : {
      " "color" : "#656C96",
      " "thickness" : 6
    " },

    " "name" : "Material Palenight"
  " }
" }
