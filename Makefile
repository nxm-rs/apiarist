# Apiarist Makefile
# Local development helpers for building and testing

# Image settings
IMAGE_NAME ?= apiarist
IMAGE_TAG ?= local
FULL_IMAGE := $(IMAGE_NAME):$(IMAGE_TAG)

# Kurtosis settings
ENCLAVE_NAME ?= apiary-local

.PHONY: help build test docker-build docker-run clean kurtosis-run kurtosis-clean

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

# ============================================================================
# Local Development
# ============================================================================

build: ## Build apiarist binary locally
	cargo build --release

test: ## Run all tests
	cargo test --workspace

check: ## Run cargo check (fast compilation check)
	cargo check --workspace

fmt: ## Format code
	cargo fmt --all

lint: ## Run clippy linter
	cargo clippy --workspace --all-targets -- -D warnings

# ============================================================================
# Docker
# ============================================================================

docker-build: ## Build Docker image locally (apiarist:local)
	docker build -t $(FULL_IMAGE) .
	@echo ""
	@echo "Built: $(FULL_IMAGE)"
	@echo ""
	@echo "To use with apiary:"
	@echo '  kurtosis run ../apiary '"'"'{"apiarist_image": "$(FULL_IMAGE)"}'"'"

docker-run: docker-build ## Build and run Docker image with --help
	docker run --rm $(FULL_IMAGE) --help

# ============================================================================
# Kurtosis Integration
# ============================================================================

kurtosis-run: docker-build ## Build local image and run with apiary
	kurtosis run ../apiary \
		--enclave $(ENCLAVE_NAME) \
		'{"apiarist_image": "$(FULL_IMAGE)"}'

kurtosis-run-minimal: docker-build ## Build and run with minimal config
	kurtosis run ../apiary \
		--enclave $(ENCLAVE_NAME) \
		--args-file ../apiary/network_params_local.json

kurtosis-logs: ## Show apiarist logs from running enclave
	kurtosis service logs $(ENCLAVE_NAME) apiarist -f

kurtosis-status: ## Check apiarist status in running enclave
	@PORT=$$(kurtosis port print $(ENCLAVE_NAME) apiarist api 2>/dev/null | cut -d':' -f2) && \
	if [ -n "$$PORT" ]; then \
		curl -s http://localhost:$$PORT/status | jq .; \
	else \
		echo "Error: Could not find apiarist port. Is the enclave running?"; \
	fi

kurtosis-results: ## Get apiarist results from running enclave
	@PORT=$$(kurtosis port print $(ENCLAVE_NAME) apiarist api 2>/dev/null | cut -d':' -f2) && \
	if [ -n "$$PORT" ]; then \
		curl -s http://localhost:$$PORT/results | jq .; \
	else \
		echo "Error: Could not find apiarist port. Is the enclave running?"; \
	fi

kurtosis-clean: ## Remove the local Kurtosis enclave
	kurtosis enclave rm $(ENCLAVE_NAME) --force || true

kurtosis-clean-all: ## Remove all Kurtosis enclaves
	kurtosis clean -a

# ============================================================================
# CI
# ============================================================================

ci: fmt lint test build ## Run all CI checks
	@echo "All CI checks passed!"

# ============================================================================
# Cleanup
# ============================================================================

clean: ## Clean build artifacts
	cargo clean
	rm -rf target/
