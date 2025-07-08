.PHONY: run build test coverage

test:
	@cargo test

run:
	@cargo run --bin hibiki

build:
	@cargo build --release

coverage: 
	@RUSTFLAGS="-C instrument-coverage" cargo tarpaulin \
					--workspace \
					--timeout 180 \
					--out Html \
					--no-fail-fast \
					--locked \
					--engine llvm
