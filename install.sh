#!/usr/bin/env bash

set -u

version=v0.39
APP=maple

remote_latest_tag() {
  git -c 'versionsort.suffix=-' ls-remote --exit-code --refs --sort='version:refname' --tags "$REPO" 'v0.*' \
    | tail --lines=1 \
    | cut --delimiter='/' --fields=3
}

local_tag() {
  "bin/$APP" version | cut --delimiter=' ' --fields=4 | cut --delimiter='-' --fields=1
}

remote_latest_tag=$(remote_latest_tag)

exists() {
  command -v "$1" >/dev/null 2>&1
}

download() {
  local from=$1
  local to=$2
  if exists "curl"; then
    curl -fLo "$to" "$from"
  elif exists 'wget'; then
    wget --output-document="$to" "$from"
  else
    echo 'curl or wget is required'
    exit 1
  fi
}

try_download() {
  local DOWNLOAD_URL="$REPO/releases/download/$remote_latest_tag"
  local asset=$1
  if [ -z "${TMPDIR+x}" ]; then
    rm -f bin/$APP
    download "$DOWNLOAD_URL/$asset" bin/$APP
  else
    local temp=${TMPDIR}/maple
    download "$DOWNLOAD_URL/$asset" "$temp"
    mv "$temp" bin/$APP
  fi
  chmod a+x "bin/$APP"
}

do_download() {
  arch=$(uname -sm)
  case "${arch}" in
      "Linux x86_64")
        try_download "$APP"-x86_64-unknown-linux-musl ;;
      "Linux aarch64")
        try_download "$APP"-aarch64-unknown-linux-musl ;;
      "Darwin x86_64")
        try_download "$APP"-x86_64-apple-darwin ;;
      "Darwin arm64")
        try_download "$APP"-aarch64-apple-darwin ;;
      *)
        echo "No prebuilt maple binary available for ${arch}."
        exit 1
        ;;
  esac
}

if [ ! -f "bin/$APP" ]; then
  echo "bin/$APP is empty, try downloading $APP $remote_latest_tag from GitHub directly..."
  do_download
else
  if [ $(local_tag) == remote_latest_tag ]; then
    echo "Local binary "bin/$APP" is already the latest version"
    exit 0
  else
    echo "Try downloading latest version of $APP $remote_latest_tag from GitHub..."
    do_download
  fi
fi
