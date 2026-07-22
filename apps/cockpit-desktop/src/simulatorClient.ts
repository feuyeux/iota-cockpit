import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { APP_CONFIG } from "./config/constants";
import type {
  EvaluationReportRecord,
  RecordedAuditPage,
  RecordingDiff,
  SimulatorEvent,
  SimulatorEventBatch,
  ScenarioSummary,
  WorldSnapshot,
  LiveRunSummary,
  OfflineRunSummary,
  RulePolicyStatus
} from "./types/simulation";

function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

function invokeSimulator<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) return Promise.resolve(undefined as T);
  return invoke<T>(command, args);
}

export interface SimulatorClient {
  connect(): Promise<void>;
  validateScenario(path: string): Promise<ScenarioSummary>;
  listRulePolicies(): Promise<RulePolicyStatus>;
  selectRulePolicy(policyId: string): Promise<unknown>;
  createOfflineRun(path: string): Promise<OfflineRunSummary>;
  createLiveRun(path: string, timeoutMs: number): Promise<LiveRunSummary>;
  start(): Promise<void>;
  pause(): Promise<void>;
  stepLive(): Promise<unknown>;
  step(): Promise<unknown>;
  stop(): Promise<void>;
  resume(scenarioPath: string, runId: string): Promise<void>;
  approveAction(requestId: string): Promise<unknown>;
  rejectAction(requestId: string, reason?: string): Promise<unknown>;
  cancelAgentTurn(): Promise<void>;
  cancelLiveTurn(): Promise<void>;
  setApprovalRequired(required: boolean): Promise<void>;
  startReplay(scenarioPath: string, recordingPath: string): Promise<unknown>;
  diffRecordings(sourceRecordingPath: string, candidateRecordingPath: string): Promise<RecordingDiff>;
  snapshot(cursor?: number): Promise<SimulatorEventBatch>;
  recordedAuditPage(request: {
    runId: string;
    startTick: number;
    endTick: number;
    offset?: number;
    afterSequence?: number;
    tailLimit?: number;
  }): Promise<RecordedAuditPage>;
  recordedAuditEvents(runId: string, startTick: number, endTick: number): Promise<{
    events: SimulatorEvent[];
    totalEvents: number;
    earliestOffset: number;
  }>;
  simulationSnapshot(): Promise<WorldSnapshot>;
  evaluateRun(runId: string, scenarioId: string): Promise<EvaluationReportRecord>;
  listEvaluationReports(): Promise<EvaluationReportRecord[]>;
  openScenarioFilePicker(): Promise<string | null>;
  openRecordingFilePicker(): Promise<string | null>;
}

