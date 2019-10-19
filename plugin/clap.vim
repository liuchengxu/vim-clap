" vim-clap - Modern interactive filter and dispatcher
" Author:    Liu-Cheng Xu <xuliuchengxlc@gmail.com>
" Website:   https://github.com/liuchengxu/vim-clap
" License:   MIT

command! -bang -nargs=* -bar -complete=custom,clap#complete Clap call clap#(<bang>0, <f-args>)

let g:__clap_buffers = get(g:, '__clap_buffers', {})

augroup clapBuffers
  autocmd!
  autocmd BufWinEnter,WinEnter * let g:__clap_buffers[bufnr('')] = reltimefloat(reltime())
  autocmd BufDelete * silent! call remove(g:__clap_buffers, expand('<abuf>'))
augroup END
