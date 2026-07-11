# Cockpit Simulation TODO

Status: in progress. This file records the next implementation work from `doc/001.md` and preserves the current handoff state.

## Immediate Handoff

- Finish the bounded runner event-history slice currently left uncommitted:
  - `crates/cockpit-runner/src/ipc/handler.rs` retains at most `MAX_EVENT_HISTORY` events and returns `firstAvailableCursor` plus `resetRequired` from `GetSimulationEvents`.
  - `tests/contract/runner_ipc.rs` adds coverage for a stale cursor after retention trimming.
  - Re-export `MAX_EVENT_HISTORY` from `crates/cockpit-runner/src/ipc/mod.rs`, update the test import, run formatting/tests/clippy, and commit.
- Extend Tauri `get_simulation_events` and `runnerClient.snapshot` to return cursor metadata, not only events.
- On desktop reconnect, when `resetRequired` is true, fetch the authoritative simulation snapshot and replace stale event-derived state before consuming retained events.

## Runtime And ACP

- Integrate live ACP execution into the runner lifecycle, recording fallback/degraded evidence per tick.
- Add live backend startup and failure handling tests.
- Integrate retry/circuit-breaker policy with a cancellable/retryable iota-core turn handle when the public API supports it. The pinned iota-core API currently exposes `run_with_timing` but no public active-turn cancellation handle.
- Keep cancellation semantics explicit: pending cockpit action cancellation exists; live ACP process cancellation is not yet implemented or verified.
- Enforce backend network policy through iota configuration and add integration evidence.

## Plugin Execution

- Connect validated plugin executor output to `cockpit-runner` and the core tick path.
- Record plugin hash, version, failures, and policy decisions in recordings.
- Apply `PauseRun` and `FailRun` plugin policies to runner state, not just plugin status.
- Add bounded plugin tick execution and OS-level isolation/sandboxing. The current host validates untrusted output but does not run third-party plugin binaries.

## Simulation And Recording

- Implement bounded recording queue behavior with explicit pause/fail/drop policy and observable queue health.
- Build scheduled event delivery, subscriptions, and versioned influence rules beyond the smoke scenario's direct systems.
- Expand conflict policies and deterministic arbitration evidence.
- Add recording migration tooling and a compatibility strategy beyond version rejection.
- Ensure every log/export path has an artifact scan/redaction test, including screenshots and issue/runbook output.

## Desktop And Packaging

- Add desktop reducer/component tests with a real frontend test runner.
- Add manual/native Tauri acceptance for replay, reconnect reset, approval, errors, loading, and empty states.
- Validate external runner process recovery across a real process restart.
- Package the runner alongside Tauri on supported operating systems.
- Add recording selection via a native file dialog if desired after the path-based replay workflow is manually accepted.

## Acceptance Evidence

- Run cross-platform performance acceptance at 1,000 entities and 10,000 events/minute, including peak memory measurements.
- Add clean-machine CI validation for the pinned iota-core git dependency.
- Keep `README.md` and `docs/simulation-mvp-acceptance.md` synchronized with only verified claims.
- Do not mark the MVP complete until every item in the `doc/001.md` checklist has authoritative evidence.

