#!/usr/bin/env bash

cd "$(dirname "${BASH_SOURCE[0]}")"

new_tag=v0.31
# If the latest tag is v0.10, returns v0.9
prev_tag=$(git describe --abbrev=0 --tags "$(git rev-list --tags --skip=1  --max-count=1)")
changelog=$(../ci/get_changelog.sh "$new_tag" "$prev_tag")

echo "Release $new_tag" > tmp_release_notes
echo ""                >> tmp_release_notes
echo "$changelog"      >> tmp_release_notes

cat tmp_release_notes

ask() {
  while true; do
    read -p "$1 ([y]/n) " -r
    REPLY=${REPLY:-"y"}
    if [[ $REPLY =~ ^[Yy]$ ]]; then
      return 1
    elif [[ $REPLY =~ ^[Nn]$ ]]; then
      return 0
    fi
  done
}

echo ""
ask "Update $new_tag release notes?"
confirmed=$?

if [ $confirmed -eq 0 ]; then
  echo "Cancelled"
else
  hub release edit --file tmp_release_notes "$new_tag"
  rm tmp_release_notes
fi
