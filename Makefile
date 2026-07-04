export CARGO_BUILD_JOBS ?= 4

.DEFAULT_GOAL := help
.PHONY: help build run check fmt lint test test-unit test-integration test-doc \
        coverage bench gallery doc package verify clean

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'

build: ## Debug build (library + mermaid-svg binary)
	cargo build

run: ## Run the CLI (pass flags via ARGS="...", defaults to --help)
	cargo run --bin mermaid-svg -- $(or $(ARGS),--help)

check: ## Fast typecheck of all targets
	cargo check --all-targets

fmt: ## Apply formatting
	cargo fmt

lint: ## fmt-check + clippy with warnings as errors
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings

test-unit: ## Unit tests (in-module #[cfg(test)] + sugiyama)
	cargo test --lib --bins

test-integration: ## Integration tests (tests/integration.rs; writes target/test-samples/*.svg)
	cargo test --test '*'

test-doc: ## Doctests (lib.rs examples)
	cargo test --doc

test: test-unit test-integration test-doc ## All tests (unit + integration + doctest)

coverage: ## Test coverage report (needs cargo-llvm-cov): summary + lcov + HTML under target/llvm-cov/
	@mkdir -p target/llvm-cov
	cargo llvm-cov --lcov --output-path target/llvm-cov/lcov.info
	cargo llvm-cov report --html
	cargo llvm-cov report

bench: ## Criterion benches: parse/<kind> + render/<kind> over samples/
	cargo bench

gallery: ## Regenerate assets/gallery/*.md from samples/
	cargo run --example gen-doc-diagrams

doc: ## Build rustdoc (embeds the gallery)
	cargo doc --no-deps

package: ## Dry-run crates.io packaging
	cargo package --allow-dirty

verify: lint test ## Hard gate: lint + all tests — must pass before pushing

clean: ## Remove build artifacts
	cargo clean
