#!/usr/bin/env bash

current_tag=$(git tag | tail -1)
echo 'Prepare new release for vim-clap'
echo ''

echo "     Current tag: $current_tag"
read -p "Next tag version: " next_tag
echo ''

current_maple_version=$(cat Cargo.toml | grep '^version' | cut -f 2 -d '='  | tr -d '[:space:]' | tr -d '"')
echo "Current maple version: $current_maple_version"
read -p "   Next maple version: " next_maple_version
echo ''

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

echo "          Next tag: $next_tag"
echo "Next maple version: $next_maple_version"
ask "confirmed?"
confirmed=$?

if [ $confirmed -eq 0 ]; then
  echo "Cancelled"
else
  ./prepare_release.py "$next_tag" "$next_maple_version"

  # Fix Cargo.lock needs to be updated but --locked was passed to prevent this
  cargo build --release --locked

  echo ''
  echo "New release $next_tag is ready to go!"
  echo ''
  echo 'Now run git diff to check again, then commit and tag a new version:'
  echo "    git commit -m $next_tag"
  echo "    git tag $next_tag"
  echo "    git push origin $next_tag"
fi
