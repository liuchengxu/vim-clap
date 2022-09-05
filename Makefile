maple:
	cargo build --release

all: maple python-dynamic-module

MAKE_CMD ?= "make"

python-dynamic-module:
	cd pythonx/clap && $(MAKE_CMD) build

.PHONY: all maple python-dynamic-module
