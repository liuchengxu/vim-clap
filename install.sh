#!/usr/bin/env bash

set -u

REPO=https://github.com/liuchengxu/vim-clap
APP=maple

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

remote_latest_tag() {
  git -c 'versionsort.suffix=-' ls-remote --exit-code --refs --sort='version:refname' --tags "$REPO" 'v0.*' \
    | tail --lines=1 \
    | awk -F "/" '{print $NF}'
}

try_download() {
  local remote_latest_tag=$(remote_latest_tag)
  echo "bin/$APP is empty, try downloading the latest prebuilt binary $APP $remote_latest_tag from GitHub ..."

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

download_prebuilt_binary() {
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
        echo "No prebuilt maple binary available for this platform ${arch}."
        echo "You can compile the binary locally by running `make` or `cargo build --release` if Rust has been installed."
        exit 1
        ;;
  esac
}

if [ ! -f "bin/$APP" ]; then
  download_prebuilt_binary
else
  "bin/$APP" upgrade
fi
