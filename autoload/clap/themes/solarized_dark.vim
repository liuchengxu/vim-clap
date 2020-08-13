" Author: Jesse Cooke <jesse@relativepath.io>
" Description: Clap theme based on the Solarized Dark theme.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:base03  = { 'hex': '#002b36', 'xterm': '234', 'xterm_hex': '#1c1c1c' }
let s:base02  = { 'hex': '#073642', 'xterm': '235', 'xterm_hex': '#262626' }
let s:base01  = { 'hex': '#586e75', 'xterm': '240', 'xterm_hex': '#585858' }
let s:base00  = { 'hex': '#657b83', 'xterm': '241', 'xterm_hex': '#626262' }
let s:base0   = { 'hex': '#839496', 'xterm': '244', 'xterm_hex': '#808080' }
let s:base1   = { 'hex': '#93a1a1', 'xterm': '245', 'xterm_hex': '#8a8a8a' }
let s:base2   = { 'hex': '#eee8d5', 'xterm': '254', 'xterm_hex': '#e4e4e4' }
let s:base3   = { 'hex': '#fdf6e3', 'xterm': '230', 'xterm_hex': '#ffffd7' }
let s:yellow  = { 'hex': '#b58900', 'xterm': '136', 'xterm_hex': '#af8700' }
let s:orange  = { 'hex': '#cb4b16', 'xterm': '166', 'xterm_hex': '#d75f00' }
let s:red     = { 'hex': '#dc322f', 'xterm': '160', 'xterm_hex': '#d70000' }
let s:magenta = { 'hex': '#d33682', 'xterm': '125', 'xterm_hex': '#af005f' }
let s:violet  = { 'hex': '#6c71c4', 'xterm':  '61', 'xterm_hex': '#5f5faf' }
let s:blue    = { 'hex': '#268bd2', 'xterm':  '33', 'xterm_hex': '#0087ff' }
let s:cyan    = { 'hex': '#2aa198', 'xterm':  '37', 'xterm_hex': '#00afaf' }
let s:green   = { 'hex': '#859900', 'xterm':  '64', 'xterm_hex': '#5f8700' }

let s:palette = {}

let s:palette.display = {
  \'ctermbg': s:base03.xterm,
  \'guibg':   s:base03.hex,
  \'ctermfg': s:base3.xterm,
  \'guifg':   s:base3.hex,
\}

" Let ClapInput, ClapSpinner and ClapSearchText use the same backgound.
let s:bg0 = {
  \'guibg': s:base02.hex,
  \'ctermbg': s:base02.xterm,
\}
let s:palette.input = s:bg0
let s:palette.spinner = extend({
  \'guifg': s:base1.hex,
  \'ctermfg': s:base1.xterm,
  \'cterm': 'bold',
  \'gui': 'bold',
\}, s:bg0)


let s:palette.preview = s:bg0

let s:selected = {
  \'guifg': s:base2.hex,
  \'ctermfg': s:base2.xterm,
  \'cterm': 'bold',
  \'gui': 'bold',
\}
let s:palette.search_text = extend(s:selected, s:bg0)
let s:palette.selected = s:selected
let s:palette.selected_sign = s:selected

let s:palette.current_selection = {
  \'guibg': s:base02.hex,
  \'ctermbg': s:base02.xterm,
  \'guifg': s:base2.hex,
  \'ctermfg': s:base2.xterm,
  \'cterm': 'bold',
  \'gui': 'bold',
\}

let s:palette.current_selection_sign = extend({
  \'guifg': s:red.hex,
  \'ctermfg': s:red.xterm,
\}, s:palette.current_selection)

let s:fuzzy = [
  \ [s:base03.xterm, s:base3.hex],
  \ [s:base02.xterm, s:base2.hex],
  \ [s:base01.xterm, s:base1.hex],
  \ [s:base00.xterm, s:base0.hex],
  \ [s:base0.xterm, s:base00.hex],
  \ [s:base1.xterm, s:base01.hex],
\ ]
let g:clap_fuzzy_match_hl_groups = s:fuzzy

let s:clap_file_style = 'ctermfg=' . s:base0.xterm . ' ctermbg=NONE guifg=' . s:base0.hex . ' guibg=NONE'
execute 'highlight ClapFile '. s:clap_file_style

let g:clap#themes#solarized_dark#palette = s:palette

let &cpoptions = s:save_cpo
unlet s:save_cpo
