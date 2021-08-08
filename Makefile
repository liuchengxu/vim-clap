all: maple python-dynamic-module

MAKE_CMD ?= "make"

maple:
	cargo build --release

python-dynamic-module:
	cd pythonx/clap && $(MAKE_CMD) build

.PHONY: all maple python-dynamic-module
