#!/usr/bin/env bash

cd "$(dirname "${BASH_SOURCE[0]}")" || exit

cd ..

cargo install ripgrep

cargo test --verbose --all -- --nocapture
