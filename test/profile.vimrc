set nocompatible
set runtimepath^=~/.vim/plugged/vim-clap
syntax on
filetype plugin indent on

let g:clap_builtin_fuzzy_filter_threshold = 200000

function RunInputOnce() abort
  silent Clap files /usr
  if has('nvim')
    " wait for the forerunner job and then input something.
    call timer_start(5000, { -> feedkeys("sr", "xt") } )
  else
    call timer_start(5000, { -> feedkeys("sr") } )
  endif
  call timer_start(15000, { -> execute("qa!") } )
endfunction

function RunInputMulti() abort
  silent Clap files /usr
  if has('nvim')
    call timer_start(5000, { -> feedkeys("s", "xt") } )
    call timer_start(2000, { -> feedkeys("r", "xt") } )
    call timer_start(2000, { -> feedkeys("q", "xt") } )
  else
    call timer_start(5000, { -> feedkeys("s") } )
    call timer_start(2000, { -> feedkeys("r") } )
    call timer_start(2000, { -> feedkeys("q") } )
  endif
  call timer_start(15000, { -> execute("qa!") } )
endfunction

function RunBenchmarkDirectly() abort
  let candidates = readfile(expand('testdata.txt'), '', 100000)
  call clap#filter#benchmark('sr', candidates)
  call timer_start(5000, { -> execute("qa!") } )
endfunction
