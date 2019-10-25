" vim-clap - Modern interactive filter and dispatcher
" Author:    Liu-Cheng Xu <xuliuchengxlc@gmail.com>
" Website:   https://github.com/liuchengxu/vim-clap
" Version:   Pre-0.1
" License:   MIT

if exists('g:loaded_clap')
  finish
endif

let g:loaded_clap = 1

command! -bang -nargs=* -bar -complete=custom,clap#complete Clap call clap#(<bang>0, <f-args>)

let g:__clap_buffers = get(g:, '__clap_buffers', {})

augroup ClapBuffers
  autocmd!
  autocmd BufWinEnter,WinEnter * let g:__clap_buffers[bufnr('')] = reltimefloat(reltime())
  autocmd BufDelete * silent! call remove(g:__clap_buffers, expand('<abuf>'))
augroup END

" yanks provider
if get(g:, 'clap_enable_yanks_provider', 1)
  augroup ClapYanks
    autocmd!
    autocmd VimEnter * call clap#provider#yanks#init()
  augroup END
endif
