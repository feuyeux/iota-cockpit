import { afterEach, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { SimulationActivityFeed } from "./SimulationActivityFeed";
import { I18nProvider } from "../i18n";
import { simulatorClient } from "../simulatorClient";
import { initialSimulationModel } from "../state/simulationReducer";

let container: HTMLDivElement | null = null;
let root: Root | null = null;

function render() {
  const dispatch = vi.fn();
  container = document.createElement("div");
  document.body.appendChild(container);
  root = createRoot(container);
  act(() => {
    root!.render(
      <I18nProvider>
        <SimulationActivityFeed
          model={{
            ...initialSimulationModel,
            runId: "run-audit",
            tick: 8,
            auditRecovery: { totalEvents: 1_200, earliestOffset: 1_024 }
          }}
          dispatch={dispatch}
        />
      </I18nProvider>
    );
  });
  return { element: container, dispatch };
}

afterEach(() => {
  act(() => root?.unmount());
  container?.remove();
  container = null;
  root = null;
  vi.restoreAllMocks();
  window.localStorage.clear();
});

describe("SimulationActivityFeed durable audit recovery", () => {
  it("shows truncation and requests the adjacent earlier evidence page", async () => {
    const page = vi.spyOn(simulatorClient, "recordedAuditPage").mockResolvedValue({
      events: [],
      offset: 768,
      totalEvents: 1_200,
      truncated: false
    });
    const { element, dispatch } = render();

    expect(element.textContent).toContain("已恢复最近 1024 条证据，共 1200 条。");
    const load = Array.from(element.querySelectorAll("button")).find(
      (button) => button.textContent === "加载更早证据"
    ) as HTMLButtonElement;
    await act(async () => {
      load.click();
      await Promise.resolve();
    });

    expect(page).toHaveBeenCalledWith({ runId: "run-audit", startTick: 0, endTick: 8, offset: 768 });
    expect(dispatch).toHaveBeenCalledWith({
      type: "recordedAuditPage",
      events: [],
      totalEvents: 1_200,
      earliestOffset: 768
    });
  });
});
