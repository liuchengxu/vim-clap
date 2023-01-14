if exists('b:clap_display_loaded') || !has('nvim')
  finish
endif

let b:clap_display_loaded = 1

setlocal
  \ nowrap
  \ nonumber
  \ norelativenumber
  \ nopaste
  \ nocursorline
  \ nocursorcolumn
  \ foldcolumn=0
  \ nomodeline
  \ noswapfile
  \ colorcolumn=
  \ nobuflisted
  \ buftype=nofile
  \ bufhidden=hide
  \ signcolumn=yes
  \ textwidth=0
  \ nolist
  \ winfixwidth
  \ winfixheight
  \ nospell
  \ nofoldenable

inoremap <silent> <buffer> <ScrollWheelDown> <C-R>=clap#navigation#linewise_scroll('down')<CR>
inoremap <silent> <buffer> <ScrollWheelUp>   <C-R>=clap#navigation#linewise_scroll('up')<CR>

inoremap <silent> <buffer> <LeftMouse>       <C-R>=clap#handler#tab_action()<CR>
inoremap <silent> <buffer> <RightMouse>      <C-R>=clap#handler#tab_action()<CR>

nnoremap <silent> <buffer> <C-c>     :<c-u>call clap#handler#exit()<CR>
nnoremap <silent> <buffer> <C-g>     :<c-u>call clap#handler#exit()<CR>
nnoremap <silent> <buffer> <CR>      :<c-u>call clap#handler#sink()<CR>

nnoremap <silent> <buffer> <ScrollWheelDown> :<c-u>call clap#navigation#linewise_scroll('down')<CR>
nnoremap <silent> <buffer> <ScrollWheelUp>   :<c-u>call clap#navigation#linewise_scroll('up')<CR>

nnoremap <silent> <buffer> <LeftMouse>       :<c-u>call clap#handler#tab_action()<CR>
nnoremap <silent> <buffer> <RightMouse>      :<c-u>call clap#handler#tab_action()<CR>
