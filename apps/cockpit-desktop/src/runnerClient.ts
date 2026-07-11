import { invoke } from "@tauri-apps/api/core";
import type { RunnerEvent, ScenarioSummary } from "./types/simulation";

function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

function invokeRunner<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) return Promise.resolve(undefined as T);
  return invoke<T>(command, args);
}

export interface RunnerClient {
  connect(): Promise<void>;
  validateScenario(path: string): Promise<ScenarioSummary>;
  createRun(path: string): Promise<string>;
  start(): Promise<void>;
  pause(): Promise<void>;
  step(): Promise<void>;
  stop(): Promise<void>;
  resume(scenarioPath: string, runId: string): Promise<void>;
  approveAction(requestId: string): Promise<unknown>;
  rejectAction(requestId: string, reason?: string): Promise<unknown>;
  cancelAgentTurn(): Promise<void>;
  setApprovalRequired(required: boolean): Promise<void>;
  startReplay(scenarioPath: string, recordingPath: string): Promise<unknown>;
  snapshot(cursor?: number): Promise<RunnerEvent[]>;
}

export const runnerClient: RunnerClient = {
  async connect() {
    await invokeRunner<void>("connect_runner");
  },
  async validateScenario(path: string) {
    if (!isTauri()) {
      return {
        id: "smoke-in-cockpit",
        path,
        schemaVersion: 1,
        scenarioHash: "dev-preview",
        seed: 42,
        agentId: "cockpit-agent"
      };
    }
    return invokeRunner("validate_scenario", { path });
  },
  async createRun(path: string) {
    if (!isTauri()) return "preview-run";
    return invokeRunner<string>("create_simulation_run", { path });
  },
  async start() {
    await invokeRunner<void>("start_simulation");
  },
  async pause() {
    await invokeRunner<void>("pause_simulation");
  },
  async step() {
    await invokeRunner<void>("step_simulation");
  },
  async stop() {
    await invokeRunner<void>("stop_simulation");
  },
  async resume(scenarioPath, runId) {
    await invokeRunner<void>("resume_simulation", { scenarioPath, runId });
  },
  async approveAction(requestId) {
    return invokeRunner("approve_action", { requestId });
  },
  async rejectAction(requestId, reason) {
    return invokeRunner("reject_action", { requestId, reason });
  },
  async cancelAgentTurn() {
    await invokeRunner<void>("cancel_agent_turn");
  },
  async setApprovalRequired(required) {
    await invokeRunner<void>("set_approval_required", { required });
  },
  async startReplay(scenarioPath, recordingPath) {
    return invokeRunner("start_replay", { scenarioPath, recordingPath });
  },
  async snapshot(cursor?: number) {
    return (await invokeRunner<RunnerEvent[]>("get_simulation_events", { cursor })) ?? [];
  }
};
