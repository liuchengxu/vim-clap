#!/usr/bin/env bash

# wait for the forerunner job and then input something.
vim -u profile.vimrc \
  --cmd 'profile start vimprofile.log' \
  --cmd 'profile func *' \
  --cmd 'profile file *' \
  --cmd 'set verbosefile=verbose.log' \
  --cmd 'set verbose=9' \
  -c ':silent Clap files ~/src/github.com' \
  -c 'call timer_start(5000, { -> feedkeys("sr", "xt") } )' \
  -c 'call timer_start(15000, { -> execute("qa!") } )'

nvim -u profile.vimrc \
  --cmd 'profile start nvimprofile.log' \
  --cmd 'profile func *' \
  --cmd 'profile file *' \
  --cmd 'set verbosefile=verbose.log' \
  --cmd 'set verbose=9' \
  -c ':silent Clap files ~/src/github.com' \
  -c 'call timer_start(5000, { -> feedkeys("sr") } )' \
  -c 'call timer_start(15000, { -> execute("qa!") } )'

vim -u profile.vimrc \
  --cmd 'profile start vimprofile_m.log' \
  --cmd 'profile func *' \
  --cmd 'profile file *' \
  --cmd 'set verbosefile=verbose.log' \
  --cmd 'set verbose=9' \
  -c ':silent Clap files ~/src/github.com' \
  -c 'call timer_start(5000, { -> feedkeys("s", "xt") } )' \
  -c 'call timer_start(2000, { -> feedkeys("r", "xt") } )' \
  -c 'call timer_start(2000, { -> feedkeys("q", "xt") } )' \
  -c 'call timer_start(15000, { -> execute("qa!") } )'

nvim -u profile.vimrc \
  --cmd 'profile start nvimprofile_m.log' \
  --cmd 'profile func *' \
  --cmd 'profile file *' \
  --cmd 'set verbosefile=verbose.log' \
  --cmd 'set verbose=9' \
  -c ':silent Clap files ~/src/github.com' \
  -c 'call timer_start(5000, { -> feedkeys("s") } )' \
  -c 'call timer_start(2000, { -> feedkeys("r") } )' \
  -c 'call timer_start(2000, { -> feedkeys("q") } )' \
  -c 'call timer_start(15000, { -> execute("qa!") } )'
