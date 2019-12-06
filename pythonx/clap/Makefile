OS := $(shell uname -s | tr A-Z a-z)

help:
	@echo "usage: make [OPTIONS]"
	@echo "    help        Show this message"
	@echo "    test        Run the fuzzy match benchmark using pytest"
	@echo "    build       Build the Rust extension and copy to the right place"

test:
	python3 -m pytest fzy_with_rust.py

run-cargo:
	@echo "\033[1;34m==>\033[0m Trying to build rust extension"; \
	cd fuzzymatch-rs; \
	cargo build --release

ifeq ($(OS),darwin)
move-so: run-cargo
	cp fuzzymatch-rs/target/release/libfuzzymatch_rs.dylib fuzzymatch_rs.so
else
move-so: run-cargo
	cp fuzzymatch-rs/target/release/libfuzzymatch_rs.so fuzzymatch_rs.so
endif

post-check:
	@python3 -c 'import fuzzymatch_rs' >/dev/null && echo 'Build successfully!' || echo 'Build failed!'

build: move-so post-check

.PHONY: help test build
