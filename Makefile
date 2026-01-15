.PHONY: run build test coverage sync-plutus

BRANCH := $(shell git rev-parse --abbrev-ref HEAD)
SCRIPTS_REPO := deltadefi-protocol/deltadefi-scripts

sync-plutus:
	@echo "Syncing plutus.json from $(SCRIPTS_REPO) branch: $(BRANCH)"
	@curl -sfL "https://raw.githubusercontent.com/$(SCRIPTS_REPO)/$(BRANCH)/plutus.json" \
		-o src/scripts/plutus.json || \
		(echo "Branch $(BRANCH) not found, falling back to main" && \
		curl -sfL "https://raw.githubusercontent.com/$(SCRIPTS_REPO)/main/plutus.json" \
		-o src/scripts/plutus.json)

test: sync-plutus
	@cargo test

run: sync-plutus
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
