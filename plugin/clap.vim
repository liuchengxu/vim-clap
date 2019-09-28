" vim-clap - Modern interactive filter and dispatcher
" Author:    Liu-Cheng Xu <xuliuchengxlc@gmail.com>
" Website:   https://github.com/liuchengxu/vim-clap
" License:   MIT

command! -bang -nargs=* -bar -complete=custom,clap#complete Clap call clap#(<bang>0, <f-args>)
