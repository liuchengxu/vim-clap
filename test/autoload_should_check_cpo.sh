#!/usr/bin/env bash

cd "$(dirname "${BASH_SOURCE[0]}")"

cd ..

expected="unlet s:save_cpo"

for entry in $(find autoload -path '*' -type f)
do
  last_line=$(tail -n 1 $entry)
  if [ "$expected" == "$last_line" ]; then
    echo "[PASS] $entry"
  else
    echo "[ERROR] $entry does not check compatible-options."
    exit 1
  fi
done
