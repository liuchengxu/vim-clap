" With my machine:
" /usr in macOS has over 300,000 files.
" /usr in Ubuntu18.04 has less than 300,000 files
let g:clap_builtin_fuzzy_filter_threshold = 400000

if has('macunix')
  function! s:RunClap() abort
    silent Clap files ~/src/github.com
  endfunction
else
  function! s:RunClap() abort
    silent Clap files /usr
  endfunction
endif

function RunInputOnce() abort
  call s:RunClap()
  if has('nvim')
    call timer_start(5000, { -> feedkeys("sr") } )
  else
    " wait for the forerunner job and then input something.
    call timer_start(15000, { -> feedkeys("sr", "xt") } )
  endif
  call timer_start(18000, { -> writefile(['total items: '.g:clap.display.initial_size], 'stats.log', 'a') })
  call timer_start(20000, { -> execute("qa!") } )
endfunction

function RunInputMulti() abort
  call s:RunClap()
  if has('nvim')
    call timer_start(5000, { -> feedkeys("s") } )
    call timer_start(10000, { -> feedkeys("r") } )
    call timer_start(15000, { -> feedkeys("q") } )
  else
    call timer_start(5000, { -> feedkeys("s", "xt") } )
    call timer_start(10000, { -> feedkeys("r", "xt") } )
    call timer_start(15000, { -> feedkeys("q", "xt") } )
  endif
  call timer_start(18000, { -> writefile(['total items: '.g:clap.display.initial_size], 'stats.log', 'a') })
  call timer_start(20000, { -> execute("qa!") } )
endfunction

function! PythonFilter(query, candidates) abort
  call clap#filter#python#(a:query, a:candidates, 60)
endfunction

function RunBench100000() abort
  let candidates = readfile(expand('testdata.txt'), '', 100000)
  call PythonFilter('sr', candidates)
  call timer_start(10000, { -> execute("qa!") } )
endfunction

function RunBench200000() abort
  let candidates = readfile(expand('testdata.txt'), '', 200000)
  call PythonFilter('sr', candidates)
  call timer_start(15000, { -> execute("qa!") } )
endfunction

function RunBench300000() abort
  let candidates = readfile(expand('testdata.txt'), '', 300000)
  call PythonFilter('sr', candidates)
  call timer_start(20000, { -> execute("qa!") } )
endfunction