export const simulatorClient: SimulatorClient = {
  async connect() {
    await invokeSimulator<void>("connect_simulator");
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
    return invokeSimulator("validate_scenario", { path });
  },
  async listRulePolicies() {
    if (!isTauri()) return { available: false, policies: [] };
    return invokeSimulator<RulePolicyStatus>("list_rule_policies");
  },
  async selectRulePolicy(policyId: string) {
    if (!isTauri()) return { policyId };
    return invokeSimulator("select_rule_policy", { policyId });
  },
  async createOfflineRun(path: string) {
    if (!isTauri()) return { runId: "preview-offline-run", status: "ready", scenarioHash: "dev-preview" };
    return invokeSimulator<OfflineRunSummary>("create_simulation_run", { path });
  },
  async createLiveRun(path: string, timeoutMs: number) {
    if (!isTauri()) return { runId: "preview-live-run", backend: "preview-no-backend" };
    return invokeSimulator<LiveRunSummary>("create_live_simulation_run", { path, timeoutMs });
  },
  async start() {
    await invokeSimulator<void>("start_simulation");
  },
  async pause() {
    await invokeSimulator<void>("pause_simulation");
  },
  async stepLive() {
    return invokeSimulator("step_live_simulation");
  },
  async step() {
    return invokeSimulator("step_simulation");
  },
  async stop() {
    await invokeSimulator<void>("stop_simulation");
  },
  async resume(scenarioPath, runId) {
    await invokeSimulator<void>("resume_simulation", { scenarioPath, runId });
  },
  async approveAction(requestId) {
    return invokeSimulator("approve_action", { requestId });
  },
  async rejectAction(requestId, reason) {
    return invokeSimulator("reject_action", { requestId, reason });
  },
  async cancelAgentTurn() {
    await invokeSimulator<void>("cancel_agent_turn");
  },
  async cancelLiveTurn() {
    await invokeSimulator<void>("cancel_live_turn");
  },
  async setApprovalRequired(required) {
    await invokeSimulator<void>("set_approval_required", { required });
  },
  async startReplay(scenarioPath, recordingPath) {
    return invokeSimulator("start_replay", { scenarioPath, recordingPath });
  },
  async diffRecordings(sourceRecordingPath, candidateRecordingPath) {
    if (!isTauri()) {
      return {
        equivalent: sourceRecordingPath === candidateRecordingPath,
        sourceMetrics: { ticks: 0, events: 0, toolCalls: 0, actionResults: 0, stateDiffs: 0 },
        candidateMetrics: { ticks: 0, events: 0, toolCalls: 0, actionResults: 0, stateDiffs: 0 },
        tickDifferences: [],
        truncated: false
      };
    }
    return invokeSimulator("diff_recordings", { sourceRecordingPath, candidateRecordingPath });
  },
  async snapshot(cursor?: number) {
    return (await invokeSimulator<SimulatorEventBatch>("get_simulation_events", { cursor })) ?? {
      events: [],
      nextCursor: cursor ?? 0,
      firstAvailableCursor: cursor ?? 0,
      resetRequired: false
    };
  },
  async recordedAuditPage(request) {
    if (!isTauri()) return { events: [], offset: request.offset ?? 0, totalEvents: 0, truncated: false };
    return invokeSimulator<RecordedAuditPage>("get_recorded_audit_events", {
      request: { ...request, limit: 256 }
    });
  },
  async recordedAuditEvents(runId, startTick, endTick) {
    if (!isTauri()) return { events: [], totalEvents: 0, earliestOffset: 0 };
    const events: SimulatorEvent[] = [];
    let afterSequence: number | undefined;
    let hasMore = true;
    let totalEvents = 0;
    let earliestOffset = 0;
    while (hasMore) {
      const page = await this.recordedAuditPage({
        runId,
        startTick,
        endTick,
        afterSequence,
        tailLimit: APP_CONFIG.MAX_AUDIT_RECOVERY_EVENTS,
      });
      events.push(...page.events.map((item) => item.event));
      afterSequence = page.nextSequence;
      hasMore = afterSequence !== undefined;
      totalEvents = page.totalEvents;
      if (events.length === page.events.length) earliestOffset = page.offset;
    }
    return { events, totalEvents, earliestOffset };
  },
  async simulationSnapshot() {
    return invokeSimulator<WorldSnapshot>("get_simulation_snapshot");
  },
  async evaluateRun(runId, scenarioId) {
    if (!isTauri()) throw new Error("Independent evaluation requires the Tauri desktop host");
    return invokeSimulator<EvaluationReportRecord>("evaluate_run", { runId, scenarioId });
  },
  async listEvaluationReports() {
    if (!isTauri()) return [];
    return invokeSimulator<EvaluationReportRecord[]>("list_evaluation_reports");
  },
  async openScenarioFilePicker() {
    if (!isTauri()) return null;
    const result = await open({
      multiple: false,
      directory: false,
      filters: [
        { name: "YAML", extensions: ["yaml", "yml"] },
        { name: "*", extensions: ["*"] },
      ],
    });
    if (!result) return null;
    if (typeof result === "string") return result;
    return (result as { path: string }).path ?? null;
  },
  async openRecordingFilePicker() {
    if (!isTauri()) return null;
    const result = await open({
      multiple: false,
      directory: false,
      filters: [
        { name: "JSON / JSONL", extensions: ["json", "jsonl"] },
        { name: "*", extensions: ["*"] },
      ],
    });
    if (!result) return null;
    if (typeof result === "string") return result;
    return (result as { path: string }).path ?? null;
  },
};
