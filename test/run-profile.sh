#!/usr/bin/env bash

run_exe() {
  local exe=$1
  local profile_log=$2
  local test_fn=$3

  $exe -u profile.vimrc \
    --cmd "profile start $profile_log.log" \
    --cmd 'profile func *' \
    --cmd 'profile file *' \
    --cmd 'set verbosefile=verbose.log' \
    --cmd 'set verbose=9' \
    -c "call $test_fn()"
}


# run_exe vim  vimprofile  RunInputOnce
# run_exe nvim nvimprofile RunInputOnce

# run_exe vim  vimprofile_multi  RunInputMulti
# run_exe nvim nvimprofile_multi RunInputMulti

run_exe vim  vimprofile_bench  RunBenchmarkDirectly
run_exe nvim nvimprofile_bench RunBenchmarkDirectly
