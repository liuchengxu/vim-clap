#!/usr/bin/env bash

cd "$(dirname "${BASH_SOURCE[0]}")"

MAPLE_CARGO_TOML="../Cargo.toml"

current_tag=$(git describe --abbrev=0 --tags "$(git rev-list --tags --max-count=1)")
echo 'Prepare new release for vim-clap'
echo ''

echo "     Current tag: $current_tag"
read -p "Next tag version: v0." next_tag
echo ''

current_maple_version=$(cat "$MAPLE_CARGO_TOML" | grep '^version' | cut -f 2 -d '='  | tr -d '[:space:]' | tr -d '"')
echo "Current maple version: $current_maple_version"
read -p "   Next maple version: 0.1." next_maple_version
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

echo "          Next tag: v0.$next_tag"
echo "Next maple version: v0.1.$next_maple_version"
ask "confirmed?"
confirmed=$?

if [ $confirmed -eq 0 ]; then
  echo "Cancelled"
else
  ./prepare_release.py "v0.$next_tag" "0.1.$next_maple_version"

  # Fix Cargo.lock needs to be updated but --locked was passed to prevent this
  cd ..
  cargo build --release

  echo ''
  echo "New release v0.$next_tag is ready to go!"
  echo ''
  echo 'Now run git diff to check again, then commit and tag a new version:'
  echo '    git add -u' > publish.sh
  echo "    git commit -m v0.$next_tag" >> publish.sh
  echo "    git push origin master" >> publish.sh
  echo "    git tag v0.$next_tag" >> publish.sh
  echo "    git push origin v0.$next_tag" >> publish.sh
  echo 'Run `bash publish.sh` to publish this new release'

fi
