#!/usr/bin/env bash

set -u

version=v0.4

APP=maple

DOWNLOAD_URL="https://github.com/liuchengxu/vim-clap/releases/download/$version"

exists() {
  command -v "$1" >/dev/null 2>&1
}

try_download() {
  local asset=$1
  local temp=${TMPDIR}/maple
  if exists "curl"; then
    curl -fLo "$temp" "$DOWNLOAD_URL/$asset"
  elif exists 'wget'; then
    wget --output-document="$temp" "$DOWNLOAD_URL/$asset"
  else
    echo 'curl or wget is required'
    exit 1
  fi
  chmod a+x "$temp"
  mv "$temp" bin/$APP
}

main() {
  arch=$(uname -sm)
  case "${arch}" in
      "Linux x86_64")
        try_download "$APP"-x86_64-unknown-linux-gnu ;;
      "Darwin x86_64")
        try_download "$APP"-x86_64-apple-darwin ;;
      *)
        echo "No prebuilt maple binary available for ${arch}."
        exit 1
        ;;
  esac
}

main
