import { afterEach, describe, expect, it } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { SimulationProgress } from "./SimulationProgress";
import { I18nProvider } from "../i18n";

let container: HTMLDivElement | null = null;
let root: Root | null = null;

function render(tick: number, state: "running" | "completed" = "running") {
  container = document.createElement("div");
  document.body.appendChild(container);
  root = createRoot(container);
  act(() => {
    root!.render(
      <I18nProvider>
        <SimulationProgress deadlineTick={20} state={state} tick={tick} />
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

describe("SimulationProgress", () => {
  it("shows the completed tick count, percentage, and remaining steps", () => {
    const element = render(8);
    const progress = element.querySelector('[role="progressbar"]');

    expect(element.textContent).toContain("t8 / t20");
    expect(element.textContent).toContain("剩余 12 个节拍");
    expect(progress?.getAttribute("aria-valuenow")).toBe("8");
    expect(progress?.getAttribute("aria-valuetext")).toContain("40%");
  });

  it("clamps progress and announces the deadline once reached", () => {
    const element = render(24, "completed");

    expect(element.textContent).toContain("t20 / t20");
    expect(element.textContent).toContain("已到截止步数");
  });
});
