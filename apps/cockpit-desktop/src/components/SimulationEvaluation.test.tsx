import { afterEach, describe, expect, it } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { SimulationEvaluation } from "./SimulationEvaluation";
import { I18nProvider } from "../i18n";
import { initialSimulationModel } from "../state/simulationReducer";
import type { EvaluationReportRecord } from "../types/simulation";

let container: HTMLDivElement | null = null;
let root: Root | null = null;

function fixtureReport(passed = true): EvaluationReportRecord {
  return {
    id: "report-run",
    createdAtMs: 1,
    runId: "run",
    scenarioId: "smoke-in-cockpit",
    report: {
      schemaVersion: 1,
      verdict: passed ? "pass" : "fail",
      rubricId: "smoke-private",
      rubricVersion: "1",
      rubricHash: "sha256:rubric",
      inputHash: "sha256:input",
      schemaHash: "sha256:schema",
      deterministicResults: [{
        ruleId: passed ? "shutdown-before-spread" : "thermal-comfort-restored",
        deadlineTick: passed ? 30 : 12,
        verdict: passed ? "pass" : "fail",
        result: { passed, score: passed ? 1 : 0, evidenceEventIds: ["shutdown-evidence"], firstFailureTick: passed ? null : 8, explanation: "fixture" }
      }],
      evidence: [{ tick: 6, eventId: "shutdown-evidence", kind: "EngineShutdown" }],
      judges: [],
      judgeDisagreement: false,
      releaseGatePassed: passed,
      explanation: passed ? "all deterministic hidden-rubric gates passed" : "one or more deterministic hidden-rubric gates failed"
    }
  };
}

function render(report = fixtureReport()) {
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
            runId: "run",
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
              request: { requestId: "shutdown", agentId: "cockpit-agent", target: "engine-1", capabilityId: "engine.shutdown", expectedStateVersion: 6, expiresAtTick: 9, correlationId: "c" },
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
          }}
          completedReport={report}
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

  it("shows a failed final report and its failing deterministic rule", () => {
    const element = render(fixtureReport(false));
    expect(element.textContent).toContain("thermal-comfort-restored");
    expect(element.textContent).toContain("最终独立报告");
    expect(element.textContent).toContain("阻断");
  });
});
