#!/usr/bin/env bash

new_tag=v0.9

prev_tag=$(git tag | tail -n2 | head -n1)
changelog=$(./ci/get_changelog.sh "$new_tag" "$prev_tag")

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
