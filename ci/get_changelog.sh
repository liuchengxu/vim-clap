#!/usr/bin/env bash

cd "$(dirname "${BASH_SOURCE[0]}")"

cd ..

# v0.3
cur_tag=$(git describe --abbrev=0)
prev_tag=$(git describe --abbrev=0 --tags "$(git rev-list --tags --skip=1 --max-count=1)")

# 0.3
cur_header="[${cur_tag:1:3}]"
prev_header="[${prev_tag:1:3}]"

begin=$(grep -Fn "$cur_header" CHANGELOG.md | awk '{split($0,a,":"); print a[1]}')

end=$(grep -Fn "$prev_header" CHANGELOG.md | awk '{split($0,a,":"); print a[1]}')
end="$(("$end"-1))"

sed -n "$begin","$end"p CHANGELOG.md
