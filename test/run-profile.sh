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

run_once() {
  run_exe vim  vimprofile  RunInputOnce
  run_exe nvim nvimprofile RunInputOnce

  echo 'run_once' > run_once.log
  echo '---------- vim' >> run_once.log
  grep 'ext_filter()' vimprofile.log >> run_once.log
  echo '---------- nvim' >> run_once.log
  grep 'ext_filter()' nvimprofile.log >> run_once.log
}

run_multi() {
  run_exe vim  vimprofile_multi  RunInputMulti
  run_exe nvim nvimprofile_multi RunInputMulti
  echo 'run_multi' > run_multi.log
  echo '---------- vim' >> run_multi.log
  grep 'ext_filter()' vimprofile_multi.log >> run_multi.log
  echo '---------- nvim' >> run_multi.log
  grep 'ext_filter()' nvimprofile_multi.log >> run_multi.log
}


run_bench() {
  run_exe vim  vimprofile_bench  RunBenchmarkDirectly
  run_exe nvim nvimprofile_bench RunBenchmarkDirectly

  echo 'run_bench' > run_bench.log
  echo '---------- vim' >> run_bench.log
  grep 'ext_filter()' vimprofile_bench.log >> run_bench.log
  echo '---------- nvim' >> run_bench.log
  grep 'ext_filter()' nvimprofile_bench.log >> run_bench.log
}

for opt in "$@"; do
  case $opt in
    --once)  run_once  ;;
    --multi) run_multi ;;
    --bench) run_bench ;;
    --all)
      run_once
      run_multi
      run_bench
      ;;
    *)
      echo "unknown option: $opt"
      exit 1
      ;;
  esac
done
