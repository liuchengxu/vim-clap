maple:
	cargo build --release

all: maple python-dynamic-module

MAKE_CMD ?= "make"

python-dynamic-module:
	cd pythonx/clap && $(MAKE_CMD) build

config-md:
	cd crates/maple_config/doc_gen && cargo run

clippy:
	cd crates && cargo clippy --workspace --all-features --all-targets -- -D warnings

.PHONY: all maple python-dynamic-module config-md clippy
