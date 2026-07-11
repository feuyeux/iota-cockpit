# iota-cockpit-simulator

An independent cockpit world simulation desktop application and agent runtime.

The project uses a new Tauri 2, React 19, TypeScript, Vite, Tailwind, and Lucide desktop application. It reuses only iota-core as an external Rust library dependency; it does not reuse iota-cli, iota-desktop, iota-kanban, or the iota daemon.

The complete product, architecture, interaction model, implementation plan, and acceptance criteria are documented in [doc/001.md](doc/001.md).

## Current implementation

This repository currently implements the Phase 0 deterministic smoke-scenario slice and a desktop UI shell:

- Rust workspace with pure simulation core, scenario loading, recording/replay, evaluation, and `cockpit-runner`.
- `scenarios/smoke-in-cockpit.yaml` drives smoke detection, a scripted shutdown action, recording, replay, and evaluation.
- `apps/cockpit-desktop` is an independent React 19 + Vite 7 + TypeScript + Tailwind 4 + Lucide app with typed runner state, controls, world, timeline, trace, and evaluation panels.

The iota-core ACP/MCP adapter is not wired yet because the local reference path from the plan, `D:\coding\iota-sympantos\crates\iota-core`, is not present in this workspace.

## Verify

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p cockpit-runner -- validate scenarios/smoke-in-cockpit.yaml
cargo run -p cockpit-runner -- run scenarios/smoke-in-cockpit.yaml --ticks 80
cd apps/cockpit-desktop
npm install
npm test
npm run build
```

## Run the desktop shell

```bash
cd apps/cockpit-desktop
npm run dev -- --host 127.0.0.1 --port 15342
```

Open <http://127.0.0.1:15342>.
