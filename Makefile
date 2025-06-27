.PHONY: run build test coverage

run:
	@cargo run --bin hibiki

generate-dev-ci-config:
	@sh scripts/generate_ci_config.sh dev

generate-prod-ci-config:
	@sh scripts/generate_ci_config.sh prod

generate-container-ci-config:
	@sh scripts/generate_ci_container_env.sh