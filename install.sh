#!/usr/bin/env bash

set -u

version=v0.8

APP=maple

DOWNLOAD_URL="https://github.com/liuchengxu/vim-clap/releases/download/$version"

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
