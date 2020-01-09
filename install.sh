#!/usr/bin/env bash

set -u

version=v0.4

APP=maple

DOWNLOAD_URL="https://github.com/liuchengxu/vim-clap/releases/download/$version"

exists() {
  command -v "$1" >/dev/null 2>&1
}

try_download() {
  local url=$1
  local temp=${TMPDIR}/maple
  if exists "curl"; then
    curl -fLo "$temp"  "$url"
  elif exists 'wget'; then
    wget --output-document="$temp" "$url"
  else
    echo 'curl or wget is required'
    exit 1
  fi
  ls $TMPDIR
  # move $temp
}

main() {
  arch=$(uname -sm)
  case "${arch}" in
      "Linux x86_64")
        download "$APP"-x86_64-unknown-linux-gnu ;;
      "Darwin x86_64")
        download "$APP"-x86_64-apple-darwin ;;
      *)
        echo "No prebuilt binary available for ${arch}.";
        try_build ;;
  esac
}

# main
try_download "$DOWNLOAD_URL/$APP"-x86_64-unknown-linux-gnu
