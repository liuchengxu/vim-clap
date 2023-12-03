maple:
	cargo build --release

all: maple python-dynamic-module

MAKE_CMD ?= "make"

python-dynamic-module:
	cd pythonx/clap && $(MAKE_CMD) build

config-md:
	cd crates/config_gen && cargo run

.PHONY: all maple python-dynamic-module config-md
