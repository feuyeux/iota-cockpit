import { afterEach, describe, expect, it } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { SimulationEvaluation } from "./SimulationEvaluation";
import { I18nProvider } from "../i18n";
import { initialSimulationModel } from "../state/simulationReducer";
import type { EvaluationResult } from "../types/simulation";

let container: HTMLDivElement | null = null;
let root: Root | null = null;

function render(evaluationOverride?: EvaluationResult) {
  container = document.createElement("div");
  document.body.appendChild(container);
  root = createRoot(container);
  act(() => {
    root!.render(
      <I18nProvider>
        <SimulationEvaluation
          model={{
            ...initialSimulationModel,
            state: "running",
            tick: 8,
            scenario: {
              id: "smoke-in-cockpit",
              path: "scenarios/smoke-in-cockpit.yaml",
              schemaVersion: 1,
              scenarioHash: "hash",
              seed: 42,
              agentId: "cockpit-agent"
            },
            humanTurns: [{
              tick: 6,
              backend: "synthetic",
              evidence: {
                humanId: "pilot-1",
                decision: { narrative: "", actions: [{ target: "engine-1", command: "engineShutdown" }], internalStateDelta: {} },
                toolCalls: [{ tool: "simulation.request_action", arguments: { target: "engine-1", command: "engineShutdown" } }]
              }
            }],
            actionResults: [{
              request: { requestId: "shutdown", agentId: "cockpit-agent", target: "engine-1", command: "engineShutdown", expectedStateVersion: 6, expiresAtTick: 9, correlationId: "c" },
              status: "applied",
              runId: "run",
              tick: 6,
              correlationId: "c"
            }],
            events: [{
              eventId: "shutdown-evidence",
              eventType: "EngineShutdown",
              runId: "run",
              tick: 6,
              source: "device-system",
              priority: 0,
              sequence: 1,
              correlationId: "c",
              payload: { message: "" }
            }],
            evaluation: evaluationOverride ?? { passed: true, score: 1, evidenceEventIds: ["shutdown-evidence"], firstFailureTick: null, explanation: "engine shutdown occurred within the smoke response deadline" }
          }}
        />
      </I18nProvider>
    );
  });
  return container;
}

afterEach(() => {
  act(() => root?.unmount());
  container?.remove();
  container = null;
  root = null;
  window.localStorage?.clear();
});

describe("SimulationEvaluation", () => {
  it("presents the risk-to-evidence path in Chinese with readable evidence", () => {
    const element = render();
    expect(element.textContent).toContain("仿真过程与评测");
    expect(element.textContent).toContain("模型决策");
    expect(element.textContent).toContain("系统动作");
    expect(element.textContent).toContain("动力系统已关闭");
    expect(element.textContent).toContain("通过");
  });

  it("shows execution, safety, and rule-level evaluation failures", () => {
    const element = render({
      passed: false,
      score: 0,
      evidenceEventIds: [],
      firstFailureTick: 8,
      explanation: "mandatory agent execution failed",
      executionPassed: false,
      executionError: "backend timeout",
      safetyPassed: false,
      safetyViolations: [{ tick: 7, requestId: "request", code: "TOOL_CALL_DENIED" }],
      trajectoryPassed: false,
      trajectory: {
        actionRequests: 3,
        appliedActions: 2,
        rejectedActions: 1,
        sideEffectToolCalls: 2,
        deniedToolCalls: 1,
        alertTickExposure: 5,
        firstAppliedActionTick: 6
      },
      ruleResults: [{
        ruleId: "thermal-comfort-restored",
        deadlineTick: 12,
        result: { passed: false, score: 0, evidenceEventIds: [], firstFailureTick: 8, explanation: "failed" }
      }]
    });
    expect(element.textContent).toContain("执行失败");
    expect(element.textContent).toContain("backend timeout");
    expect(element.textContent).toContain("TOOL_CALL_DENIED");
    expect(element.textContent).toContain("thermal-comfort-restored");
    expect(element.textContent).toContain("轨迹指标");
    expect(element.textContent).toContain("风险暴露: 5");
    expect(element.textContent).toContain("首次动作: t6");
  });
});
