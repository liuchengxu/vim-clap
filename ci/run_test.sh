#!/usr/bin/env bash

cd "$(dirname "${BASH_SOURCE[0]}")" || exit

cd ..

cd crates && cargo test --verbose --all -- --nocapture
