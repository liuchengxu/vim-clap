OS := $(shell uname -s | tr A-Z a-z)

LIB := fuzzymatch_rs

help:
	@echo "usage: make [OPTIONS]"
	@echo "    help        Show this message"
	@echo "    test        Run the fuzzy match benchmark using pytest"
	@echo "    build       Build the Rust extension and copy to the right place"

tools:
	pip3 install pytest pytest-benchmark

test:
	python3 -m pytest test_fzy_with_rust.py

run-cargo:
	@echo "\033[1;34m==>\033[0m Trying to build rust extension"; \
	cd fuzzymatch-rs; \
	cargo +nightly build --release

ifeq ($(OS),darwin)
move-so: run-cargo
	cp -f fuzzymatch-rs/target/release/lib$(LIB).dylib $(LIB).so
else
move-so: run-cargo
	cp -f fuzzymatch-rs/target/release/lib$(LIB).so $(LIB).so
endif

post-check:
	@python3 -c 'import fuzzymatch_rs' >/dev/null && echo 'Build successfully!' || echo 'Build failed!'

build: move-so post-check

.PHONY: help test build tools run-cargo move-so post-check
