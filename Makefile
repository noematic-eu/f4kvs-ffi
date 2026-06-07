.PHONY: build test

build:
	cargo build -p f4kvs-ffi --release

test:
	cargo test -p f4kvs-ffi
