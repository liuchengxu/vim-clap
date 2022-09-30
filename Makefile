all: maple python-dynamic-module

MAKE_CMD ?= "make"

maple:
	cargo build --release

python-dynamic-module:
	cd pythonx/clap && $(MAKE_CMD) build

install:
	cp -f target/release/maple bin/

.PHONY: all maple python-dynamic-module install
