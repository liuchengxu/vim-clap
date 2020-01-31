#!/usr/bin/env bash

# Usage: ./ci/get_changelog.sh new_tag
#
# Update prev_tag manually for new release.

cd "$(dirname "${BASH_SOURCE[0]}")" || exit

cd ..

# v0.3
cur_tag=$1
# 0.3
cur_header="[${cur_tag:1:8}]"

# FIXME get prev_tag in GA
# prev_tag=$(git describe --abbrev=0 --tags "$(git rev-list --tags --skip=1 --max-count=1)")
prev_tag="v0.6"
prev_header="[${prev_tag:1:8}]"

begin=$(grep -Fn "$cur_header" CHANGELOG.md | awk '{split($0,a,":"); print a[1]}')

end=$(grep -Fn "$prev_header" CHANGELOG.md | awk '{split($0,a,":"); print a[1]}')
end="$(("$end"-1))"

sed -n "$begin","$end"p CHANGELOG.md
