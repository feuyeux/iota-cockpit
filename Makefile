.PHONY: help dev dev-desktop dev-web test test-rust test-desktop test-all build build-release clean install prepare-sidecar lint lint-fix validate calibrate simulator eval-suite check ci

# Default target
.DEFAULT_GOAL := help

# Colors
CYAN := \033[0;36m
GREEN := \033[0;32m
YELLOW := \033[0;33m
RED := \033[0;31m
RESET := \033[0m

help: ## Show this help message
	@echo "$(CYAN)IOTA Cockpit - Available Commands$(RESET)"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(GREEN)%-20s$(RESET) %s\n", $$1, $$2}'
	@echo ""
	@echo "$(YELLOW)Quick Start:$(RESET)"
	@echo "  make install        # Install dependencies"
	@echo "  make dev            # Start desktop app"
	@echo "  make test           # Run all tests"
	@echo ""
	@echo "$(YELLOW)Note:$(RESET) For detailed options, see ./run.sh --help"
	@echo ""

install: ## Install all dependencies (Rust crates + Node packages)
	@echo "$(CYAN)📦 Installing dependencies...$(RESET)"
	@cd apps/cockpit-desktop && npm ci
	@echo "$(GREEN)✅ Dependencies installed$(RESET)"

prepare-sidecar: ## Build sidecar binaries (simulator + evaluator)
	@echo "$(CYAN)🔨 Building sidecar binaries...$(RESET)"
	@cd apps/cockpit-desktop && npm run tauri:prepare-sidecar
	@echo "$(GREEN)✅ Sidecar binaries ready$(RESET)"

dev: ## Interactive development mode - starts desktop application
	@./run.sh

dev-clean: ## Start desktop app with clean build
	@./run.sh --clean

dev-web: ## Start web development server only
	@echo "$(CYAN)🌐 Starting web development server...$(RESET)"
	@cd apps/cockpit-desktop && npm run dev

test: test-rust test-desktop ## Run all tests (Rust + Desktop)

test-rust: ## Run Rust workspace tests
	@echo "$(CYAN)🦀 Running Rust tests...$(RESET)"
	@cargo test --workspace

test-desktop: ## Run desktop application tests
	@echo "$(CYAN)🖥️  Running desktop tests...$(RESET)"
	@cd apps/cockpit-desktop && npm test && npm run test:tsc

test-all: ## Run all tests including scenario validation
	@./scripts/test.sh --all

test-watch: ## Run desktop tests in watch mode
	@cd apps/cockpit-desktop && npm run test:watch

build: ## Build workspace (debug mode)
	@echo "$(CYAN)🔨 Building workspace (debug)...$(RESET)"
	@cargo build --workspace
	@cd apps/cockpit-desktop && npm run build

build-release: ## Build workspace (release mode)
	@echo "$(CYAN)🔨 Building workspace (release)...$(RESET)"
	@cargo build --release --workspace
	@cd apps/cockpit-desktop && npm run tauri:build

lint: ## Run all linters (Rust fmt + clippy, TypeScript)
	@echo "$(CYAN)🔍 Running linters...$(RESET)"
	@cargo fmt --all --check
	@cargo clippy --workspace --all-targets -- -D warnings
	@cd apps/cockpit-desktop && npm run test:tsc
	@echo "$(GREEN)✅ All linters passed$(RESET)"

lint-fix: ## Auto-fix linting issues
	@echo "$(CYAN)🔧 Fixing linting issues...$(RESET)"
	@cargo fmt --all
	@cargo clippy --workspace --all-targets --fix --allow-dirty -- -D warnings
	@echo "$(GREEN)✅ Linting issues fixed$(RESET)"

validate: ## Validate all scenarios
	@echo "$(CYAN)📋 Validating scenarios...$(RESET)"
	@for scenario in scenarios/*.yaml; do \
		echo "  Checking $$(basename $$scenario)..."; \
		cargo run -q -p cockpit-simulator -- validate "$$scenario" || exit 1; \
	done
	@echo "$(GREEN)✅ All scenarios valid$(RESET)"

calibrate: ## Run calibration scripts
	@echo "$(CYAN)📐 Running calibration...$(RESET)"
	@python3 calibration/calibrate.py
	@python3 calibration/calibrate_vehicle_fire.py
	@python3 calibration/validate_human_heat_stress.py
	@python3 calibration/verify.py
	@echo "$(GREEN)✅ Calibration complete$(RESET)"

simulator: ## Run simulator with smoke-in-cockpit scenario
	@echo "$(CYAN)⚙️  Running simulator...$(RESET)"
	@cargo run -p cockpit-simulator -- run scenarios/smoke-in-cockpit.yaml --ticks 80

simulator-live: ## Run simulator in live mode
	@echo "$(CYAN)⚙️  Running simulator (live mode)...$(RESET)"
	@cargo run -p cockpit-simulator -- run-live scenarios/smoke-in-cockpit.yaml --ticks 80

simulator-live-acp: ## Run simulator in live mode with ACP features
	@echo "$(CYAN)⚙️  Running simulator (live mode with ACP)...$(RESET)"
	@cargo run -p cockpit-simulator --features live-acp -- run-live scenarios/smoke-in-cockpit.yaml --ticks 80

eval-suite: ## Run complete evaluation suite (debug mode)
	@./scripts/eval.sh

eval-suite-release: ## Run complete evaluation suite (release mode)
	@./scripts/eval.sh --release

check: lint test validate ## Run all checks (lint + test + validate)

ci: check build ## Run CI pipeline (check + build)

clean: ## Clean all build artifacts
	@echo "$(CYAN)🧹 Cleaning build artifacts...$(RESET)"
	@cargo clean
	@rm -rf apps/cockpit-desktop/dist
	@rm -rf apps/cockpit-desktop/node_modules
	@rm -rf apps/cockpit-desktop/src-tauri/target
	@rm -rf apps/cockpit-desktop/src-tauri/binaries
	@echo "$(GREEN)✅ Clean complete$(RESET)"

clean-deep: clean ## Deep clean including Cargo cache
	@echo "$(CYAN)🧹 Deep cleaning...$(RESET)"
	@rm -rf target
	@rm -rf ~/.cargo/registry/index/*
	@echo "$(GREEN)✅ Deep clean complete$(RESET)"

info: ## Show environment information
	@echo "$(CYAN)📋 Environment Information$(RESET)"
	@echo ""
	@echo "Rust:"
	@rustc --version
	@cargo --version
	@echo ""
	@echo "Node.js:"
	@node --version
	@npm --version
	@echo ""
	@echo "Project:"
	@echo "  Root: $$(pwd)"
	@echo "  Workspaces: apps/cockpit-desktop"
	@echo ""

scenarios: ## List all available scenarios
	@echo "$(CYAN)📋 Available Scenarios:$(RESET)"
	@echo ""
	@for scenario in scenarios/*.yaml; do \
		echo "  • $$(basename $$scenario)"; \
	done
	@echo ""

docs: ## Open documentation
	@echo "$(CYAN)📚 Opening documentation...$(RESET)"
	@open docs/user-guide-zh.md || xdg-open docs/user-guide-zh.md || cat docs/user-guide-zh.md
