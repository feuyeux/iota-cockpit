import { afterEach, describe, expect, it } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { SimulationEvaluation } from "./SimulationEvaluation";
import { I18nProvider } from "../i18n";
import { initialSimulationModel } from "../state/simulationReducer";

let container: HTMLDivElement | null = null;
let root: Root | null = null;

function render() {
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
                decision: { narrative: "", actions: [{ target: "engine-1", command: "engineShutdown" }], internalStateDelta: {} }
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
            evaluation: { passed: true, score: 1, evidenceEventIds: ["shutdown-evidence"], firstFailureTick: null, explanation: "engine shutdown occurred within the smoke response deadline" }
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
  window.localStorage.clear();
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
});
