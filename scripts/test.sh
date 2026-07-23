#!/usr/bin/env bash
# Comprehensive test runner
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

echo "🧪 Running IOTA Cockpit Test Suite"
echo ""

# Parse options
RUN_RUST=1
RUN_DESKTOP=1
RUN_SCENARIOS=0
RUN_LINT=0
VERBOSE=0

while [[ $# -gt 0 ]]; do
    case $1 in
        --rust-only)
            RUN_DESKTOP=0
            shift
            ;;
        --desktop-only)
            RUN_RUST=0
            shift
            ;;
        --with-scenarios)
            RUN_SCENARIOS=1
            shift
            ;;
        --with-lint)
            RUN_LINT=1
            shift
            ;;
        --verbose)
            VERBOSE=1
            shift
            ;;
        --all)
            RUN_SCENARIOS=1
            RUN_LINT=1
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--rust-only|--desktop-only] [--with-scenarios] [--with-lint] [--verbose] [--all]"
            exit 1
            ;;
    esac
done

FAILED=0

# Rust tests
if [ $RUN_RUST -eq 1 ]; then
    echo "📦 Running Rust workspace tests..."
    if [ $VERBOSE -eq 1 ]; then
        cargo test --workspace -- --nocapture || FAILED=1
    else
        cargo test --workspace || FAILED=1
    fi
    echo ""
fi

# Desktop tests
if [ $RUN_DESKTOP -eq 1 ]; then
    echo "🖥️  Running Desktop tests..."
    cd apps/cockpit-desktop
    npm test || FAILED=1
    npm run test:tsc || FAILED=1
    cd "$PROJECT_ROOT"
    echo ""
fi

# Scenario validation
if [ $RUN_SCENARIOS -eq 1 ]; then
    echo "📋 Validating scenarios..."
    for scenario in scenarios/*.yaml; do
        echo "  Checking $(basename "$scenario")..."
        cargo run -q -p cockpit-simulator -- validate "$scenario" || FAILED=1
    done
    echo ""
fi

# Linting
if [ $RUN_LINT -eq 1 ]; then
    echo "🔍 Running linters..."
    cargo fmt --all --check || FAILED=1
    cargo clippy --workspace --all-targets -- -D warnings || FAILED=1
    echo ""
fi

# Summary
if [ $FAILED -eq 0 ]; then
    echo "✅ All tests passed!"
    exit 0
else
    echo "❌ Some tests failed"
    exit 1
fi
