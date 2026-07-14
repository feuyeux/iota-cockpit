import { describe, expect, it } from "vitest";
import {
  actionStatusLabel,
  alertLabel,
  commandLabel,
  eventDescription,
  eventLabel,
  evaluationExplanation,
  lifecycleLabel
} from "./domainPresentation";

describe("domain presentation", () => {
  it("localizes known runtime evidence while preserving unknown codes", () => {
    expect(eventLabel("ThermalComfortRestored", "zh-CN")).toBe("热舒适已恢复");
    expect(commandLabel("cyberSafeModeActivate", "zh-CN")).toBe("激活网络安全模式");
    expect(alertLabel("EvRangeRisk", "en-US")).toBe("EV range risk");
    expect(lifecycleLabel("Recovering", "zh-CN")).toBe("恢复中");
    expect(actionStatusLabel("rejected", "zh-CN")).toBe("已拒绝");
    expect(eventLabel("PluginSpecificEvent", "zh-CN")).toBe("PluginSpecificEvent");
  });

  it("uses localized descriptions for benchmark success events", () => {
    expect(eventDescription("MedicalResponseActivated", "raw", "zh-CN"))
      .toContain("稳定患者");
    expect(eventDescription("UnknownEvent", "raw", "zh-CN")).toBe("raw");
    expect(evaluationExplanation("在截止时间前达到热舒适目标", "en-US"))
      .toBe("thermal comfort target was reached before the deadline");
  });
});
