import type { RunnerEvent, ScenarioSummary } from "./types/simulation";

type TauriCore = {
  invoke: <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
};

function tauriCore(): TauriCore | undefined {
  return (window as Window & { __TAURI_INTERNALS__?: unknown; __TAURI__?: TauriCore }).__TAURI__;
}

export interface RunnerClient {
  connect(): Promise<void>;
  validateScenario(path: string): Promise<ScenarioSummary>;
  createRun(): Promise<string>;
  start(): Promise<void>;
  pause(): Promise<void>;
  step(): Promise<void>;
  stop(): Promise<void>;
  snapshot(cursor?: number): Promise<RunnerEvent[]>;
}

export const runnerClient: RunnerClient = {
  async connect() {
    const tauri = tauriCore();
    if (!tauri) return;
    await tauri.invoke("connect_runner");
  },
  async validateScenario(path: string) {
    const tauri = tauriCore();
    if (!tauri) {
      return {
        id: "smoke-in-cockpit",
        path,
        schemaVersion: 1,
        scenarioHash: "dev-preview",
        seed: 42,
        agentId: "cockpit-agent"
      };
    }
    return tauri.invoke("validate_scenario", { path });
  },
  async createRun() {
    const tauri = tauriCore();
    if (!tauri) return "preview-run";
    return tauri.invoke("create_simulation_run");
  },
  async start() {
    await tauriCore()?.invoke("start_simulation");
  },
  async pause() {
    await tauriCore()?.invoke("pause_simulation");
  },
  async step() {
    await tauriCore()?.invoke("step_simulation");
  },
  async stop() {
    await tauriCore()?.invoke("stop_simulation");
  },
  async snapshot(cursor?: number) {
    const tauri = tauriCore();
    if (!tauri) return [];
    return tauri.invoke("get_simulation_events", { cursor });
  }
};
