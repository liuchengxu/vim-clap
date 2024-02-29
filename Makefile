maple:
	cargo build --release

all: maple python-dynamic-module

MAKE_CMD ?= "make"

python-dynamic-module:
	cd pythonx/clap && $(MAKE_CMD) build

config-md:
	cd crates/maple_config/doc_gen && cargo run

clippy:
	cargo clippy --workspace --all-features --all-targets -- -D warnings

release:
	cargo xtask release

fmt:
	cargo +nightly fmt --all

.PHONY: all maple python-dynamic-module config-md clippy release fmt
