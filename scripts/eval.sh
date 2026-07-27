#!/usr/bin/env bash
# Evaluation suite runner
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

echo "📊 IOTA Cockpit Evaluation Suite"
echo ""

# Parse options
MODE="debug"
SUITE="evaluations/suite.yaml"
OUTPUT_DIR="target"
BASELINE=""
MIN_PASS_RATE="1.0"

while [[ $# -gt 0 ]]; do
    case $1 in
        --release)
            MODE="release"
            shift
            ;;
        --suite)
            SUITE="$2"
            shift 2
            ;;
        --output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --baseline)
            BASELINE="$2"
            shift 2
            ;;
        --min-pass-rate)
            MIN_PASS_RATE="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--release] [--suite FILE] [--output DIR] [--baseline FILE] [--min-pass-rate 0..1]"
            exit 1
            ;;
    esac
done

# Build binaries
echo "🔨 Building cockpit-simulator and cockpit-evaluator ($MODE mode)..."
if [ "$MODE" = "release" ]; then
    cargo build --release -p cockpit-simulator -p cockpit-evaluator
    SIMULATOR_BIN="./target/release/cockpit-simulator"
    EVALUATOR_BIN="./target/release/cockpit-evaluator"
else
    cargo build -p cockpit-simulator -p cockpit-evaluator
    SIMULATOR_BIN="./target/debug/cockpit-simulator"
    EVALUATOR_BIN="./target/debug/cockpit-evaluator"
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Build command
CMD="$EVALUATOR_BIN"
CMD="$CMD --suite $SUITE"
CMD="$CMD --simulator-command $SIMULATOR_BIN"
CMD="$CMD --json-report $OUTPUT_DIR/evaluation-report.json"
CMD="$CMD --junit-report $OUTPUT_DIR/evaluation-junit.xml"

if [ -n "$BASELINE" ]; then
    CMD="$CMD --baseline $BASELINE"
fi

if [ "$MIN_PASS_RATE" != "1.0" ]; then
    CMD="$CMD --minimum-pass-rate $MIN_PASS_RATE"
fi

echo ""
echo "🚀 Running evaluation suite..."
echo "   Suite: $SUITE"
echo "   Output: $OUTPUT_DIR"
[ -n "$BASELINE" ] && echo "   Baseline: $BASELINE"
echo "   Min Pass Rate: $MIN_PASS_RATE"
echo ""

# Run evaluation
if eval $CMD; then
    echo ""
    echo "✅ Evaluation suite completed successfully!"
    echo ""
    echo "📄 Reports:"
    echo "   JSON: $OUTPUT_DIR/evaluation-report.json"
    echo "   JUnit: $OUTPUT_DIR/evaluation-junit.xml"
    exit 0
else
    EXIT_CODE=$?
    echo ""
    if [ $EXIT_CODE -eq 2 ]; then
        echo "❌ Evaluation suite failed: Release gate not met"
    else
        echo "❌ Evaluation suite failed with exit code: $EXIT_CODE"
    fi
    exit $EXIT_CODE
fi
