all: maple rust-ext

maple:
	cargo build --release

rust-ext:
	cd pythonx/clap && make build

.PHONY: all maple rust-ext
