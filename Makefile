all: maple python-dynamic-module

maple:
	cargo build --release

python-dynamic-module:
	cd pythonx/clap && make build

.PHONY: all maple python-dynamic-module
