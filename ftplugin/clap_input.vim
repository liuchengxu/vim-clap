if exists('b:clap_input_loaded') || !has('nvim')
  finish
endif

let b:clap_input_loaded = 1

setlocal
  \ nonumber
  \ norelativenumber
  \ nopaste
  \ nomodeline
  \ noswapfile
  \ nocursorline
  \ nocursorcolumn
  \ colorcolumn=
  \ nobuflisted
  \ buftype=nofile
  \ bufhidden=hide
  \ signcolumn=no
  \ textwidth=0
  \ nolist
  \ winfixwidth
  \ winfixheight
  \ nospell
  \ nofoldenable
  \ foldcolumn=0
  \ nowrap

if g:clap.provider.id !=# 'filer'
  augroup ClapOnTyped
    autocmd!
    autocmd CursorMoved,CursorMovedI <buffer> call clap#handler#on_typed()
  augroup END
endif

" From vim-rsi
if !exists('g:loaded_rsi')
  inoremap <silent> <buffer> <C-a>  <C-o>0
  inoremap <silent> <buffer> <C-X><C-A> <C-A>

  inoremap <silent> <buffer> <expr> <C-B> getline('.')=~'^\s*$'&&col('.')>strlen(getline('.'))?"0\<Lt>C-D>\<Lt>Esc>kJs":"\<Lt>Left>"
  inoremap <silent> <buffer> <expr> <C-F> col('.')>strlen(getline('.'))?"\<Lt>C-F>":"\<Lt>Right>"

  inoremap <silent> <buffer> <expr> <C-D> col('.')>strlen(getline('.'))?"\<Lt>C-D>":"\<Lt>Del>"
endif

inoremap <silent> <buffer> <expr> <C-E> col('.')>strlen(getline('.'))<bar><bar>pumvisible()?"\<Lt>C-E>":"\<Lt>End>"

inoremap <silent> <buffer> <CR> <Esc>:call clap#handler#sink()<CR>

nnoremap <silent> <buffer> <C-c> :call clap#handler#exit()<CR>
nnoremap <silent> <buffer> <Esc> :call clap#handler#exit()<CR>
nnoremap <silent> <buffer> <C-g> :call clap#handler#exit()<CR>

inoremap <silent> <buffer> <C-c> <Esc>:call clap#handler#exit()<CR>
inoremap <silent> <buffer> <Esc> <Esc>:call clap#handler#exit()<CR>
inoremap <silent> <buffer> <C-g> <Esc>:call clap#handler#exit()<CR>

inoremap <silent> <buffer> <C-j> <C-R>=clap#handler#navigate_result('down')<CR>
inoremap <silent> <buffer> <C-k> <C-R>=clap#handler#navigate_result('up')<CR>

inoremap <silent> <buffer> <Down> <C-R>=clap#handler#navigate_result('down')<CR>
inoremap <silent> <buffer> <Up> <C-R>=clap#handler#navigate_result('up')<CR>

inoremap <silent> <buffer> <PageDown> <C-R>=clap#handler#scroll('down')<CR>
inoremap <silent> <buffer> <PageUp> <C-R>=clap#handler#scroll('up')<CR>

inoremap <silent> <buffer> <Tab> <C-R>=clap#handler#select_toggle()<CR>

inoremap <silent> <buffer> <Tab> <C-R>=clap#provider#filer#tab()<CR>
inoremap <silent> <buffer> <BS> <C-R>=clap#provider#filer#bs()<CR>

call clap#util#define_open_action_mappings()
