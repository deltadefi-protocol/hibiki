.PHONY: run build test coverage

test:
	@cargo test

run:
	@cargo run --bin hibiki

build:
	@cargo build --release

generate-dev-ci-config:
	@sh scripts/generate_ci_config.sh dev

generate-prod-ci-config:
	@sh scripts/generate_ci_config.sh prod

generate-container-ci-config:
	@sh scripts/generate_ci_container_env.sh
