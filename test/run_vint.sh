#!/usr/bin/env bash

cd "$(dirname "${BASH_SOURCE[0]}")"

cd ..

# Skip autoload/clap/filter.vim for this vimlparser issue.
#
# See https://github.com/vim-jp/vim-vimlparser/issues/33
to_be_tested=$(find . -name "*.vim" -type f | grep -v "filter.vim")

for entry in $to_be_tested
do
  vint -e "$entry"
done
