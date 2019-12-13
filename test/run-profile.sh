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
    --cmd "let g:clap_use_pure_python = $USE_PURE_PYTHON" \
    -c "call $test_fn()"
}

note() {
  local exe=$1
  local from=$2

  echo "====== $exe ======" >> stats.log
  grep 'ext_filter()' $from.log | head -2 | sed -e 's/^[ \t]*//' >> stats.log

  echo '' >> stats.log
}

run_once() {
  run_exe vim  vimprofile  RunInputOnce
  run_exe nvim nvimprofile RunInputOnce

  echo '[once]' >> stats.log
  note vim vimprofile
  note nvim nvimprofile
}

run_multi() {
  run_exe vim  vimprofile_multi  RunInputMulti
  run_exe nvim nvimprofile_multi RunInputMulti

  echo '[multi]' >> stats.log
  note vim vimprofile_multi
  note nvim nvimprofile_multi
}

bench_100000() {
  run_exe vim  vimprofile_bench_100000  RunBench100000
  run_exe nvim nvimprofile_bench_100000 RunBench100000

  echo '[bench100000]' >> stats.log
  note vim vimprofile_bench_100000
  note nvim nvimprofile_bench_100000
}

bench_200000() {
  run_exe vim  vimprofile_bench_200000  RunBench200000
  run_exe nvim nvimprofile_bench_200000 RunBench200000

  echo '[bench200000]' >> stats.log
  note vim vimprofile_bench_200000
  note nvim nvimprofile_bench_200000
}

bench_300000() {
  run_exe vim  vimprofile_bench_300000  RunBench300000
  run_exe nvim nvimprofile_bench_300000 RunBench300000

  echo '[bench300000]' >> stats.log
  note vim vimprofile_bench_300000
  note nvim nvimprofile_bench_300000
}

run_bench() {
  bench_100000
}

run_all() {
  run_once
  run_multi
  run_bench
}

test_python_and_rust() {
  echo 'stats of pure Python fuzzy filter performance:' > stats.log
  echo '' >> stats.log
  USE_PURE_PYTHON=1
  $1

  echo '>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>' >> stats.log
  echo '<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<' >> stats.log
  echo '' >> stats.log

  echo 'stats of Rust fuzzy filter performance:' >> stats.log
  echo '' >> stats.log
  USE_PURE_PYTHON=0
  $1
}

help() {
  cat << EOF
usage: $0 [OPTIONS]
    --help               Show this message
    --once
    --multi
    --bench
    --all
EOF
}

if [ $# -eq 0 ]; then
  help
  exit 1
fi

for opt in "$@"; do
  case $opt in
    --help)  help      ;;
    --once)
      test_python_and_rust run_once
      ;;
    --multi)
      test_python_and_rust run_multi
      ;;
    --bench)
      test_python_and_rust run_bench
      ;;
    --all)
      test_python_and_rust run_all
      ;;
    *)
      echo "unknown option: $opt"
      exit 1
      ;;
  esac
done
